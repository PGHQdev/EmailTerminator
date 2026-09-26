//! S01's IMAP and Outlook paths and S12's list: connect a mailbox, list what
//! is connected, and sign in to one for a sync or an evidence link.

use std::sync::Arc;

use et_core::ingest::imap::{
    self, Account, Auth, FASTMAIL, GMAIL, ICLOUD, ImapError, Preset, Security, YAHOO,
};
use et_core::ingest::outlook::{self, OAuthError};
use et_core::source::{self, ImapConfig};
use et_core::store::{Store, StoreError};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri_plugin_opener::OpenerExt;

use crate::AppState;

/// Where Microsoft's sign-in answers from; S16 names it when it refuses.
const MICROSOFT: &str = "login.microsoftonline.com";

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
        auth: Auth::Password,
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
        source::password_secret,
        password.as_bytes(),
    )
    .await
}

/// Opens Microsoft's sign-in in the browser, waits for it to come back,
/// checks the token against IMAP, then saves the source and its refresh
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

    let account = outlook_account(email.clone());
    imap::check_sign_in(&account, &tokens.access)
        .await
        .map_err(|err| match err {
            ImapError::Auth(server_says) => ConnectError::SignInRefused {
                host: outlook::HOST.into(),
                server_says,
                guide: None,
            },
            ImapError::Connect { .. } | ImapError::Tls(_) => ConnectError::Unreachable {
                message: err.to_string(),
            },
            other => failed(other),
        })?;

    let config = ImapConfig {
        host: account.host,
        port: account.port,
        username: email.clone(),
    };
    save(
        &state,
        format!("Outlook · {email}"),
        config,
        source::add_outlook,
        source::refresh_secret,
        refresh.as_bytes(),
    )
    .await
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
        OAuthError::Http(_) => ConnectError::Unreachable {
            message: err.to_string(),
        },
        other => failed(other),
    }
}

fn outlook_account(username: String) -> Account {
    Account {
        host: outlook::HOST.into(),
        port: outlook::PORT,
        username,
        security: Security::Tls,
        auth: Auth::XOAuth2,
    }
}

/// Saves a checked source, then puts its secret in the secret store.
async fn save(
    state: &AppState,
    label: String,
    config: ImapConfig,
    add: fn(&Store, &str, &ImapConfig) -> Result<i64, StoreError>,
    secret_name: fn(i64) -> String,
    secret: &[u8],
) -> Result<SourceSummary, ConnectError> {
    let store = state.store().map_err(failed)?;
    let (id, store) = tauri::async_runtime::spawn_blocking(move || {
        add(&store, &label, &config).map(|id| (id, store))
    })
    .await
    .map_err(failed)?
    .map_err(failed)?;
    state
        .secrets
        .set(&secret_name(id), secret)
        .map_err(failed)?;

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

/// The account and secret a sync or an evidence link signs in with, or
/// `None` for a source that is not a mailbox. An Outlook source trades its
/// refresh token for an access token, and keeps the new refresh token
/// Microsoft may send with it.
pub(crate) async fn credentials(
    state: &AppState,
    store: &Arc<Store>,
    source_id: i64,
) -> Result<Option<(Account, String)>, SignInError> {
    let fail = |e: &dyn std::fmt::Display| SignInError::Failed(e.to_string());
    let Some(mailbox) = source::mailbox(store, source_id).map_err(|e| fail(&e))? else {
        return Ok(None);
    };
    let name = match mailbox.auth {
        Auth::Password => source::password_secret(source_id),
        Auth::XOAuth2 => source::refresh_secret(source_id),
    };
    let secret = state
        .secrets
        .get(&name)
        .map_err(|e| fail(&e))?
        .ok_or_else(|| SignInError::Failed("the sign-in is missing from the keychain".into()))?;
    let secret = String::from_utf8_lossy(&secret).into_owned();
    let account = Account {
        host: mailbox.config.host,
        port: mailbox.config.port,
        username: mailbox.config.username,
        security: Security::Tls,
        auth: mailbox.auth,
    };
    if mailbox.auth == Auth::Password {
        return Ok(Some((account, secret)));
    }
    let tokens = outlook::refresh(&secret).await.map_err(|err| match err {
        OAuthError::Refused(server_says) => SignInError::Refused {
            host: MICROSOFT.into(),
            server_says,
        },
        OAuthError::Http(_) => SignInError::Unreachable(err.to_string()),
        other => fail(&other),
    })?;
    if let Some(refresh) = &tokens.refresh {
        state
            .secrets
            .set(&name, refresh.as_bytes())
            .map_err(|e| fail(&e))?;
    }
    Ok(Some((account, tokens.access.to_string())))
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
