//! S03–S06 and S08: the read paths over a scanned mailbox.

use std::sync::Arc;

use et_core::source::unix_now;
use et_core::store::{Store, StoreError};
use et_core::view::{self, Dashboard, Newsletter, PlaybookView, ServiceDetail, Subscription};

use crate::AppState;

/// Runs a query off the main thread, as every database call does (PLAN.md 2.1).
async fn read<T: Send + 'static>(
    state: &AppState,
    query: impl FnOnce(&Store) -> Result<T, StoreError> + Send + 'static,
) -> Result<T, String> {
    let store: Arc<Store> = state.store()?;
    tauri::async_runtime::spawn_blocking(move || query(&store))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn dashboard(state: tauri::State<'_, AppState>) -> Result<Dashboard, String> {
    read(&state, |store| view::dashboard(store, unix_now())).await
}

#[tauri::command]
#[specta::specta]
pub async fn subscriptions(state: tauri::State<'_, AppState>) -> Result<Vec<Subscription>, String> {
    read(&state, |store| view::subscriptions(store, unix_now())).await
}

#[tauri::command]
#[specta::specta]
pub async fn newsletters(state: tauri::State<'_, AppState>) -> Result<Vec<Newsletter>, String> {
    read(&state, |store| view::newsletters(store, unix_now())).await
}

#[tauri::command]
#[specta::specta]
pub async fn service_detail(
    state: tauri::State<'_, AppState>,
    id: u32,
) -> Result<Option<ServiceDetail>, String> {
    read(&state, move |store| {
        view::service_detail(store, id, unix_now())
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn playbook(
    state: tauri::State<'_, AppState>,
    id: u32,
) -> Result<Option<PlaybookView>, String> {
    read(&state, move |store| view::playbook(store, id, unix_now())).await
}
