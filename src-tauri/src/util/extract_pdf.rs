use std::{
    fs,
    hash::{Hash, Hasher},
    path::Path,
};

use lopdf::{content::Content, Document, Object, ObjectId};
use serde::{Deserialize, Serialize};
use crate::util::pdfium_loader;
use once_cell::sync::Lazy;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH, Duration};

// Prefer pdfium-render for accurate Unicode extraction; fall back to lopdf if binding fails
// or extraction encounters an error.
pub fn extract_pdf_pages(path: &Path) -> Result<(String, Vec<(u32, String)>, String), String> {
    match extract_with_pdfium(path) {
        Ok((title, pages)) => Ok((title, pages, "pdfium".to_string())),
        Err(_) => extract_with_lopdf(path).map(|(t, p)| (t, p, "lopdf".to_string())),
    }
}

fn extract_with_pdfium(path: &Path) -> Result<(String, Vec<(u32, String)>), String> {
    let pdfium = pdfium_loader::bind_pdfium()?;

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
                .map(|t| t.all())
                .unwrap_or_default();
            let norm = normalize_ws_preserve_newlines(&sanitize_text(&text));
            if !norm.is_empty() {
                out.push((((i as u32) + 1), norm));
            }
        }
    }
    Ok((title, out))
}

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
    // Opportunistic LRU pruning of cache to keep its size bounded.
    maybe_prune_cache(cache_dir).ok();
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
    // Trim again after writing to enforce budget eagerly
    maybe_prune_cache(cache_dir).ok();
    Ok((title, pages, which))
}

// ---------------- Cache maintenance (LRU-ish) -----------------

const MAX_CACHE_BYTES: u64 = 300 * 1024 * 1024; // 300 MB cap
const MAX_CACHE_AGE_SECS: u64 = 30 * 24 * 60 * 60; // 30 days
const PRUNE_INTERVAL_SECS: u64 = 10 * 60; // run at most every 10 minutes

static LAST_PRUNE_SECS: Lazy<Mutex<u64>> = Lazy::new(|| Mutex::new(0));

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or(Duration::from_secs(0)).as_secs()
}

fn maybe_prune_cache(cache_dir: &Path) -> Result<(), String> {
    // Rate-limit to avoid heavy scans when many extracts happen in a row
    let now = now_secs();
    {
        let mut last = LAST_PRUNE_SECS.lock().map_err(|_| "prune lock".to_string())?;
        if now.saturating_sub(*last) < PRUNE_INTERVAL_SECS { return Ok(()); }
        *last = now;
    }
    prune_cache(cache_dir, MAX_CACHE_BYTES, MAX_CACHE_AGE_SECS)
}

fn prune_cache(cache_dir: &Path, max_bytes: u64, max_age_secs: u64) -> Result<(), String> {
    let mut entries: Vec<(std::path::PathBuf, u64, u64)> = Vec::new(); // (path, size, mtime)
    if !cache_dir.exists() { return Ok(()); }
    for e in fs::read_dir(cache_dir).map_err(|e| e.to_string())? {
        let e = match e { Ok(x) => x, Err(_) => continue };
        let p = e.path();
        if !p.is_file() { continue; }
        // only manage our pdf json cache files
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if !name.starts_with("pdf_") || !name.ends_with(".json") { continue; }
        if let Ok(meta) = e.metadata() {
            let size = meta.len();
            let mtime = meta.modified().ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs()).unwrap_or(0);
            entries.push((p, size, mtime));
        }
    }
    if entries.is_empty() { return Ok(()); }

    let mut total: u64 = entries.iter().map(|x| x.1).sum();
    let cutoff = now_secs().saturating_sub(max_age_secs);
    // Remove too-old files first
    for (p, size, mtime) in entries.iter() {
        if *mtime < cutoff {
            let _ = fs::remove_file(p);
            total = total.saturating_sub(*size);
        }
    }
    // Re-scan remaining entries (those not deleted may still be in entries; filter)
    let mut keep: Vec<(std::path::PathBuf, u64, u64)> = Vec::new();
    for (p, size, mtime) in entries.into_iter() {
        if p.exists() { keep.push((p, size, mtime)); }
    }
    // If still over budget, remove oldest by mtime until under the cap
    if total > max_bytes {
        keep.sort_by_key(|x| x.2); // oldest first
        for (p, size, _mtime) in keep {
            if total <= max_bytes { break; }
            let _ = fs::remove_file(&p);
            total = total.saturating_sub(size);
        }
    }
    Ok(())
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

fn bytes_to_text(bytes: &[u8]) -> String {
    // Try UTF-8; fall back to PDFDocEncoding-ish by lossy conversion
    String::from_utf8(bytes.to_vec()).unwrap_or_else(|_| String::from_utf8_lossy(bytes).to_string())
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
// This function cleans up whitespace within text extracted from a PDF while preserving
// paragraph breaks, which are essential for good snippet generation.
fn normalize_ws_preserve_newlines(s: &str) -> String {
    // Normalize Windows newlines and stray CRs first so paragraph splitting is consistent.
    let s = s.replace("\r\n", "\n").replace('\r', "\n");
    let mut result = String::with_capacity(s.len());
    for paragraph in s.split("\n\n") {
        // Collapse intra-line whitespace while preserving paragraph breaks.
        let joined = paragraph
            .lines()
            .map(|line| line.trim())
            .collect::<Vec<&str>>()
            .join(" ");
        // Collapse repeated spaces/tabs etc. into single spaces.
        let mut collapsed = String::with_capacity(joined.len());
        let mut last_space = false;
        for ch in joined.chars() {
            let is_space = ch.is_whitespace();
            if is_space {
                if !last_space { collapsed.push(' '); }
            } else {
                collapsed.push(ch);
            }
            last_space = is_space;
        }
        let collapsed = collapsed.trim();
        if !collapsed.is_empty() {
            if !result.is_empty() { result.push_str("\n\n"); }
            result.push_str(collapsed);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_text_drops_zero_width_and_controls() {
        let s = "a\u{200B}b\u{FFFD}c\x07d"; // zero-width space, replacement char, bell
        let out = sanitize_text(s);
        assert_eq!(out, "abcd");
    }

    #[test]
    fn test_normalize_preserves_paragraphs() {
        let s = "Line 1  with   spaces\nLine 2\n\nNew para\nline";
        let out = normalize_ws_preserve_newlines(s);
        // paragraphs separated by blank line should remain separated
        let parts: Vec<&str> = out.split("\n\n").collect();
        assert_eq!(parts.len(), 2);
        assert!(parts[0].contains("Line 1 with spaces Line 2"));
        assert!(parts[1].contains("New para line"));
    }

    #[test]
    fn test_bytes_to_text_latin_fallback() {
        // invalid UTF-8, should not panic
        let bytes = vec![0xFF, 0xFE, b'A'];
        let s = bytes_to_text(&bytes);
        assert!(s.len() >= 1);
    }
}
