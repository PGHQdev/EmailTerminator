//! S01's paths and S12: connect a mailbox or add a file, list and remove what
//! is connected, and sign in to one for a sync or an evidence link.

use std::path::PathBuf;
use std::sync::Arc;

use et_core::ingest::graph::{self, GraphError};
use et_core::ingest::imap::{
    self, Account, FASTMAIL, GMAIL, ICLOUD, ImapError, Preset, Security, YAHOO,
};
use et_core::ingest::outlook::{self, OAuthError};
use et_core::ingest::{maildir, mbox};
use et_core::scan::rebuild;
use et_core::source::{self, FileConfig, ImapConfig, OutlookConfig, Reader};
use et_core::store::{Store, StoreError};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use crate::AppState;

/// Where Microsoft's sign-in and mail answer from; S16 names them when they
/// refuse.
const MICROSOFT: &str = "login.microsoftonline.com";
const GRAPH: &str = "graph.microsoft.com";

#[derive(Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ImapProvider {
    Gmail,
    Icloud,
    Fastmail,
    Yahoo,
    Other,
}

impl ImapProvider {
    fn preset(self) -> Option<Preset> {
        match self {
            ImapProvider::Gmail => Some(GMAIL),
            ImapProvider::Icloud => Some(ICLOUD),
            ImapProvider::Fastmail => Some(FASTMAIL),
            ImapProvider::Yahoo => Some(YAHOO),
            ImapProvider::Other => None,
        }
    }
}

/// A provider as the connect form lists it.
#[derive(Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    provider: ImapProvider,
    label: String,
    guide: Option<String>,
}

#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NewImapSource {
    provider: ImapProvider,
    email: String,
    password: String,
    /// Only for `other`.
    host: Option<String>,
    port: Option<u16>,
}

#[derive(Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SourceSummary {
    id: u32,
    kind: String,
    label: String,
    last_sync_at: Option<String>,
    message_count: u32,
}

impl From<source::Source> for SourceSummary {
    fn from(s: source::Source) -> Self {
        Self {
            id: s.id as u32,
            kind: s.kind,
            label: s.label,
            last_sync_at: s.last_sync_at,
            message_count: s.message_count as u32,
        }
    }
}

/// Why a mailbox could not be connected. S16 shows `SignInRefused` with the
/// server's own words.
#[derive(Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ConnectError {
    #[serde(rename_all = "camelCase")]
    SignInRefused {
        host: String,
        server_says: String,
        guide: Option<String>,
    },
    Unreachable {
        message: String,
    },
    Invalid {
        message: String,
    },
    /// The user stopped a sign-in that was waiting for the browser.
    Cancelled,
    Failed {
        message: String,
    },
}

fn failed(message: impl ToString) -> ConnectError {
    ConnectError::Failed {
        message: message.to_string(),
    }
}

#[tauri::command]
#[specta::specta]
pub fn imap_presets() -> Vec<ProviderInfo> {
    [
        ImapProvider::Gmail,
        ImapProvider::Icloud,
        ImapProvider::Fastmail,
        ImapProvider::Yahoo,
    ]
    .into_iter()
    .filter_map(|p| {
        p.preset().map(|preset| ProviderInfo {
            provider: p,
            label: preset.label.into(),
            guide: Some(preset.guide.into()),
        })
    })
    .chain([ProviderInfo {
        provider: ImapProvider::Other,
        label: "Other IMAP".into(),
        guide: None,
    }])
    .collect()
}

/// Signs in once to check the app password, then saves the source and puts
/// the password in the secret store.
#[tauri::command]
#[specta::specta]
pub async fn add_imap_source(
    state: tauri::State<'_, AppState>,
    input: NewImapSource,
) -> Result<SourceSummary, ConnectError> {
    let email = input.email.trim().to_owned();
    if !email.contains('@') || input.password.trim().is_empty() {
        return Err(ConnectError::Invalid {
            message: "Enter the mailbox address and its app password.".into(),
        });
    }
    let preset = input.provider.preset();
    let (host, port, label) = match (preset, input.host.as_deref().map(str::trim)) {
        (Some(p), _) => (p.host.to_owned(), p.port, p.label.to_owned()),
        (None, Some(host)) if !host.is_empty() => {
            (host.to_owned(), input.port.unwrap_or(993), host.to_owned())
        }
        (None, _) => {
            return Err(ConnectError::Invalid {
                message: "Enter the IMAP server name.".into(),
            });
        }
    };
    // Gmail shows app passwords in groups of four; the spaces are not part of it.
    let password = input.password.split_whitespace().collect::<String>();

    let account = Account {
        host: host.clone(),
        port,
        username: email.clone(),
        security: Security::Tls,
    };
    imap::check_sign_in(&account, &password)
        .await
        .map_err(|err| match err {
            ImapError::Auth(server_says) => ConnectError::SignInRefused {
                host: host.clone(),
                server_says,
                guide: preset.map(|p| p.guide.to_owned()),
            },
            ImapError::Connect { .. } | ImapError::Tls(_) => ConnectError::Unreachable {
                message: err.to_string(),
            },
            other => failed(other),
        })?;

    let config = ImapConfig {
        host,
        port,
        username: email.clone(),
    };
    save(
        &state,
        format!("{label} · {email}"),
        config,
        source::add_imap,
        Some((source::password_secret, password.as_bytes())),
    )
    .await
}

