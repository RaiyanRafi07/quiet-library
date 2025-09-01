use std::{fs, path::PathBuf};

use tauri::State;
use uuid::Uuid;

use crate::{models::Bookmark, AppState};

const BOOKMARKS_FILE: &str = "bookmarks.json";

fn path(state: &AppState) -> PathBuf { state.app_dir.join(BOOKMARKS_FILE) }

fn read_all(state: &AppState) -> Vec<Bookmark> {
    let p = path(state);
    if let Ok(bytes) = fs::read(&p) {
        serde_json::from_slice(&bytes).unwrap_or_default()
    } else { vec![] }
}

fn write_all(state: &AppState, list: &[Bookmark]) -> Result<(), String> {
    fs::create_dir_all(&state.app_dir).map_err(|e| e.to_string())?;
    let p = path(state);
    let bytes = serde_json::to_vec_pretty(list).map_err(|e| e.to_string())?;
    fs::write(p, bytes).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_bookmark(path: String, page: Option<u32>, section: Option<String>, note: Option<String>, state: State<AppState>) -> Result<(), String> {
    let mut all = read_all(&state);
    let id = Uuid::new_v4().to_string();
    let created_at = time::OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339).unwrap_or_else(|_| "".into());
    all.push(Bookmark { id, path, page, section, note, created_at });
    write_all(&state, &all)
}

#[tauri::command]
pub fn list_bookmarks(path: Option<String>, state: State<AppState>) -> Result<Vec<Bookmark>, String> {
    let mut all = read_all(&state);
    if let Some(p) = path { all.retain(|b| b.path == p); }
    Ok(all)
}

#[tauri::command]
pub fn remove_bookmark(id: String, state: State<AppState>) -> Result<(), String> {
    let mut all = read_all(&state);
    all.retain(|b| b.id != id);
    write_all(&state, &all)
}

