use std::{
    fs,
    hash::{Hash, Hasher},
    path::Path,
};

use lopdf::{content::Content, Document, Object, ObjectId};
use serde::{Deserialize, Serialize};
use once_cell::sync::Lazy;
use std::sync::Mutex;

// Prefer pdfium-render for accurate Unicode extraction; fall back to lopdf if binding fails
// or extraction encounters an error.
pub fn extract_pdf_pages(path: &Path) -> Result<(String, Vec<(u32, String)>, String), String> {
    match extract_with_pdfium(path) {
        Ok((title, pages)) => Ok((title, pages, "pdfium".to_string())),
        Err(_) => extract_with_lopdf(path).map(|(t, p)| (t, p, "lopdf".to_string())),
    }
}

static PDFIUM_GUARD: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

fn extract_with_pdfium(path: &Path) -> Result<(String, Vec<(u32, String)>), String> {
    use pdfium_render::prelude::*;
    let _guard = PDFIUM_GUARD.lock().unwrap();
    // Try to bind to a PDFium DLL placed next to the exe or provided via PDFIUM_PATH;
    // fall back to a system-installed library.
    let mut maybe = None;
    for lib_path in candidate_pdfium_paths() {
        if lib_path.exists() {
            if let Ok(bind) = Pdfium::bind_to_library(&lib_path) { maybe = Some(bind); break; }
        }
    }
    let bindings = match maybe {
        Some(b) => b,
        None => Pdfium::bind_to_system_library().map_err(|e| format!("pdfium bind failed: {}", e))?,
    };
    let pdfium = Pdfium::new(bindings);

    let doc = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|e| format!("load failed: {}", e))?;

    // Some versions of pdfium-render expose limited metadata APIs; keep it simple.
    let title = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();

    let pages = doc.pages();
    let page_count = pages.len() as usize;
    let mut out: Vec<(u32, String)> = Vec::with_capacity(page_count);
    for i in 0..page_count {
        if let Ok(page) = pages.get(i as u16) {
            let text = page
                .text()
                .and_then(|t| Ok(t.all()))
                .unwrap_or_default();
            let norm = normalize_ws_preserve_newlines(&sanitize_text(&text));
            if !norm.is_empty() {
                out.push((((i as u32) + 1), norm));
            }
        }
    }
    Ok((title, out))
}

#[cfg(target_os = "windows")]
fn candidate_pdfium_paths() -> Vec<std::path::PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(p) = std::env::var("PDFIUM_PATH") { candidates.push(std::path::PathBuf::from(p)); }
    if let Ok(exe) = std::env::current_exe() { if let Some(dir) = exe.parent() { 
        candidates.push(dir.join("pdfium.dll"));
        // Also check parent of the exe dir (e.g., target/ vs target/debug/)
        if let Some(parent) = dir.parent() { candidates.push(parent.join("pdfium.dll")); }
        candidates.push(dir.join("pdfium").join("pdfium.dll"));
        candidates.push(dir.join("resources").join("pdfium.dll"));
        candidates.push(dir.join("resources").join("pdfium").join("pdfium.dll"));
        // Walk upwards a couple of levels to reach project root and check src-tauri/resources
        if let Some(parent) = dir.parent() {
            candidates.push(parent.join("src-tauri").join("resources").join("pdfium.dll"));
            candidates.push(parent.join("src-tauri").join("resources").join("pdfium").join("pdfium.dll"));
            if let Some(parent2) = parent.parent() {
                candidates.push(parent2.join("src-tauri").join("resources").join("pdfium.dll"));
                candidates.push(parent2.join("src-tauri").join("resources").join("pdfium").join("pdfium.dll"));
            }
        }
    } }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("pdfium.dll"));
        candidates.push(cwd.join("resources").join("pdfium.dll"));
        candidates.push(cwd.join("resources").join("pdfium").join("pdfium.dll"));
        candidates.push(cwd.join("src-tauri").join("resources").join("pdfium.dll"));
        candidates.push(cwd.join("src-tauri").join("resources").join("pdfium").join("pdfium.dll"));
    }
    // Also check a conventional subfolder
    if let Ok(exe) = std::env::current_exe() { if let Some(dir) = exe.parent() { candidates.push(dir.join("pdfium").join("pdfium.dll")); } }
    candidates
}

