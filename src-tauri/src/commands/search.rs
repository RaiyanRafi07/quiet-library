use std::{fs, path::{Path, PathBuf}};

use tauri::State;

use crate::{
    commands::library,
    models::SearchResult,
    AppState,
};
use crate::util::tantivy_index;
use crate::util::{
    extract_text::{extract_title_and_text, is_supported_text},
    extract_pdf::extract_pdf_pages_cached,
    snippet::make_snippets,
};

#[tauri::command]
pub fn search(query: String, limit: u32, state: State<AppState>) -> Result<Vec<SearchResult>, String> {
    let q = query.trim();
    if q.is_empty() { return Ok(vec![]); }
    // Prefer the Tantivy index for speed if present.
    if let Ok(results) = tantivy_index::search_index(&state, q, limit as usize) {
        if !results.is_empty() { return Ok(results); }
    }

    let folders = library::watched_folders(&state);
    let mut results: Vec<SearchResult> = Vec::new();

    for folder in folders {
        let path = PathBuf::from(&folder);
        scan_folder(&path, q, limit, &mut results)?;
        if results.len() as u32 >= limit { break; }
    }

    // sort by score desc, then by path
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal).then(a.path.cmp(&b.path)));
    if results.len() as u32 > limit { results.truncate(limit as usize); }
    Ok(results)
}

fn scan_folder(dir: &Path, q: &str, limit: u32, out: &mut Vec<SearchResult>) -> Result<(), String> {
    if !dir.exists() { return Ok(()); }
    let entries = match fs::read_dir(dir) { Ok(e) => e, Err(_) => return Ok(()) };
    for entry in entries {
        let entry = match entry { Ok(e) => e, Err(_) => continue };
        let path = entry.path();
        if path.is_dir() {
            scan_folder(&path, q, limit, out)?;
            if out.len() as u32 >= limit { return Ok(()); }
            continue;
        }
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
        if is_supported_text(&path) {
            match extract_title_and_text(&path) {
                Ok((title, text)) => push_text_results(&path, q, &title, &text, out),
                Err(_) => {}
            }
        } else if ext == "pdf" {
            // Use per-user temp dir for cache to avoid passing state. Key includes mtime+size so safe enough.
            let sys_tmp = std::env::temp_dir().join("quietlibrary-cache");
            match extract_pdf_pages_cached(&path, &sys_tmp, 50) {
                Ok((title, pages, which)) => {
                    for (page, text) in &pages {
                        push_page_results(&path, q, &title, *page, &text, Some(&which), out);
                        if out.len() as u32 >= limit { return Ok(()); }
                    }
                    println!("quietlibrary: extractor={} file={} ({} pages)", which, path.to_string_lossy(), pages.len());
                }
                Err(_) => {
                    // fallback to filename match
                    let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                    let lf = filename.to_lowercase();
                    let lq = q.to_lowercase();
                    if lf.contains(&lq) {
                        out.push(SearchResult { title: filename.to_string(), path: path.to_string_lossy().to_string(), page: None, section: None, snippet: String::new(), score: 0.05 });
                    }
                }
            }
        } else if ext == "epub" {
            // Keep EPUB as filename-only for now
            let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            let lf = filename.to_lowercase();
            let lq = q.to_lowercase();
            if lf.contains(&lq) {
                out.push(SearchResult {
                    title: filename.to_string(),
                    path: path.to_string_lossy().to_string(),
                    page: None,
                    section: None,
                    snippet: String::new(),
                    score: 0.05,
                });
            }
        } else {
            // unsupported type
        }
        if out.len() as u32 >= limit { return Ok(()); }
    }
    Ok(())
}

fn push_text_results(path: &Path, q: &str, title: &str, text: &str, out: &mut Vec<SearchResult>) {
    let snippets = make_snippets(text, q, 400);
    for snippet in snippets {
        out.push(SearchResult {
            title: title.to_string(),
            path: path.to_string_lossy().to_string(),
            page: None,
            section: None,
            snippet,
            score: 1.0,
        });
    }
}

fn push_page_results(path: &Path, q: &str, title: &str, page: u32, text: &str, extractor: Option<&str>, out: &mut Vec<SearchResult>) {
    let mut snippets = make_snippets(text, q, 400);
    if let Some(which) = extractor { for s in &mut snippets { s.push_str(&format!(" \u{00B7} [{}]", which)); } }
    for snippet in snippets {
        out.push(SearchResult {
            title: title.to_string(),
            path: path.to_string_lossy().to_string(),
            page: Some(page),
            section: None,
            snippet,
            score: 1.1,
        });
    }
}
