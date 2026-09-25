//! S01's IMAP path and S12's list: connect a mailbox, list what is connected.

use et_core::ingest::imap::{
    self, Account, FASTMAIL, GMAIL, ICLOUD, ImapError, Preset, Security, YAHOO,
};
use et_core::source::{self, ImapConfig};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::AppState;

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
    Failed {
        message: String,
    },
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
    let failed = |message: String| ConnectError::Failed { message };
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
            other => failed(other.to_string()),
        })?;

    let store = state.store().map_err(failed)?;
    let config = ImapConfig {
        host,
        port,
        username: email.clone(),
    };
    let label = format!("{label} · {email}");
    let id = tauri::async_runtime::spawn_blocking(move || {
        source::add_imap(&store, &label, &config).map(|id| (id, store))
    })
    .await
    .map_err(|e| failed(e.to_string()))?
    .map_err(|e| failed(e.to_string()))?;
    let (id, store) = id;
    state
        .secrets
        .set(&source::password_secret(id), password.as_bytes())
        .map_err(|e| failed(e.to_string()))?;

    source::list(&store)
        .map_err(|e| failed(e.to_string()))?
        .into_iter()
        .find(|s| s.id == id)
        .map(SourceSummary::from)
        .ok_or_else(|| failed("the new source was not saved".into()))
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
