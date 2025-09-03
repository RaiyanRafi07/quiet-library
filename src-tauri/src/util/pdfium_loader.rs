use once_cell::sync::Lazy;
use pdfium_render::prelude::*;
use std::path::PathBuf;
use std::sync::Mutex;

static INIT_GUARD: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
pub static PDFIUM_SOURCE: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));
pub static PDFIUM_TRIED: Lazy<Mutex<Vec<String>>> = Lazy::new(|| Mutex::new(Vec::new()));

pub fn bind_pdfium() -> Result<Pdfium, String> {
    let _guard = INIT_GUARD.lock().unwrap();
    // Strategy: prefer app-bundled PDFium first, then PDFIUM_PATH, then system library.
    let mut candidates: Vec<PathBuf> = Vec::new();
    let mut preferred: Vec<PathBuf> = Vec::new();

    // 1) Likely locations relative to the executable and project for dev/builds
    if let Ok(cwd) = std::env::current_dir() {
        // Strongly prefer repo-bundled PDFium during dev
        preferred.push(cwd.join("src-tauri").join("resources").join("pdfium"));
        preferred.push(cwd.join("src-tauri").join("resources"));
        preferred.push(cwd.join("resources").join("pdfium"));
        preferred.push(cwd.join("resources"));
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.to_path_buf());
            if let Some(p) = dir.parent() {
                candidates.push(p.to_path_buf());
                if let Some(pp) = p.parent() {
                    candidates.push(pp.to_path_buf());
                }
            }
            candidates.push(dir.join("resources"));
            candidates.push(dir.join("resources").join("pdfium"));
            // Look up the tree for `src-tauri/resources[/pdfium]` (useful during `tauri dev`)
            for anc in [
                dir,
                dir.parent().unwrap_or(dir),
                dir.parent().and_then(|x| x.parent()).unwrap_or(dir),
            ] {
                candidates.push(anc.join("src-tauri").join("resources"));
                candidates.push(anc.join("src-tauri").join("resources").join("pdfium"));
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        // Project root when running `tauri dev`
        candidates.push(cwd.clone());
        // Common resource locations in release builds
        candidates.push(cwd.join("resources"));
        candidates.push(cwd.join("resources").join("pdfium"));
        // Also check under `src-tauri/resources` during dev
        candidates.push(cwd.join("src-tauri").join("resources"));
        candidates.push(cwd.join("src-tauri").join("resources").join("pdfium"));
    }
    // Add common install locations for PDFium
    if cfg!(windows) {
        candidates.push(std::path::PathBuf::from("C:\\Program Files\\PDFium"));
        candidates.push(std::path::PathBuf::from("C:\\Program Files (x86)\\PDFium"));
    }
    if let Some(home) = tauri::api::path::home_dir() {
        candidates.push(home.join(".pdfium"));
    }

    // Try explicit env var next
    if let Ok(path) = std::env::var("PDFIUM_PATH") {
        let p = std::path::PathBuf::from(&path);
        if p.exists() {
            *PDFIUM_TRIED.lock().unwrap() =
                candidates.iter().map(|p| p.to_string_lossy().to_string()).collect();
            match Pdfium::bind_to_library(p.clone()).map(Pdfium::new) {
                Ok(pdf) => {
                    *PDFIUM_SOURCE.lock().unwrap() = Some(format!("env:{}", p.display()));
                    return Ok(pdf);
                }
                Err(e) => {
                    eprintln!("quietlibrary: failed binding PDFIUM_PATH at {}: {:?}", p.display(), e);
                }
            }
        }
    }

    // Try preferred repo resources first
    for dir in preferred.clone() {
        let p = Pdfium::pdfium_platform_library_name_at_path(&dir);
        if p.exists() {
            *PDFIUM_TRIED.lock().unwrap() = preferred.iter().chain(candidates.iter()).map(|p| p.to_string_lossy().to_string()).collect();
            match Pdfium::bind_to_library(p.clone()).map(Pdfium::new) {
                Ok(pdf) => { *PDFIUM_SOURCE.lock().unwrap() = Some(format!("bundled:{}", p.display())); return Ok(pdf); },
                Err(e2) => return Err(format!("bind pdfium from {:?}: {:?}", dir, e2)),
            }
        }
    }

    // Try all other candidate directories for a platform-appropriate library name
    let mut tried: Vec<std::path::PathBuf> = Vec::new();
    for dir in candidates.clone() {
        let p = Pdfium::pdfium_platform_library_name_at_path(&dir);
        if p.exists() {
            *PDFIUM_TRIED.lock().unwrap() = preferred.iter().chain(candidates.iter()).map(|p| p.to_string_lossy().to_string()).collect();
            match Pdfium::bind_to_library(p.clone()).map(Pdfium::new) {
                Ok(pdf) => { *PDFIUM_SOURCE.lock().unwrap() = Some(format!("bundled:{}", p.display())); return Ok(pdf); },
                Err(e2) => return Err(format!("bind pdfium from {:?}: {:?}", dir, e2)),
            }
        }
        tried.push(dir);
    }

    // Finally, fall back to system library
    match Pdfium::bind_to_system_library() {
        Ok(bindings) => { *PDFIUM_SOURCE.lock().unwrap() = Some("system".to_string()); Ok(Pdfium::new(bindings)) },
        Err(e) => {
            eprintln!("quietlibrary: pdfium not found; tried: {:?}", tried);
            Err(format!("bind pdfium: {:?}", e))
        }
    }
}