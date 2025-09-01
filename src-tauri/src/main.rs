#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

mod commands;
mod models;
mod util;

use std::path::PathBuf;

use tauri::{Manager};

pub struct AppState {
    pub app_dir: PathBuf,
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
            app.manage(AppState { app_dir });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::library::add_watched_folder,
            commands::library::list_watched_folders,
            commands::library::remove_watched_folder,
            commands::indexer::reindex_all,
            commands::indexer::index_incremental,
            commands::indexer::clear_extract_cache,
            commands::search::search,
            commands::open::resolve_open_target,
            commands::bookmarks::add_bookmark,
            commands::bookmarks::list_bookmarks,
            commands::bookmarks::remove_bookmark,
            commands::open::reveal_in_os,
            commands::open::pdf_page_count,
            commands::open::render_pdf_page,
            commands::open::pdf_find_matches,
            commands::open::warm_render_cache,
            commands::open::pdf_next_prev_page_with_match,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
