//! S02: run a scan and stream its progress over a `Channel` (PLAN.md 1.2).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use et_core::action::{self, Kind, NewEntry, Outcome};
use et_core::extract::rfc3339_utc;
use et_core::ingest::graph::{self, GraphError};
use et_core::ingest::imap::{self, ImapError, Options};
use et_core::ingest::{maildir, mbox};
use et_core::scan::{ImportError, MailScan, rebuild};
use et_core::source::{self, unix_now};
use serde::Serialize;
use specta::Type;
use tauri::ipc::Channel;

use crate::AppState;
use crate::sources::{Access, SignInError, access, graph_refusal};

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
    /// S16's parse failure: a file could not be read past `message`. What
    /// came before it is kept and rebuilt.
    Stopped {
        message: u32,
        reason: String,
    },
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
        Err(ScanError::Stopped { message, reason }) => (
            Outcome::Failed,
            format!("stopped at message {message}: {reason}; everything before it is kept"),
        ),
        Err(ScanError::AlreadyRunning) => return,
    };
    let _ = tauri::async_runtime::spawn_blocking(move || {
        let target = source::reader(&store, source_id)
            .ok()
            .flatten()
            .map_or_else(|| "a mailbox".to_owned(), |r| r.target().to_owned());
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
    let source = access(state, &store, source_id)
        .await
        .map_err(sign_in_error)?
        .ok_or_else(|| failed("no such mail source"))?;
    // The account's own mail is skipped; a file has no account.
    let owner = match &source {
        Access::Imap { account, .. } => account.username.clone(),
        Access::Outlook { username, .. } => username.clone(),
        Access::Mbox(_) | Access::Maildir(_) => String::new(),
    };
    let scan = Arc::new(MailScan::new(store.clone(), source_id, &owner));
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

    let mut stopped = None;
    match source {
        Access::Imap { account, password } => {
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
        Access::Outlook { mut token, .. } => {
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
                        token = match access(state, &store, source_id)
                            .await
                            .map_err(sign_in_error)?
                        {
                            Some(Access::Outlook { token, .. }) => token,
                            _ => return Err(failed("the Outlook source is gone")),
                        };
                    }
                    Err(GraphError::Cancelled) => return Err(ScanError::Cancelled),
                    Err(err) => return Err(sign_in_error(graph_refusal(err))),
                }
            }
        }
        Access::Mbox(path) => {
            stopped = import(&scan, cancel, progress, move |scan, cancel, progress| {
                let file = mbox::open(&path).map_err(|e| stop(0, e))?;
                let total = mbox::count(&path).unwrap_or(0);
                progress(0, total);
                scan.import(
                    "mbox",
                    file.map(|m| {
                        m.map_err(|e| match e {
                            mbox::MboxError::Stopped { message, reason } => {
                                ImportError::Stopped { message, reason }
                            }
                            other => stop(0, other),
                        })
                    }),
                    cancel,
                    |read| progress(read, total.max(read)),
                )?;
                progress(total, total);
                Ok(())
            })
            .await?;
        }
        Access::Maildir(path) => {
            stopped = import(&scan, cancel, progress, move |scan, cancel, progress| {
                let files = maildir::open(&path).map_err(|e| stop(0, e))?;
                let total = files.total();
                progress(0, total);
                scan.import("maildir", files.map(Ok), cancel, |read| {
                    progress(read, total)
                })
            })
            .await?;
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
    if let Some((message, reason)) = stopped {
        return Err(ScanError::Stopped { message, reason });
    }

    Ok(ScanSummary {
        scanned: scan.counts().scanned as u32,
        senders: totals.senders as u32,
        subscriptions: totals.subscriptions as u32,
        newsletters: totals.newsletters as u32,
    })
}

fn stop(message: u64, err: impl ToString) -> ImportError {
    ImportError::Stopped {
        message,
        reason: err.to_string(),
    }
}

/// Runs a file import off the async runtime. A stop is returned as
/// `Some((message, reason))`, so the scan still rebuilds what was read.
async fn import(
    scan: &Arc<MailScan>,
    cancel: Arc<AtomicBool>,
    progress: impl Fn(u64, u64) + Send + 'static,
    read: impl FnOnce(&MailScan, &AtomicBool, &dyn Fn(u64, u64)) -> Result<(), ImportError>
    + Send
    + 'static,
) -> Result<Option<(u32, String)>, ScanError> {
    let scan = scan.clone();
    let result = tauri::async_runtime::spawn_blocking(move || read(&scan, &cancel, &progress))
        .await
        .map_err(failed)?;
    match result {
        Ok(()) => Ok(None),
        Err(ImportError::Stopped { message, reason }) => Ok(Some((message as u32, reason))),
        Err(ImportError::Cancelled) => Err(ScanError::Cancelled),
        Err(ImportError::Store(message)) => Err(ScanError::Failed { message }),
    }
}
