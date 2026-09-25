//! S02: run a scan and stream its progress over a `Channel` (PLAN.md 1.2).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use et_core::ingest::imap::{self, Account, ImapError, Options, Security};
use et_core::scan::{ImapScan, rebuild};
use et_core::source;
use serde::Serialize;
use specta::Type;
use tauri::ipc::Channel;

use crate::AppState;

#[derive(Clone, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ScanEvent {
    #[serde(rename_all = "camelCase")]
    Progress {
        fetched: u32,
        total: u32,
        senders: u32,
        subscriptions: u32,
        newsletters: u32,
        latest_find: Option<String>,
    },
    /// Fetching is done; senders, services and rollups are being rebuilt.
    Settling,
}

#[derive(Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ScanSummary {
    scanned: u32,
    senders: u32,
    subscriptions: u32,
    newsletters: u32,
}

#[derive(Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ScanError {
    #[serde(rename_all = "camelCase")]
    SignInRefused {
        host: String,
        server_says: String,
    },
    Unreachable {
        message: String,
    },
    Cancelled,
    AlreadyRunning,
    Failed {
        message: String,
    },
}

fn failed(message: impl ToString) -> ScanError {
    ScanError::Failed {
        message: message.to_string(),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn start_scan(
    state: tauri::State<'_, AppState>,
    source_id: u32,
    events: Channel<ScanEvent>,
) -> Result<ScanSummary, ScanError> {
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut running = state.scan.lock().map_err(failed)?;
        if running.is_some() {
            return Err(ScanError::AlreadyRunning);
        }
        *running = Some(cancel.clone());
    }
    let result = run(&state, i64::from(source_id), cancel, events).await;
    if let Ok(mut running) = state.scan.lock() {
        *running = None;
    }
    result
}

#[tauri::command]
#[specta::specta]
pub fn cancel_scan(state: tauri::State<'_, AppState>) {
    if let Ok(running) = state.scan.lock()
        && let Some(cancel) = running.as_ref()
    {
        cancel.store(true, Ordering::Relaxed);
    }
}

async fn run(
    state: &AppState,
    source_id: i64,
    cancel: Arc<AtomicBool>,
    events: Channel<ScanEvent>,
) -> Result<ScanSummary, ScanError> {
    let store = state.store().map_err(failed)?;
    let config = source::imap_config(&store, source_id)
        .map_err(failed)?
        .ok_or_else(|| failed("no such IMAP source"))?;
    let password = state
        .secrets
        .get(&source::password_secret(source_id))
        .map_err(failed)?
        .ok_or_else(|| failed("the app password is missing from the keychain"))?;
    let password = String::from_utf8_lossy(&password).into_owned();

    let account = Account {
        host: config.host.clone(),
        port: config.port,
        username: config.username.clone(),
        security: Security::Tls,
    };
    let scan = Arc::new(ImapScan::new(store.clone(), source_id, &config.username));
    let progress_scan = scan.clone();
    let progress_events = events.clone();
    imap::sync(
        &account,
        &password,
        scan.clone(),
        cancel,
        Options::default(),
        move |p| {
            let c = progress_scan.counts();
            let _ = progress_events.send(ScanEvent::Progress {
                fetched: p.fetched as u32,
                total: p.total as u32,
                senders: c.senders as u32,
                subscriptions: c.subscriptions as u32,
                newsletters: c.newsletters as u32,
                latest_find: c.latest_find,
            });
        },
    )
    .await
    .map_err(|err| match err {
        ImapError::Auth(server_says) => ScanError::SignInRefused {
            host: config.host.clone(),
            server_says,
        },
        ImapError::Cancelled => ScanError::Cancelled,
        ImapError::Connect { .. } | ImapError::Tls(_) => ScanError::Unreachable {
            message: err.to_string(),
        },
        other => failed(other),
    })?;

    let _ = events.send(ScanEvent::Settling);
    let totals = tauri::async_runtime::spawn_blocking(move || {
        let totals = rebuild(&store)?;
        source::mark_synced(&store, source_id)?;
        Ok::<_, et_core::store::StoreError>(totals)
    })
    .await
    .map_err(failed)?
    .map_err(failed)?;

    Ok(ScanSummary {
        scanned: scan.counts().scanned as u32,
        senders: totals.senders as u32,
        subscriptions: totals.subscriptions as u32,
        newsletters: totals.newsletters as u32,
    })
}
