//! The Tauri command layer over `et-core` (PLAN.md 1.2).

use std::path::PathBuf;
use std::sync::Mutex;

use et_core::crypt::{load_or_create_key, native_key_store};
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

struct AppState {
    status: StoreStatus,
    // The single writer behind a channel arrives with the schema in M1 (PLAN.md 2.1).
    _store: Mutex<Option<Store>>,
}

#[tauri::command]
#[specta::specta]
fn store_status(state: tauri::State<'_, AppState>) -> StoreStatus {
    state.status.clone()
}

pub fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .commands(tauri_specta::collect_commands![store_status])
}

fn data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|dir| dir.join("EmailTerminator"))
}

fn open_store() -> (StoreStatus, Option<Store>) {
    let failed = |message: String| (StoreStatus::Failed { message }, None);
    let Some(dir) = data_dir() else {
        return failed("no data directory on this system".into());
    };
    if let Err(err) = std::fs::create_dir_all(&dir) {
        return failed(format!("{}: {err}", dir.display()));
    }
    let key_store = match native_key_store(&dir) {
        Ok(store) => store,
        Err(err) => return failed(err.to_string()),
    };
    let key = match load_or_create_key(key_store.as_ref()) {
        Ok(key) => key,
        Err(err) => return failed(err.to_string()),
    };
    match Store::open(&dir.join("emailterminator.db"), &key) {
        Ok(store) => match store.cipher_version() {
            Ok(cipher_version) => (
                StoreStatus::Open {
                    key_backend: key_store.describe().into(),
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
    let (status, store) = open_store();

    tauri::Builder::default()
        // Must be the first plugin: a second launch focuses this window and exits.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .manage(AppState {
            status,
            _store: Mutex::new(store),
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