#[cfg(not(target_os = "windows"))]
fn candidate_pdfium_paths() -> Vec<std::path::PathBuf> { Vec::new() }

// Previous lopdf-based best-effort extraction retained as fallback
fn extract_with_lopdf(path: &Path) -> Result<(String, Vec<(u32, String)>), String> {
    let doc = Document::load(path).map_err(|e| e.to_string())?;
    let title = doc
        .trailer
        .get(b"Info")
        .ok()
        .and_then(|id| match id {
            Object::Reference(oid) => doc.get_dictionary(*oid).ok(),
            _ => None,
        })
        .and_then(|dict| dict.get(b"Title").ok())
        .and_then(|obj| obj.as_str().ok())
        .map(|s| String::from_utf8_lossy(s).to_string())
        .unwrap_or_else(|| path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string());

    let mut pages_text: Vec<(u32, String)> = Vec::new();
    let pages = doc.get_pages(); // BTreeMap<u32, ObjectId>
    for (page_num, page_id) in pages {
        let text = extract_page_text(&doc, page_id);
        if !text.trim().is_empty() {
            pages_text.push((page_num, text));
        }
    }
    Ok((title, pages_text))
}

#[derive(Serialize, Deserialize)]
struct PdfCacheFile {
    title: String,
    pages: Vec<(u32, String)>,
    mtime_secs: u64,
    size: u64,
    which: Option<String>,
}

fn file_fingerprint(path: &Path) -> Result<(u64, u64), String> {
    let meta = fs::metadata(path).map_err(|e| e.to_string())?;
    let size = meta.len();
    let mtime = meta.modified().ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Ok((mtime, size))
}

fn cache_key(path: &Path, mtime: u64, size: u64) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.to_string_lossy().hash(&mut hasher);
    mtime.hash(&mut hasher);
    size.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

pub fn extract_pdf_pages_cached(
    path: &Path,
    cache_dir: &Path,
    max_pages: u32,
) -> Result<(String, Vec<(u32, String)>, String), String> {
    fs::create_dir_all(cache_dir).ok();
    let (mtime, size) = file_fingerprint(path)?;
    let key = cache_key(path, mtime, size);
    let cache_path = cache_dir.join(format!("pdf_{}.json", key));

    if let Ok(bytes) = fs::read(&cache_path) {
        if let Ok(mut cached) = serde_json::from_slice::<PdfCacheFile>(&bytes) {
            if cached.mtime_secs == mtime && cached.size == size {
                // If cache exists but was produced by a poorer extractor, try upgrading to Pdfium.
                let which = cached.which.clone().unwrap_or_else(|| "cache".to_string());
                if which != "pdfium" {
                    if let Ok((title_new, mut pages_new)) = extract_with_pdfium(path) {
                        if (pages_new.len() as u32) > max_pages { pages_new.truncate(max_pages as usize); }
                        let to_store = PdfCacheFile { title: title_new.clone(), pages: pages_new.clone(), mtime_secs: mtime, size, which: Some("pdfium".to_string()) };
                        if let Ok(bytes) = serde_json::to_vec(&to_store) { let _ = fs::write(&cache_path, bytes); }
                        return Ok((title_new, pages_new, "pdfium".to_string()));
                    }
                }
                if (cached.pages.len() as u32) > max_pages { cached.pages.truncate(max_pages as usize); }
                return Ok((cached.title, cached.pages, which));
            }
        }
    }

    let (title, mut pages, which) = extract_pdf_pages(path)?;
    if (pages.len() as u32) > max_pages { pages.truncate(max_pages as usize); }
    let to_store = PdfCacheFile { title: title.clone(), pages: pages.clone(), mtime_secs: mtime, size, which: Some(which.clone()) };
    if let Ok(bytes) = serde_json::to_vec(&to_store) { let _ = fs::write(&cache_path, bytes); }
    Ok((title, pages, which))
}

