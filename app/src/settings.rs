//! S17's M2 and M3 parts (appearance, sweep behaviour, data location,
//! encryption, erase) and the environment S15 attaches to an issue.

use et_core::crypt::{Keychain, SecretStore, load_or_create_key};
use et_core::local::{self, MoveError};
use et_core::setting;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

use crate::AppState;

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Appearance {
    Light,
    Dark,
    System,
}

const APPEARANCE: &str = "appearance";

/// `system` until the user picks one, and whenever the data cannot open.
#[tauri::command]
#[specta::specta]
pub fn appearance(state: tauri::State<'_, AppState>) -> Appearance {
    state
        .store()
        .ok()
        .and_then(|store| setting::get(&store, APPEARANCE).ok().flatten())
        .and_then(|value| serde_json::from_value(serde_json::Value::String(value)).ok())
        .unwrap_or(Appearance::System)
}

#[tauri::command]
#[specta::specta]
pub fn set_appearance(
    state: tauri::State<'_, AppState>,
    appearance: Appearance,
) -> Result<(), String> {
    let value = match appearance {
        Appearance::Light => "light",
        Appearance::Dark => "dark",
        Appearance::System => "system",
    };
    setting::set(&*state.store()?, APPEARANCE, value).map_err(|e| e.to_string())
}

/// S17's sweep behaviour: M3's defaults, made editable (PLAN.md M3).
#[derive(Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Sweep {
    /// Show S11 before every sweep. When off, a sweep of fewer than
    /// `confirm_from` items runs straight away.
    pub confirm_always: bool,
    pub confirm_from: u32,
    /// Critical services start excluded from a sweep.
    pub exclude_critical: bool,
}

impl Default for Sweep {
    fn default() -> Self {
        Self {
            confirm_always: true,
            confirm_from: 10,
            exclude_critical: true,
        }
    }
}

const SWEEP: &str = "sweep";

pub(crate) fn read_sweep(state: &AppState) -> Sweep {
    state
        .store()
        .ok()
        .and_then(|store| setting::get(&store, SWEEP).ok().flatten())
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default()
}

#[tauri::command]
#[specta::specta]
pub fn sweep_settings(state: tauri::State<'_, AppState>) -> Sweep {
    read_sweep(&state)
}

#[tauri::command]
#[specta::specta]
pub fn set_sweep_settings(state: tauri::State<'_, AppState>, sweep: Sweep) -> Result<(), String> {
    let sweep = Sweep {
        confirm_from: sweep.confirm_from.max(2),
        ..sweep
    };
    let value = serde_json::to_string(&sweep).map_err(|e| e.to_string())?;
    setting::set(&*state.store()?, SWEEP, &value).map_err(|e| e.to_string())
}

/// Where the key that encrypts the local data is kept (PLAN.md 2.1, 2.2).
#[derive(Clone, Copy, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum KeyPlace {
    Keychain,
    /// The Linux fallback: a key file beside the data.
    FileBeside,
}

#[derive(Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DataLocation {
    path: String,
    bytes: f64,
    key_place: KeyPlace,
}

#[tauri::command]
#[specta::specta]
pub fn data_location(state: tauri::State<'_, AppState>) -> Result<DataLocation, String> {
    let dir = state
        .dir
        .as_ref()
        .ok_or("no data directory on this system")?;
    Ok(DataLocation {
        path: dir.to_string_lossy().into_owned(),
        bytes: local::size(dir) as f64,
        key_place: if state.secrets.describe() == Keychain.describe() {
            KeyPlace::Keychain
        } else {
            KeyPlace::FileBeside
        },
    })
}

#[derive(Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MoveOutcome {
    /// The user closed the folder picker.
    Cancelled,
    Refused {
        message: String,
    },
}

/// Asks for a folder, copies the data there and checks it, then restarts so
/// the copy is the one in use. Returns only when nothing moved.
#[tauri::command]
#[specta::specta]
pub async fn move_data_location(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<MoveOutcome, String> {
    let refused = |message: String| Ok(MoveOutcome::Refused { message });
    if state.scan_running() {
        return refused("Wait for the scan to finish, then move the data.".into());
    }
    let (Some(from), Some(default)) = (state.dir.clone(), state.default_dir.clone()) else {
        return Err("no data directory on this system".into());
    };
    let store = state.store()?;
    let key = load_or_create_key(state.secrets.as_ref()).map_err(|e| e.to_string())?;

    let picker = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        picker
            .dialog()
            .file()
            .set_title("Choose where to keep EmailTerminator's data")
            .blocking_pick_folder()
    })
    .await
    .map_err(|e| e.to_string())?;
    let Some(parent) = picked.and_then(|p| p.into_path().ok()) else {
        return Ok(MoveOutcome::Cancelled);
    };

    let moved = tauri::async_runtime::spawn_blocking(move || {
        local::move_to(&store, &key, &from, &default, &parent)
    })
    .await
    .map_err(|e| e.to_string())?;
    match moved {
        Ok(_) => app.restart(),
        Err(err @ (MoveError::Same | MoveError::NotEmpty(_) | MoveError::Mismatch)) => {
            refused(err.to_string())
        }
        Err(err) => Err(err.to_string()),
    }
}

/// Erases the local data and restarts into a fresh install. The mailbox,
/// and the licence once there is one, are untouched.
#[tauri::command]
#[specta::specta]
pub fn erase_local_data(app: AppHandle, state: tauri::State<'_, AppState>) -> Result<(), String> {
    if state.scan_running() {
        return Err("Wait for the scan to finish, then erase.".into());
    }
    let dir = state
        .dir
        .as_ref()
        .ok_or("no data directory on this system")?;
    local::request_erase(dir).map_err(|e| e.to_string())?;
    app.restart()
}

/// What S15 attaches to an issue. Nothing here identifies the user.
#[derive(Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: String,
    os: String,
    arch: String,
}

#[tauri::command]
#[specta::specta]
pub fn app_info(app: AppHandle) -> AppInfo {
    let name = match std::env::consts::OS {
        "macos" => "macOS",
        "windows" => "Windows",
        "linux" => "Linux",
        other => other,
    };
    AppInfo {
        version: app.package_info().version.to_string(),
        os: match os_version() {
            Some(version) => format!("{name} {version}"),
            None => name.to_owned(),
        },
        arch: std::env::consts::ARCH.to_owned(),
    }
}

fn os_version() -> Option<String> {
    let output = |program: &str, args: &[&str]| {
        std::process::Command::new(program)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
    };
    match std::env::consts::OS {
        "macos" => output("sw_vers", &["-productVersion"]),
        // "Microsoft Windows [Version 10.0.22631.4037]"
        "windows" => output("cmd", &["/c", "ver"]).and_then(|v| {
            let start = v.find("Version ")? + "Version ".len();
            Some(v[start..].trim_end_matches(']').to_owned())
        }),
        "linux" => std::fs::read_to_string("/etc/os-release")
            .ok()
            .and_then(|s| {
                s.lines()
                    .find_map(|l| l.strip_prefix("PRETTY_NAME="))
                    .map(|v| v.trim_matches('"').to_owned())
            }),
        _ => None,
    }
    .filter(|v| !v.is_empty())
}
