//! S02: run a scan and stream its progress over a `Channel` (PLAN.md 1.2).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use et_core::action::{self, Kind, NewEntry, Outcome};
use et_core::extract::rfc3339_utc;
use et_core::ingest::graph::{self, GraphError};
use et_core::ingest::imap::{self, ImapError, Options};
use et_core::scan::{MailScan, rebuild};
use et_core::source::{self, unix_now};
use serde::Serialize;
use specta::Type;
use tauri::ipc::Channel;

use crate::AppState;
use crate::sources::{Login, SignInError, credentials, graph_refusal};

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

fn sign_in_error(err: SignInError) -> ScanError {
    match err {
        SignInError::Refused { host, server_says } => {
            ScanError::SignInRefused { host, server_says }
        }
        SignInError::Unreachable(message) => ScanError::Unreachable { message },
        SignInError::Failed(message) => ScanError::Failed { message },
    }
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
    log(&state, i64::from(source_id), &result).await;
    result
}

/// Every scan leaves an S14 row, however it ended. A row that cannot be
/// written does not fail the scan.
async fn log(state: &AppState, source_id: i64, result: &Result<ScanSummary, ScanError>) {
    let Ok(store) = state.store() else {
        return;
    };
    let (outcome, detail) = match result {
        Ok(summary) => (
            Outcome::Succeeded,
            format!("{} new messages", summary.scanned),
        ),
        Err(ScanError::Cancelled) => (
            Outcome::Failed,
            "cancelled; everything read so far is kept".to_owned(),
        ),
        Err(ScanError::SignInRefused { server_says, .. }) => {
            (Outcome::Failed, format!("sign-in refused: {server_says}"))
        }
        Err(ScanError::Unreachable { message } | ScanError::Failed { message }) => {
            (Outcome::Failed, message.clone())
        }
        Err(ScanError::AlreadyRunning) => return,
    };
    let _ = tauri::async_runtime::spawn_blocking(move || {
        let target = source::mailbox(&store, source_id)
            .ok()
            .flatten()
            .map_or_else(|| "a mailbox".to_owned(), |m| m.username().to_owned());
        store.write(move |conn| {
            action::record(
                conn,
                &NewEntry {
                    kind: Kind::Sync,
                    target,
                    sender_id: None,
                    service_id: None,
                    source_id: Some(source_id),
                    at: rfc3339_utc(unix_now()),
                    outcome,
                    detail,
                    request: None,
                    message: None,
                },
            )
        })
    })
    .await;
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
    let login = credentials(state, &store, source_id)
        .await
        .map_err(sign_in_error)?
        .ok_or_else(|| failed("no such mail source"))?;
    let username = match &login {
        Login::Imap { account, .. } => account.username.clone(),
        Login::Outlook { username, .. } => username.clone(),
    };
    let scan = Arc::new(MailScan::new(store.clone(), source_id, &username));
    let progress = {
        let (scan, events) = (scan.clone(), events.clone());
        move |fetched: u64, total: u64| {
            let c = scan.counts();
            let _ = events.send(ScanEvent::Progress {
                fetched: fetched as u32,
                total: total as u32,
                senders: c.senders as u32,
                subscriptions: c.subscriptions as u32,
                newsletters: c.newsletters as u32,
                latest_find: c.latest_find,
            });
        }
    };

    match login {
        Login::Imap { account, password } => {
            imap::sync(
                &account,
                &password,
                scan.clone(),
                cancel,
                Options::default(),
                |p| progress(p.fetched, p.total),
            )
            .await
            .map_err(|err| match err {
                ImapError::Auth(server_says) => ScanError::SignInRefused {
                    host: account.host.clone(),
                    server_says,
                },
                ImapError::Cancelled => ScanError::Cancelled,
                ImapError::Connect { .. } | ImapError::Tls(_) => ScanError::Unreachable {
                    message: err.to_string(),
                },
                other => failed(other),
            })?;
        }
        Login::Outlook { mut token, .. } => {
            // An access token lasts about an hour. A long first scan outlives
            // it, and resumes with a fresh one from where it stopped.
            let mut renewals = 0;
            loop {
                let result = graph::sync(&token, scan.clone(), cancel.clone(), |p| {
                    progress(p.fetched, p.total)
                })
                .await;
                match result {
                    Ok(_) => break,
                    Err(GraphError::Expired(_)) if renewals < 3 => {
                        renewals += 1;
                        token = match credentials(state, &store, source_id)
                            .await
                            .map_err(sign_in_error)?
                        {
                            Some(Login::Outlook { token, .. }) => token,
                            _ => return Err(failed("the Outlook source is gone")),
                        };
                    }
                    Err(GraphError::Cancelled) => return Err(ScanError::Cancelled),
                    Err(err) => return Err(sign_in_error(graph_refusal(err))),
                }
            }
        }
    }

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