/// Opens Microsoft's sign-in in the browser, waits for it to come back,
/// checks the token against Graph, then saves the source and its refresh
/// token.
#[tauri::command]
#[specta::specta]
pub async fn add_outlook_source(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<SourceSummary, ConnectError> {
    let sign_in = outlook::SignIn::start().await.map_err(oauth_error)?;
    app.opener()
        .open_url(sign_in.url(), None::<&str>)
        .map_err(failed)?;
    if let Ok(mut waiting) = state.outlook.lock() {
        *waiting = Some(sign_in.canceller());
    }
    let tokens = sign_in.finish().await;
    if let Ok(mut waiting) = state.outlook.lock() {
        *waiting = None;
    }
    let tokens = tokens.map_err(oauth_error)?;
    let email = tokens
        .username
        .ok_or_else(|| failed("Microsoft did not say which mailbox signed in"))?;
    let refresh = tokens
        .refresh
        .ok_or_else(|| failed("Microsoft sent no refresh token"))?;

    graph::check_sign_in(&tokens.access)
        .await
        .map_err(|err| match err {
            GraphError::Expired(server_says) | GraphError::Refused(server_says) => {
                ConnectError::SignInRefused {
                    host: GRAPH.into(),
                    server_says,
                    guide: None,
                }
            }
            GraphError::Connect(message) => ConnectError::Unreachable { message },
            other => failed(other),
        })?;

    save(
        &state,
        format!("Outlook · {email}"),
        OutlookConfig { username: email },
        source::add_outlook,
        Some((source::refresh_secret, refresh.as_bytes())),
    )
    .await
}

#[derive(Clone, Copy, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum FileKind {
    Mbox,
    Maildir,
}

/// Asks for an mbox file or a Maildir folder, then adds it. `None` when the
/// picker is closed.
#[tauri::command]
#[specta::specta]
pub async fn choose_file_source(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    kind: FileKind,
) -> Result<Option<SourceSummary>, ConnectError> {
    let picked = tauri::async_runtime::spawn_blocking(move || {
        let dialog = app.dialog().file();
        match kind {
            FileKind::Mbox => dialog.set_title("Choose an mbox file").blocking_pick_file(),
            FileKind::Maildir => dialog
                .set_title("Choose a Maildir folder")
                .blocking_pick_folder(),
        }
    })
    .await
    .map_err(failed)?;
    let Some(path) = picked.and_then(|p| p.into_path().ok()) else {
        return Ok(None);
    };
    add_file(&state, kind, path).await.map(Some)
}

/// Adds a file or folder dropped on S01: a folder is a Maildir, a file an
/// mbox.
#[tauri::command]
#[specta::specta]
pub async fn add_dropped_source(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<SourceSummary, ConnectError> {
    let path = PathBuf::from(path);
    let kind = if path.is_dir() {
        FileKind::Maildir
    } else {
        FileKind::Mbox
    };
    add_file(&state, kind, path).await
}

/// Checks that the file or folder reads as mail, then saves it as a source.
/// Nothing is copied: a scan reads it where it is.
async fn add_file(
    state: &AppState,
    kind: FileKind,
    path: PathBuf,
) -> Result<SourceSummary, ConnectError> {
    let checked = {
        let path = path.clone();
        tauri::async_runtime::spawn_blocking(move || match kind {
            FileKind::Mbox => mbox::open(&path).map(drop).map_err(|e| match e {
                mbox::MboxError::NotMbox => "This file is not an mbox. Choose an .mbox export.",
                _ => "This file cannot be read.",
            }),
            FileKind::Maildir => maildir::open(&path).map(drop).map_err(|e| match e {
                maildir::MaildirError::NotMaildir => {
                    "This folder holds no Maildir: no cur or new folder inside."
                }
                _ => "This folder cannot be read.",
            }),
        })
        .await
        .map_err(failed)?
    };
    checked.map_err(|message| ConnectError::Invalid {
        message: message.into(),
    })?;

    let label = path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let add = match kind {
        FileKind::Mbox => source::add_mbox,
        FileKind::Maildir => source::add_maildir,
    };
    let config = FileConfig {
        path: path.to_string_lossy().into_owned(),
    };
    save(state, label, config, add, None).await
}

/// Stops an Outlook sign-in that is waiting for the browser.
#[tauri::command]
#[specta::specta]
pub fn cancel_outlook_sign_in(state: tauri::State<'_, AppState>) {
    if let Ok(waiting) = state.outlook.lock()
        && let Some(cancel) = waiting.as_ref()
    {
        cancel.cancel();
    }
}

fn oauth_error(err: OAuthError) -> ConnectError {
    match err {
        OAuthError::Refused(server_says) => ConnectError::SignInRefused {
            host: MICROSOFT.into(),
            server_says,
            guide: None,
        },
        OAuthError::Cancelled => ConnectError::Cancelled,
        OAuthError::Network(_) => ConnectError::Unreachable {
            message: err.to_string(),
        },
        other => failed(other),
    }
}

/// A secret's name for a source id, and the secret.
type Secret<'a> = (fn(i64) -> String, &'a [u8]);

