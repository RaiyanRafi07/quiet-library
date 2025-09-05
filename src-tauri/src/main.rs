#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

mod commands;
mod models;
mod util;

use std::path::PathBuf;
use std::sync::Mutex;
use tantivy::{Index, IndexReader};

use tauri::{Manager};

pub struct AppState {
    pub app_dir: PathBuf,
    pub index: Mutex<Option<Index>>,      // lazily opened
    pub reader: Mutex<Option<IndexReader>>, // lazily opened
}

fn resolve_app_dir(app: &tauri::AppHandle) -> PathBuf {
    // Use Tauri's resolver to get per-app data directory
    app.path_resolver().app_data_dir().unwrap_or_else(|| {
        // Fallback to executable directory if resolver fails
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    })
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let app_dir = resolve_app_dir(&app.app_handle());
            std::fs::create_dir_all(&app_dir).ok();
            app.manage(AppState { app_dir, index: Mutex::new(None), reader: Mutex::new(None) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::library::add_watched_folder,
            commands::library::list_watched_folders,
            commands::library::remove_watched_folder,
            commands::indexer::reindex_all,
            commands::indexer::clear_extract_cache,
            commands::search::search,
            commands::bookmarks::add_bookmark,
            commands::bookmarks::list_bookmarks,
            commands::bookmarks::remove_bookmark,
            commands::open::reveal_in_os,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
