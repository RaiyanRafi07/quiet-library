use tauri::{State, async_runtime::spawn_blocking};
use crate::{AppState, util::tantivy_index};
use std::fs;

#[tauri::command]
pub async fn reindex_all(state: State<'_, AppState>) -> Result<(), String> {
    let state_clone = AppState { app_dir: state.app_dir.clone(), index: std::sync::Mutex::new(None), reader: std::sync::Mutex::new(None) };
    spawn_blocking(move || tantivy_index::rebuild_index(&state_clone))
        .await
        .map_err(|e| format!("join error: {:?}", e))?
        ;
    // After rebuild, drop cached handles so next search opens new index
    tantivy_index::drop_cached_index(&state);
    Ok(())
}

#[tauri::command]
pub fn index_incremental() -> Result<(), String> {
    // TODO: Use file watcher notifications to update index
    Ok(())
}

#[tauri::command]
pub fn clear_extract_cache(state: State<AppState>) -> Result<(), String> {
    let sys_tmp = std::env::temp_dir().join("quietlibrary-cache");
    if sys_tmp.exists() { fs::remove_dir_all(&sys_tmp).map_err(|e| e.to_string())?; }
    let app_cache = state.app_dir.join("cache");
    if app_cache.exists() { fs::remove_dir_all(&app_cache).map_err(|e| e.to_string())?; }
    Ok(())
}
