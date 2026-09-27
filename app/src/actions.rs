//! S07, S10, S11 and S14: sweeps, their progress over a `Channel`, the
//! activity log, and the original email behind an evidence link.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use et_core::action::bulk::{self, Event, Item, RunItem, Target};
use et_core::action::{self, Entry, unsubscribe};
use et_core::extract::{Preview, preview};
use et_core::ingest::{graph, imap, maildir, mbox};
use et_core::source::{self, unix_now};
use serde::Serialize;
use specta::Type;
use tauri::ipc::Channel;

use crate::AppState;
use crate::settings::read_sweep;
use crate::sources::{Access, SignInError, access};

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
    let reader = store.clone();
    let found = tauri::async_runtime::spawn_blocking(move || {
        let conn = reader.read()?;
        let Some(message_id) = action::get(&conn, action_id)?
            .and_then(|entry| entry.evidence)
            .and_then(|evidence| evidence.message_id)
        else {
            return Ok(None);
        };
        action::locate(&conn, message_id)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    let Some(locator) = found else {
        return Ok(Original::Gone);
    };
    let source = match access(&state, &store, locator.source_id).await {
        Ok(Some(source)) => source,
        Ok(None) => return Ok(Original::Gone),
        Err(SignInError::Refused { server_says, .. }) => {
            return Ok(Original::Unreachable {
                message: format!("sign-in refused: {server_says}"),
            });
        }
        Err(SignInError::Unreachable(message)) => return Ok(Original::Unreachable { message }),
        Err(SignInError::Failed(message)) => return Err(message),
    };
    let fetched = match source {
        Access::Imap { account, password } => {
            let Some(uid) = locator.uid() else {
                return Ok(Original::Gone);
            };
            imap::fetch_one(
                &account,
                &password,
                &locator.folder,
                locator.uid_validity,
                uid,
            )
            .await
            .map_err(|e| e.to_string())
        }
        Access::Outlook { token, .. } => graph::fetch_one(&token, &locator.locator)
            .await
            .map_err(|e| e.to_string()),
        Access::Mbox(path) => {
            let Ok(offset) = locator.locator.parse() else {
                return Ok(Original::Gone);
            };
            tauri::async_runtime::spawn_blocking(move || mbox::read_at(&path, offset))
                .await
                .map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())
        }
        Access::Maildir(root) => {
            let wanted = locator.locator.clone();
            tauri::async_runtime::spawn_blocking(move || maildir::read_one(&root, &wanted))
                .await
                .map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())
        }
    };
    match fetched {
        Ok(Some(raw)) => Ok(Original::Found {
            preview: preview(&raw),
        }),
        Ok(None) => Ok(Original::Gone),
        Err(err) => Ok(Original::Unreachable {
            message: err.to_string(),
        }),
    }
}

/// S08: the user finished a playbook. Returns the activity row.
#[tauri::command]
#[specta::specta]
pub async fn finish_playbook(
    state: tauri::State<'_, AppState>,
    service_id: u32,
) -> Result<Option<u32>, String> {
    let store = state.store()?;
    tauri::async_runtime::spawn_blocking(move || {
        store.write(move |conn| {
            action::playbook::finish(conn, i64::from(service_id), &source::now())
        })
    })
    .await
    .map_err(|e| e.to_string())?
    .map(|id| id.map(|id| id as u32))
    .map_err(|e| e.to_string())
}
