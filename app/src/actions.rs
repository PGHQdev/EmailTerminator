//! S07, S10, S11 and S14: sweeps, their progress over a `Channel`, the
//! activity log, and the original email behind an evidence link.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use et_core::action::bulk::{self, Event, Item, RunItem, Target};
use et_core::action::{self, Entry, unsubscribe};
use et_core::extract::{Preview, preview};
use et_core::ingest::imap::{self, Account, Security};
use et_core::source::{self, unix_now};
use serde::Serialize;
use specta::Type;
use tauri::ipc::Channel;

use crate::AppState;
use crate::settings::read_sweep;

#[tauri::command]
#[specta::specta]
pub async fn sweep_review(
    state: tauri::State<'_, AppState>,
    targets: Vec<Target>,
) -> Result<Vec<Item>, String> {
    let store = state.store()?;
    let exclude = read_sweep(&state).exclude_critical;
    tauri::async_runtime::spawn_blocking(move || {
        bulk::review(&store, &targets, exclude, unix_now())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

/// Runs a sweep to its end or until `stop_sweep`; returns how many items
/// ran. One sweep at a time.
#[tauri::command]
#[specta::specta]
pub async fn run_sweep(
    state: tauri::State<'_, AppState>,
    items: Vec<RunItem>,
    events: Channel<Event>,
) -> Result<u32, String> {
    let store = state.store()?;
    let stop = Arc::new(AtomicBool::new(false));
    {
        let mut running = state.sweep.lock().map_err(|e| e.to_string())?;
        if running.is_some() {
            return Err("a sweep is already running".into());
        }
        *running = Some(stop.clone());
    }
    let result = bulk::run(
        store,
        items,
        stop,
        |route| async move { unsubscribe::send(&route).await },
        move |event| {
            let _ = events.send(event);
        },
    )
    .await
    .map_err(|e| e.to_string());
    if let Ok(mut running) = state.sweep.lock() {
        *running = None;
    }
    result
}

#[tauri::command]
#[specta::specta]
pub fn stop_sweep(state: tauri::State<'_, AppState>) {
    if let Ok(running) = state.sweep.lock()
        && let Some(stop) = running.as_ref()
    {
        stop.store(true, Ordering::Relaxed);
    }
}

#[tauri::command]
#[specta::specta]
pub async fn activity(state: tauri::State<'_, AppState>) -> Result<Vec<Entry>, String> {
    let store = state.store()?;
    tauri::async_runtime::spawn_blocking(move || action::log(&store))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

#[derive(Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Original {
    Found {
        preview: Preview,
    },
    /// The message is no longer where it was stored (S14's designed state).
    Gone,
    /// The mailbox did not answer; the evidence may still be there.
    Unreachable {
        message: String,
    },
}

/// Re-reads the email an action came from, from its mailbox.
#[tauri::command]
#[specta::specta]
pub async fn evidence_original(
    state: tauri::State<'_, AppState>,
    action_id: u32,
) -> Result<Original, String> {
    let store = state.store()?;
    let found = tauri::async_runtime::spawn_blocking(move || {
        let conn = store.read()?;
        let Some(message_id) = action::get(&conn, action_id)?
            .and_then(|entry| entry.evidence)
            .and_then(|evidence| evidence.message_id)
        else {
            return Ok(None);
        };
        let Some(locator) = action::locate(&conn, message_id)? else {
            return Ok(None);
        };
        let config = source::imap_config(&store, locator.source_id)?;
        Ok::<_, et_core::store::StoreError>(config.map(|c| (locator, c)))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    let Some((locator, config)) = found else {
        return Ok(Original::Gone);
    };
    let password = state
        .secrets
        .get(&source::password_secret(locator.source_id))
        .map_err(|e| e.to_string())?
        .ok_or("the app password is missing from the keychain")?;
    let account = Account {
        host: config.host,
        port: config.port,
        username: config.username,
        security: Security::Tls,
    };
    match imap::fetch_one(
        &account,
        &String::from_utf8_lossy(&password),
        &locator.folder,
        locator.uid_validity,
        locator.uid,
    )
    .await
    {
        Ok(Some(raw)) => Ok(Original::Found {
            preview: preview(&raw),
        }),
        Ok(None) => Ok(Original::Gone),
        Err(err) => Ok(Original::Unreachable {
            message: err.to_string(),
        }),
    }
}
