use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::AppState;

const LIBRARY_FILE: &str = "library.json";

#[derive(Default, Serialize, Deserialize)]
pub(crate) struct LibraryData {
    folders: Vec<String>,
}

fn lib_path(state: &AppState) -> PathBuf {
    state.app_dir.join(LIBRARY_FILE)
}

pub(crate) fn read_library(state: &AppState) -> LibraryData {
    let p = lib_path(state);
    if let Ok(bytes) = fs::read(&p) {
        serde_json::from_slice(&bytes).unwrap_or_default()
    } else {
        LibraryData::default()
    }
}

fn write_library(state: &AppState, data: &LibraryData) -> Result<(), String> {
    let p = lib_path(state);
    fs::create_dir_all(&state.app_dir).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec_pretty(data).map_err(|e| e.to_string())?;
    fs::write(p, bytes).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_watched_folder(path: String, state: State<AppState>) -> Result<(), String> {
    let mut data = read_library(&state);
    if !data.folders.iter().any(|p| p == &path) {
        data.folders.push(path);
        write_library(&state, &data)?;
    }
    Ok(())
}

#[tauri::command]
pub fn list_watched_folders(state: State<AppState>) -> Result<Vec<String>, String> {
    let data = read_library(&state);
    Ok(data.folders)
}

// Internal helper for other commands (non-IPC) to access folders without exposing serde types
pub(crate) fn watched_folders(state: &AppState) -> Vec<String> {
    read_library(state).folders
}

#[tauri::command]
pub fn remove_watched_folder(path: String, state: State<AppState>) -> Result<(), String> {
    let mut data = read_library(&state);
    data.folders.retain(|p| p != &path);
    write_library(&state, &data)
}
