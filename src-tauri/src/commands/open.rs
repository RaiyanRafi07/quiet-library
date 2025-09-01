use tauri::{AppHandle, Manager};
use tauri::async_runtime::spawn_blocking;

use crate::models::OpenTarget;
use pdfium_render::prelude::*;
use base64::{engine::general_purpose, Engine as _};
use png::{BitDepth, ColorType, Encoder};
use serde::Serialize;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use once_cell::sync::Lazy;
use std::sync::Mutex;
use lru::LruCache;
use std::num::NonZeroUsize;

#[tauri::command]
pub fn resolve_open_target(app: AppHandle, path: String, page: Option<u32>, section: Option<String>) -> Result<OpenTarget, String> {
    let _resolver = app.path_resolver();
    // Convert absolute path to a file:// URL. Tauri viewer assets will use this.
    let url = if path.starts_with("file://") { path.clone() } else { format!("file://{}", path) };
    Ok(OpenTarget { url, path, page, section })
}

#[tauri::command]
pub fn reveal_in_os(window: tauri::Window, path: String) -> Result<(), String> {
    tauri::api::shell::open(&window.shell_scope(), path, None).map_err(|e| e.to_string())
}

static INIT_GUARD: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

fn bind_pdfium() -> Result<Pdfium, String> {
    let _guard = INIT_GUARD.lock().unwrap();
    // Try system library; if that fails, try to find pdfium.dll near the executable and cwd.
    if let Ok(path) = std::env::var("PDFIUM_PATH") {
        let p = std::path::PathBuf::from(&path);
        if p.exists() {
            return Pdfium::bind_to_library(p).map(Pdfium::new).map_err(|e| format!("bind pdfium (PDFIUM_PATH): {:?}", e));
        }
    }
    match Pdfium::bind_to_system_library() {
        Ok(bindings) => Ok(Pdfium::new(bindings)),
        Err(e) => {
            let mut candidates = Vec::new();
            if let Ok(exe) = std::env::current_exe() {
                if let Some(dir) = exe.parent() {
                    candidates.push(dir.to_path_buf());
                    if let Some(p) = dir.parent() { candidates.push(p.to_path_buf());
                        if let Some(pp) = p.parent() { candidates.push(pp.to_path_buf()); }
                    }
                    candidates.push(dir.join("resources"));
                    candidates.push(dir.join("resources").join("pdfium"));
                    // Look up the tree for `src-tauri/resources[/pdfium]`
                    for anc in [dir, dir.parent().unwrap_or(dir), dir.parent().and_then(|x| x.parent()).unwrap_or(dir)] {
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
            let mut tried: Vec<std::path::PathBuf> = Vec::new();
            for dir in candidates.clone() {
                let p = Pdfium::pdfium_platform_library_name_at_path(&dir);
                if p.exists() {
                    return Pdfium::bind_to_library(p)
                        .map(Pdfium::new)
                        .map_err(|e2| format!("bind pdfium from {:?}: {:?}", dir, e2));
                }
                tried.push(dir);
            }
            eprintln!("quietlibrary: pdfium not found; tried: {:?}", tried);
            Err(format!("bind pdfium: {:?}", e))
        }
    }
}

#[tauri::command]
pub async fn pdf_page_count(path: String) -> Result<u32, String> {
    eprintln!("quietlibrary: pdf_page_count path={}", path);
    spawn_blocking(move || {
        let pdfium = bind_pdfium()?;
        let doc = pdfium
            .load_pdf_from_file(&path, None)
            .map_err(|e| format!("open: {:?}", e))?;
        Ok::<u32, String>(doc.pages().len() as u32)
    })
    .await
    .map_err(|e| format!("join error: {:?}", e))?
}

#[derive(Serialize)]
pub struct RenderPageResult {
    pub data_url: Option<String>,
    pub file_path: Option<String>,
    pub width_px: u32,
    pub height_px: u32,
    pub width_pt: f32,
    pub height_pt: f32,
    pub rects_px: Vec<[u32; 4]>,
}

// Simple in-memory PNG render cache keyed by (path, page, widthBucket)
type CacheKey = (String, u32, u32);
struct CachedImage { png: Vec<u8>, w: u32, h: u32, width_pt: f32, height_pt: f32 }
static IMAGE_CACHE: Lazy<Mutex<LruCache<CacheKey, CachedImage>>> = Lazy::new(|| {
    let cap = NonZeroUsize::new(16).unwrap();
    Mutex::new(LruCache::new(cap))
});

fn width_bucket(w: u32) -> u32 { ((w + 63) / 64) * 64 }

fn render_cache_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("quietlibrary-render");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn key_hash(key: &CacheKey) -> String {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[tauri::command]
pub async fn render_pdf_page(path: String, page: u32, width: u32, query: Option<String>) -> Result<RenderPageResult, String> {
    eprintln!("quietlibrary: render_pdf_page path={} page={} width={} q={}", path, page, width, query.as_deref().unwrap_or(""));
    spawn_blocking(move || {
        let pdfium = bind_pdfium()?;
        let doc = pdfium
            .load_pdf_from_file(&path, None)
            .map_err(|e| format!("open: {:?}", e))?;
        let idx = page.saturating_sub(1) as u16;
        let page_obj = doc
            .pages()
            .get(idx)
            .map_err(|e| format!("load page: {:?}", e))?;
        let settings = PdfRenderConfig::new().set_target_width(Pixels::from(width as i32));
        let bucket = width_bucket(width);
        let key: CacheKey = (path.clone(), page as u32, bucket);
        let (png_bytes, w, h, width_pt, height_pt) = {
            if let Some(hit) = IMAGE_CACHE.lock().unwrap().get(&key) {
                (hit.png.clone(), hit.w, hit.h, hit.width_pt, hit.height_pt)
            } else {
                let bitmap = page_obj
                    .render_with_config(&settings)
                    .map_err(|e| format!("render: {:?}", e))?;
                let bytes = bitmap.as_rgba_bytes();
                let w = bitmap.width() as u32;
                let h = bitmap.height() as u32;
                let width_pt = page_obj.width().value;
                let height_pt = page_obj.height().value;
                let mut buf = Vec::new();
                {
                    let mut encoder = Encoder::new(&mut buf, w, h);
                    encoder.set_color(ColorType::Rgba);
                    encoder.set_depth(BitDepth::Eight);
                    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
                    writer.write_image_data(&bytes).map_err(|e| e.to_string())?;
                }
                IMAGE_CACHE
                    .lock()
                    .unwrap()
                    .put(key.clone(), CachedImage { png: buf.clone(), w, h, width_pt, height_pt });
                (buf, w, h, width_pt, height_pt)
            }
        };

        // Compute pixel-space rects for this page if a query is provided
        let mut rects_px: Vec<[u32; 4]> = Vec::new();
        if let Some(q) = query {
            let qt = q.trim().to_string();
            if !qt.is_empty() {
                if let Ok(text) = page_obj.text() {
                    if let Ok(search) = text.search(&qt, &PdfSearchOptions::new()) {
                        for segs in search.iter(PdfSearchDirection::SearchForward) {
                            for seg in segs.iter() {
                                let b = seg.bounds();
                                if let (Ok((x1, y1)), Ok((x2, y2))) = (
                                    page_obj.points_to_pixels(b.left(), b.top(), &settings),
                                    page_obj.points_to_pixels(b.right(), b.bottom(), &settings),
                                ) {
                                    let left = x1.max(0) as u32;
                                    let top = y1.max(0) as u32;
                                    let right = x2.max(0) as u32;
                                    let bottom = y2.max(0) as u32;
                                    if right > left && bottom > top {
                                        rects_px.push([left, top, right - left, bottom - top]);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        eprintln!("quietlibrary: rendered page={} size={} bytes w={} h={} (pt {}x{})", page, png_bytes.len(), w, h, width_pt, height_pt);
        // Write PNG to a temp file and return its path to avoid huge IPC payloads.
        let cache_dir = render_cache_dir();
        let hash = key_hash(&key);
        let path_png = cache_dir.join(format!("page_{}.png", hash));
        match std::fs::write(&path_png, &png_bytes) {
            Ok(_) => {
                Ok::<RenderPageResult, String>(RenderPageResult {
                    data_url: None,
                    file_path: Some(path_png.to_string_lossy().to_string()),
                    width_px: w,
                    height_px: h,
                    width_pt,
                    height_pt,
                    rects_px,
                })
            }
            Err(_e) => {
                // Fall back to data URL if writing fails
                let b64 = general_purpose::STANDARD.encode(png_bytes);
                Ok::<RenderPageResult, String>(RenderPageResult {
                    data_url: Some(format!("data:image/png;base64,{}", b64)),
                    file_path: None,
                    width_px: w,
                    height_px: h,
                    width_pt,
                    height_pt,
                    rects_px,
                })
            }
        }
    })
    .await
    .map_err(|e| format!("join error: {:?}", e))?
}

#[tauri::command]
pub async fn warm_render_cache(path: String, page: u32, width: u32) -> Result<(), String> {
    eprintln!("quietlibrary: warm_render_cache path={} page={} width={}", path, page, width);
    spawn_blocking(move || {
        let pdfium = bind_pdfium()?;
        let doc = pdfium
            .load_pdf_from_file(&path, None)
            .map_err(|e| format!("open: {:?}", e))?;
        let idx = page.saturating_sub(1) as u16;
        let page_obj = doc
            .pages()
            .get(idx)
            .map_err(|e| format!("load page: {:?}", e))?;
        let settings = PdfRenderConfig::new().set_target_width(Pixels::from(width as i32));
        let bucket = width_bucket(width);
        let key: CacheKey = (path.clone(), page as u32, bucket);
        if IMAGE_CACHE.lock().unwrap().get(&key).is_some() {
            return Ok::<(), String>(());
        }
        let bitmap = page_obj
            .render_with_config(&settings)
            .map_err(|e| format!("render: {:?}", e))?;
        let bytes = bitmap.as_rgba_bytes();
        let w = bitmap.width() as u32;
        let h = bitmap.height() as u32;
        let width_pt = page_obj.width().value;
        let height_pt = page_obj.height().value;
        let mut buf = Vec::new();
        {
            let mut encoder = Encoder::new(&mut buf, w, h);
            encoder.set_color(ColorType::Rgba);
            encoder.set_depth(BitDepth::Eight);
            let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
            writer.write_image_data(&bytes).map_err(|e| e.to_string())?;
        }
        IMAGE_CACHE
            .lock()
            .unwrap()
            .put(key, CachedImage { png: buf, w, h, width_pt, height_pt });
        // Also write to disk cache for the frontend to load directly.
        let disk_key = (path.clone(), page as u32, bucket);
        let hash = key_hash(&disk_key);
        let p = render_cache_dir().join(format!("page_{}.png", hash));
        let _ = std::fs::write(&p, &IMAGE_CACHE.lock().unwrap().get(&disk_key).unwrap().png);
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| format!("join error: {:?}", e))?
}

#[derive(Serialize)]
pub struct PdfMatchRects {
    pub page: u32,
    // Each rect is [left, bottom, right, top] in page points
    pub rects: Vec<[f32; 4]>,
}

#[tauri::command]
pub async fn pdf_find_matches(path: String, query: String, limit: Option<u32>) -> Result<Vec<PdfMatchRects>, String> {
    spawn_blocking(move || {
        let q = query.trim();
        if q.is_empty() { return Ok::<_, String>(vec![]); }
        let q_norm: String = q.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect();
        let pdfium = bind_pdfium()?;
        let doc = pdfium.load_pdf_from_file(&path, None).map_err(|e| format!("open: {:?}", e))?;
        let mut out: Vec<PdfMatchRects> = Vec::new();
        let mut total = 0usize;
        let max = limit.unwrap_or(2000) as usize;
        for i in doc.pages().as_range() {
            let page = match doc.pages().get(i) { Ok(p) => p, Err(_) => continue };
            let text = match page.text() { Ok(t) => t, Err(_) => continue };
            let search = match text.search(&q, &PdfSearchOptions::new()) { Ok(s) => s, Err(_) => continue };
            for segs in search.iter(PdfSearchDirection::SearchForward) {
                let mut rects: Vec<[f32;4]> = Vec::new();
                let mut combined = String::new();
                for seg in segs.iter() {
                    combined.push_str(&seg.text());
                    let b = seg.bounds();
                    rects.push([b.left().value, b.bottom().value, b.right().value, b.top().value]);
                }
                let norm = combined.replace("-
", "");
                let norm: String = norm.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect();
                if !norm.contains(&q_norm) { continue; }
                out.push(PdfMatchRects { page: i as u32 + 1, rects });
                total += 1;
                if total >= max { break; }
            }
            if total >= max { break; }
        }
        Ok::<_, String>(out)
    }).await.map_err(|e| format!("join error: {:?}", e))?
}

#[tauri::command]
pub async fn pdf_next_prev_page_with_match(path: String, query: String, from_page: u32, direction: i32, scan_limit: Option<u32>) -> Result<Option<u32>, String> {
    eprintln!("quietlibrary: pdf_next_prev_page_with_match path={} from={} dir={} q={} limit={}", path, from_page, direction, query, scan_limit.unwrap_or(0));
    spawn_blocking(move || {
        let q = query.trim();
        if q.is_empty() { return Ok::<_, String>(None); }
        let pdfium = bind_pdfium()?;
        let doc = pdfium.load_pdf_from_file(&path, None).map_err(|e| format!("open: {:?}", e))?;
        let pages = doc.pages().len() as u32;
        let limit = scan_limit.unwrap_or(200);
        let mut scanned = 0u32;
        if direction >= 0 {
            let mut p = (from_page + 1).min(pages);
            while p <= pages && scanned < limit {
                if let Ok(pg) = doc.pages().get((p-1) as u16) {
                    if let Ok(t) = pg.text() { if t.search(&q, &PdfSearchOptions::new()).is_ok() { return Ok(Some(p)); } }
                }
                p += 1; scanned += 1;
            }
        } else {
            let mut p: i32 = (from_page as i32 - 1).max(1);
            while p >= 1 && scanned < limit {
                if let Ok(pg) = doc.pages().get((p-1) as u16) {
                    if let Ok(t) = pg.text() { if t.search(&q, &PdfSearchOptions::new()).is_ok() { return Ok(Some(p as u32)); } }
                }
                p -= 1; scanned += 1;
            }
        }
        Ok(None)
    }).await.map_err(|e| format!("join error: {:?}", e))?
}