/// Saves a checked source, then puts its secret, if it has one, in the
/// secret store.
async fn save<C: Send + 'static>(
    state: &AppState,
    label: String,
    config: C,
    add: fn(&Store, &str, &C) -> Result<i64, StoreError>,
    secret: Option<Secret<'_>>,
) -> Result<SourceSummary, ConnectError> {
    let store = state.store().map_err(failed)?;
    let (id, store) = tauri::async_runtime::spawn_blocking(move || {
        add(&store, &label, &config).map(|id| (id, store))
    })
    .await
    .map_err(failed)?
    .map_err(failed)?;
    if let Some((name, secret)) = secret {
        state.secrets.set(&name(id), secret).map_err(failed)?;
    }

    source::list(&store)
        .map_err(failed)?
        .into_iter()
        .find(|s| s.id == id)
        .map(SourceSummary::from)
        .ok_or_else(|| failed("the new source was not saved"))
}

/// Why a saved mailbox could not sign in.
pub(crate) enum SignInError {
    Refused { host: String, server_says: String },
    Unreachable(String),
    Failed(String),
}

/// How a scan or an evidence link reads a source.
pub(crate) enum Access {
    Imap { account: Account, password: String },
    Outlook { username: String, token: String },
    Mbox(PathBuf),
    Maildir(PathBuf),
}

/// The access to a source, or `None` for a source that is gone. An Outlook
/// source trades its refresh token for an access token, and keeps the new
/// refresh token Microsoft may send with it.
pub(crate) async fn access(
    state: &AppState,
    store: &Arc<Store>,
    source_id: i64,
) -> Result<Option<Access>, SignInError> {
    let fail = |e: &dyn std::fmt::Display| SignInError::Failed(e.to_string());
    let missing = || SignInError::Failed("the sign-in is missing from the keychain".into());
    let Some(reader) = source::reader(store, source_id).map_err(|e| fail(&e))? else {
        return Ok(None);
    };
    match reader {
        Reader::Imap(config) => {
            let password = state
                .secrets
                .get(&source::password_secret(source_id))
                .map_err(|e| fail(&e))?
                .ok_or_else(missing)?;
            Ok(Some(Access::Imap {
                account: Account {
                    host: config.host,
                    port: config.port,
                    username: config.username,
                    security: Security::Tls,
                },
                password: String::from_utf8_lossy(&password).into_owned(),
            }))
        }
        Reader::Outlook(config) => {
            let name = source::refresh_secret(source_id);
            let refresh = state
                .secrets
                .get(&name)
                .map_err(|e| fail(&e))?
                .ok_or_else(missing)?;
            let tokens = outlook::refresh(&String::from_utf8_lossy(&refresh))
                .await
                .map_err(|err| match err {
                    OAuthError::Refused(server_says) => SignInError::Refused {
                        host: MICROSOFT.into(),
                        server_says,
                    },
                    OAuthError::Network(_) => SignInError::Unreachable(err.to_string()),
                    other => fail(&other),
                })?;
            if let Some(refresh) = &tokens.refresh {
                state
                    .secrets
                    .set(&name, refresh.as_bytes())
                    .map_err(|e| fail(&e))?;
            }
            Ok(Some(Access::Outlook {
                username: config.username,
                token: tokens.access.to_string(),
            }))
        }
        Reader::Mbox(config) => Ok(Some(Access::Mbox(config.path.into()))),
        Reader::Maildir(config) => Ok(Some(Access::Maildir(config.path.into()))),
    }
}

/// S16 shows a refused Graph request as a sign-in failure.
pub(crate) fn graph_refusal(err: GraphError) -> SignInError {
    match err {
        GraphError::Expired(server_says) | GraphError::Refused(server_says) => {
            SignInError::Refused {
                host: GRAPH.into(),
                server_says,
            }
        }
        GraphError::Connect(message) => SignInError::Unreachable(message),
        other => SignInError::Failed(other.to_string()),
    }
}

#[tauri::command]
#[specta::specta]
pub fn list_sources(state: tauri::State<'_, AppState>) -> Result<Vec<SourceSummary>, String> {
    let store = state.store()?;
    Ok(source::list(&store)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(SourceSummary::from)
        .collect())
}

/// Removes a source (S12): its sign-in from the secret store first, then its
/// messages, then the dashboard is rebuilt from what is left.
#[tauri::command]
#[specta::specta]
pub async fn remove_source(state: tauri::State<'_, AppState>, id: u32) -> Result<(), String> {
    let store = state.store()?;
    let id = i64::from(id);
    for name in [source::password_secret(id), source::refresh_secret(id)] {
        state.secrets.delete(&name).map_err(|e| e.to_string())?;
    }
    tauri::async_runtime::spawn_blocking(move || {
        source::remove(&store, id)?;
        rebuild(&store).map(drop)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}
