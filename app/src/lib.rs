//! The Tauri command layer over `et-core` (PLAN.md 1.2).

mod scan;
mod sources;

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use et_core::crypt::{SecretStore, load_or_create_key, native_secret_store};
use et_core::store::{Store, StoreError};
use serde::Serialize;
use specta::Type;
use tauri::Manager;

/// What S17 and S16 show about local data at rest.
#[derive(Clone, Serialize, Type)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum StoreStatus {
    #[serde(rename_all = "camelCase")]
    Open {
        key_backend: String,
        cipher_version: String,
    },
    /// The key no longer opens the database (S16, "database key missing").
    Locked,
    Failed {
        message: String,
    },
}

pub(crate) struct AppState {
    status: StoreStatus,
    store: Option<Arc<Store>>,
    secrets: Arc<dyn SecretStore>,
    /// The cancel flag of the running scan; one scan at a time.
    scan: Mutex<Option<Arc<AtomicBool>>>,
}

impl AppState {
    fn store(&self) -> Result<Arc<Store>, String> {
        self.store
            .clone()
            .ok_or_else(|| "local data is not open".to_owned())
    }
}

#[tauri::command]
#[specta::specta]
fn store_status(state: tauri::State<'_, AppState>) -> StoreStatus {
    state.status.clone()
}

pub fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new().commands(tauri_specta::collect_commands![
        store_status,
        sources::imap_presets,
        sources::add_imap_source,
        sources::list_sources,
        scan::start_scan,
        scan::cancel_scan,
    ])
}

fn data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|dir| dir.join("EmailTerminator"))
}

fn open_store(secrets: &dyn SecretStore, dir: Option<&PathBuf>) -> (StoreStatus, Option<Store>) {
    let failed = |message: String| (StoreStatus::Failed { message }, None);
    let Some(dir) = dir else {
        return failed("no data directory on this system".into());
    };
    if let Err(err) = std::fs::create_dir_all(dir) {
        return failed(format!("{}: {err}", dir.display()));
    }
    let key = match load_or_create_key(secrets) {
        Ok(key) => key,
        Err(err) => return failed(err.to_string()),
    };
    match Store::open(&dir.join("emailterminator.db"), &key) {
        Ok(store) => match store.cipher_version() {
            Ok(cipher_version) => (
                StoreStatus::Open {
                    key_backend: secrets.describe().into(),
                    cipher_version,
                },
                Some(store),
            ),
            Err(err) => failed(err.to_string()),
        },
        Err(StoreError::Locked) => (StoreStatus::Locked, None),
        Err(err) => failed(err.to_string()),
    }
}

pub fn run() {
    let builder = specta_builder();
    let dir = data_dir();
    let secrets: Arc<dyn SecretStore> = match &dir {
        Some(dir) => Arc::from(native_secret_store(dir)),
        None => Arc::new(et_core::crypt::Keychain),
    };
    let (status, store) = open_store(secrets.as_ref(), dir.as_ref());

    tauri::Builder::default()
        // Must be the first plugin: a second launch focuses this window and exits.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            status,
            store: store.map(Arc::new),
            secrets,
            scan: Mutex::new(None),
        })
        .invoke_handler(builder.invoke_handler())
        .run(tauri::generate_context!())
        .expect("the Tauri runtime failed to start");
}

#[cfg(test)]
mod tests {
    /// Writes the committed UI bindings; CI fails when they drift (PLAN.md 2.5).
    #[test]
    fn export_bindings() {
        super::specta_builder()
            .export(
                specta_typescript::Typescript::default().header(
                    "// Regenerate with `cargo test -p emailterminator export_bindings`.\n",
                ),
                concat!(env!("CARGO_MANIFEST_DIR"), "/../ui/src/lib/bindings.ts"),
            )
            .expect("bindings export failed");
    }
}