fn extract_page_text(doc: &Document, page_id: ObjectId) -> String {
    // Concatenate all content streams for the page and decode operations
    let content_data = match doc.get_page_content(page_id) { Ok(d) => d, Err(_) => return String::new() };
    let content = match Content::decode(&content_data) { Ok(c) => c, Err(_) => return String::new() };
    let mut out = String::new();
    for op in content.operations {
        match op.operator.as_str() {
            // Tj: show text
            "Tj" => {
                if let Some(Object::String(bytes, _)) = op.operands.get(0) {
                    out.push_str(&bytes_to_text(bytes));
                    out.push(' ');
                }
            }
            // TJ: array of strings and spacing adjustments
            "TJ" => {
                if let Some(Object::Array(items)) = op.operands.get(0) {
                    for item in items {
                        if let Object::String(bytes, _) = item { out.push_str(&bytes_to_text(bytes)); }
                    }
                    out.push(' ');
                }
            }
            // ' and " are shorthand for moving to next line and showing text
            "'" | "\"" => {
                if let Some(Object::String(bytes, _)) = op.operands.get(0) {
                    out.push('\n');
                    out.push_str(&bytes_to_text(bytes));
                    out.push(' ');
                }
            }
            // newline-ish operators to help spacing a bit
            "T*" => out.push('\n'),
            _ => {}
        }
    }
    normalize_ws_preserve_newlines(&sanitize_text(&out))
}

fn bytes_to_text(bytes: &Vec<u8>) -> String {
    // Try UTF-8; fall back to PDFDocEncoding-ish by lossy conversion
    String::from_utf8(bytes.clone()).unwrap_or_else(|_| String::from_utf8_lossy(bytes).to_string())
}

fn normalize_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_space = false;
    for ch in s.chars() {
        let is_space = ch.is_whitespace();
        if is_space {
            if !last_space { out.push(' '); }
        } else {
            out.push(ch);
        }
        last_space = is_space;
    }
    out.trim().to_string()
}

// Remove control/formatting/invisible characters that often appear in PDF text extraction
// and render as empty boxes in UI fonts (eg U+FFFD replacement, zero-width spaces).
fn sanitize_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        let cp = ch as u32;
        let drop = ch == '\u{FFFD}'
            || (ch.is_control() && ch != '\n' && ch != '\r' && ch != '\t')
            || (0x200B..=0x200F).contains(&cp) // zero-width space/marks
            || (0x2028..=0x202F).contains(&cp) // various spaces/separators
            || (0x2060..=0x206F).contains(&cp) // word joiner etc
            || cp == 0xFEFF; // BOM / zero-width no-break
        if !drop { out.push(ch); }
    }
    out
}

// Like normalize_ws(), but preserves newlines so the UI can show multi-line context.
fn normalize_ws_preserve_newlines(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_space = false;
    let mut last_newline = false;
    for ch in s.chars() {
        match ch {
            '\r' => { /* drop CR; handle via LF */ }
            '\n' => {
                if !last_newline { out.push('\n'); }
                last_newline = true;
                last_space = false;
            }
            _ if ch.is_whitespace() => {
                if !last_space { out.push(' '); }
                last_space = true;
                last_newline = false;
            }
            _ => {
                out.push(ch);
                last_space = false;
                last_newline = false;
            }
        }
    }
    // Trim spaces on each line and collapse multiple blank lines
    let mut cleaned = String::with_capacity(out.len());
    let mut prev_blank = false;
    for line in out.lines() {
        let t = line.trim();
        if t.is_empty() {
            if !prev_blank { cleaned.push('\n'); }
            prev_blank = true;
        } else {
            if !cleaned.is_empty() { cleaned.push('\n'); }
            cleaned.push_str(t);
            prev_blank = false;
        }
    }
    if cleaned.is_empty() { out.trim().to_string() } else { cleaned }
}
