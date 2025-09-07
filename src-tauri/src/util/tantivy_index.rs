use std::{fs, path::{Path, PathBuf}};

use tantivy::{
    schema::{Schema, SchemaBuilder, Field, TextOptions, TextFieldIndexing, IndexRecordOption, STORED, STRING, INDEXED},
    Index, TantivyDocument, doc
};
use tantivy::schema::Value; // bring as_str/as_u64 helpers into scope
use crate::{AppState, commands::library, util::{extract_text::{extract_title_and_text, is_supported_text}, extract_pdf::extract_pdf_pages_cached}, models::SearchResult};
use rayon::prelude::*;
use rayon::ThreadPoolBuilder;

pub struct IndexHandles {
    pub index: Index,
    pub fields: IndexFields,
}

#[derive(Clone, Copy)]
pub struct IndexFields {
    pub title: Field,
    pub path: Field,
    pub page: Field,
    pub section: Field,
    pub body: Field,
}

fn schema() -> (Schema, IndexFields) {
    let mut sb = SchemaBuilder::default();
    let text_indexing = TextFieldIndexing::default()
        .set_tokenizer("default")
        .set_index_option(IndexRecordOption::WithFreqsAndPositions);
    let text_opts = TextOptions::default().set_stored().set_indexing_options(text_indexing);
    let title = sb.add_text_field("title", text_opts.clone());
    let path = sb.add_text_field("path", STRING | STORED);
    let page = sb.add_u64_field("page", STORED | INDEXED);
    let section = sb.add_text_field("section", STRING | STORED);
    let body = sb.add_text_field("body", text_opts);
    let schema = sb.build();
    (schema, IndexFields { title, path, page, section, body })
}

fn index_dir(state: &AppState) -> PathBuf { state.app_dir.join("index") }

// Cap the number of pages we index per PDF to avoid extremely large
// indexing jobs on massive documents. This balances speed and memory.
const MAX_PDF_PAGES_INDEX: u32 = 300;

pub fn rebuild_index(state: &AppState) -> Result<(), String> {
    let dir = index_dir(state);
    if dir.exists() { fs::remove_dir_all(&dir).map_err(|e| e.to_string())?; }
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let (sch, fields) = schema();
    let index = Index::create_in_dir(&dir, sch).map_err(|e| e.to_string())?;
    let mut writer = index.writer(128 * 1024 * 1024).map_err(|e| e.to_string())?; // 128MB heap

    // Collect all files to index
    let mut all_files: Vec<PathBuf> = Vec::new();
    let folders = library::watched_folders(state);
    for folder in folders {
        let root = PathBuf::from(&folder);
        gather_files(&root, &mut all_files)?;
    }

    // Extract contents in parallel (with bounded parallelism)
    let cache_root = state.app_dir.join("cache");
    // Choose a conservative thread count to reduce I/O/CPU thrash
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let num_threads = threads.min(8).max(2);
    let pool = ThreadPoolBuilder::new().num_threads(num_threads).build().map_err(|e| e.to_string())?;
    let docs: Vec<(String, String, Option<u32>, Option<String>, String)> = pool.install(|| {
        all_files
            .par_iter()
            .flat_map_iter(|path| {
                let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
                if is_supported_text(path) {
                    if let Ok((title, text)) = extract_title_and_text(path) {
                        return vec![(title, path.to_string_lossy().to_string(), None, None, text)];
                    }
                } else if ext == "pdf" {
                    if let Ok((title, pages, which)) = extract_pdf_pages_cached(path, &cache_root, MAX_PDF_PAGES_INDEX) {
                        return pages
                            .into_iter()
                            .map(|(page, body)| (title.clone(), path.to_string_lossy().to_string(), Some(page), Some(which.clone()), body))
                            .collect();
                    }
                }
                Vec::new()
            })
            .collect()
    });

    // Add to index serially
    for (title, path, page, section, body) in docs {
        if let Some(p) = page {
            if let Some(sec) = section {
                let _ = writer.add_document(doc!(fields.title=>title, fields.path=>path, fields.page=>p as u64, fields.section=>sec, fields.body=>body));
            } else {
                let _ = writer.add_document(doc!(fields.title=>title, fields.path=>path, fields.page=>p as u64, fields.body=>body));
            }
        } else {
            if let Some(sec) = section {
                let _ = writer.add_document(doc!(fields.title=>title, fields.path=>path, fields.section=>sec, fields.body=>body));
            } else {
                let _ = writer.add_document(doc!(fields.title=>title, fields.path=>path, fields.body=>body));
            }
        }
    }

    writer.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct Fingerprints { entries: std::collections::HashMap<String, (u64, u64)> } // path -> (mtime,size)

fn load_fingerprints(dir: &Path) -> Fingerprints {
    let p = dir.join("fingerprints.json");
    if let Ok(bytes) = fs::read(&p) {
        if let Ok(fp) = serde_json::from_slice::<Fingerprints>(&bytes) { return fp; }
    }
    Fingerprints::default()
}

fn save_fingerprints(dir: &Path, fp: &Fingerprints) { let _ = fs::write(dir.join("fingerprints.json"), serde_json::to_vec(fp).unwrap_or_default()); }

fn file_fp(path: &Path) -> Option<(u64, u64)> {
    let meta = fs::metadata(path).ok()?;
    let size = meta.len();
    let mtime = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    Some((mtime, size))
}

fn open_or_create_index(dir: &Path) -> Result<Index, String> {
    let (sch, _fields) = schema();
    if dir.exists() { Index::open_in_dir(dir).map_err(|e| e.to_string()) }
    else { fs::create_dir_all(dir).ok(); Index::create_in_dir(dir, sch).map_err(|e| e.to_string()) }
}

pub fn incremental_update(state: &AppState) -> Result<(), String> {
    let dir = index_dir(state);
    let index = open_or_create_index(&dir)?;
    let (_, fields) = schema();

    // Collect current files
    let mut all_files: Vec<PathBuf> = Vec::new();
    let folders = crate::commands::library::watched_folders(state);
    for folder in folders { gather_files(&PathBuf::from(folder), &mut all_files)?; }

    let mut current_fp: std::collections::HashMap<String, (u64, u64)> = std::collections::HashMap::new();
    let mut changed: Vec<PathBuf> = Vec::new();
    for p in &all_files {
        if let Some((mt, sz)) = file_fp(p) { current_fp.insert(p.to_string_lossy().to_string(), (mt, sz)); }
    }
    let prev = load_fingerprints(&dir);
    for p in &all_files {
        let key = p.to_string_lossy().to_string();
        let cur = current_fp.get(&key).copied();
        let old = prev.entries.get(&key).copied();
        if cur != old { changed.push(p.clone()); }
    }
    // Deleted files
    let mut deleted: Vec<String> = Vec::new();
    for (k, _) in prev.entries.iter() {
        if !current_fp.contains_key(k) { deleted.push(k.clone()); }
    }

    // Extract changed in parallel
    let cache_root = state.app_dir.join("cache");
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let num_threads = threads.min(8).max(2);
    let pool = ThreadPoolBuilder::new().num_threads(num_threads).build().map_err(|e| e.to_string())?;
    let docs: Vec<(String, String, Option<u32>, Option<String>, String)> = pool.install(|| {
        changed
            .par_iter()
            .flat_map_iter(|path| {
                let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
                if is_supported_text(path) {
                    if let Ok((title, text)) = extract_title_and_text(path) {
                        return vec![(title, path.to_string_lossy().to_string(), None, None, text)];
                    }
                } else if ext == "pdf" {
                    if let Ok((title, pages, which)) = extract_pdf_pages_cached(path, &cache_root, MAX_PDF_PAGES_INDEX) {
                        return pages
                            .into_iter()
                            .map(|(page, body)| (title.clone(), path.to_string_lossy().to_string(), Some(page), Some(which.clone()), body))
                            .collect();
                    }
                }
                Vec::new()
            })
            .collect()
    });

    // Apply to index
    let mut writer = index.writer(128 * 1024 * 1024).map_err(|e| e.to_string())?;
    // Delete removed or changed paths before re-adding
    for k in deleted.iter().chain(changed.iter().map(|p| p.to_string_lossy().to_string()).collect::<Vec<_>>().iter()) {
        let term = tantivy::Term::from_field_text(fields.path, k);
        writer.delete_term(term);
    }
    for (title, path, page, section, body) in docs {
        if let Some(p) = page {
            if let Some(sec) = section {
                let _ = writer.add_document(doc!(fields.title=>title, fields.path=>path, fields.page=>p as u64, fields.section=>sec, fields.body=>body));
            } else {
                let _ = writer.add_document(doc!(fields.title=>title, fields.path=>path, fields.page=>p as u64, fields.body=>body));
            }
        } else {
            if let Some(sec) = section {
                let _ = writer.add_document(doc!(fields.title=>title, fields.path=>path, fields.section=>sec, fields.body=>body));
            } else {
                let _ = writer.add_document(doc!(fields.title=>title, fields.path=>path, fields.body=>body));
            }
        }
    }
    writer.commit().map_err(|e| e.to_string())?;

    // Save new fingerprint set
    save_fingerprints(&dir, &Fingerprints { entries: current_fp });
    // Drop cached index/reader to pick up new segments
    drop_cached_index(state);
    Ok(())
}

fn gather_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    if !dir.exists() { return Ok(()); }
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = match entry { Ok(e) => e, Err(_) => continue };
        let path = entry.path();
        if path.is_dir() { gather_files(&path, out)?; }
        else { out.push(path); }
    }
    Ok(())
}

pub fn search_index(state: &AppState, q: &str, limit: usize) -> Result<Vec<SearchResult>, String> {
    let t0 = std::time::Instant::now();
    let dir = index_dir(state);
    if !dir.exists() { return Ok(vec![]); }
    let (_, fields) = schema();
    // Lazily open and cache index + reader in AppState for faster subsequent queries
    {
        let mut idx_lock = state.index.lock().map_err(|_| "index lock".to_string())?;
        if idx_lock.is_none() {
            let index = Index::open_in_dir(&dir).map_err(|e| e.to_string())?;
            *idx_lock = Some(index);
        }
    }
    {
        let mut reader_lock = state.reader.lock().map_err(|_| "reader lock".to_string())?;
        if reader_lock.is_none() {
            let idx_lock = state.index.lock().map_err(|_| "index lock".to_string())?;
            let index = idx_lock.as_ref().ok_or_else(|| "index not available".to_string())?;
            let reader = index.reader().map_err(|e| e.to_string())?;
            *reader_lock = Some(reader);
        }
    }
    let reader = {
        let r = state.reader.lock().map_err(|_| "reader lock".to_string())?;
        r.as_ref().unwrap().clone()
    };
    // Pick up any new segments if index was rebuilt
    let _ = reader.reload();
    let searcher = reader.searcher();
    let idx_guard = state.index.lock().map_err(|_| "index lock".to_string())?;
    let index_ref = idx_guard.as_ref().ok_or_else(|| "index not available".to_string())?;
    let qp = tantivy::query::QueryParser::for_index(index_ref, vec![fields.title, fields.body]);
    let query = qp.parse_query(q).map_err(|e| e.to_string())?;
    let top_docs = searcher
        .search(&query, &tantivy::collector::TopDocs::with_limit(limit))
        .map_err(|e| e.to_string())?;

    let mut results: Vec<SearchResult> = Vec::new();
    'outer: for (score, addr) in top_docs {
        let document: TantivyDocument = searcher.doc::<TantivyDocument>(addr).map_err(|e| e.to_string())?;
        let title = document.get_first(fields.title).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let path = document.get_first(fields.path).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let page = document.get_first(fields.page).and_then(|v| v.as_u64()).map(|v| v as u32);
        let section = document.get_first(fields.section).and_then(|v| v.as_str()).map(|s| s.to_string());
        let body = document.get_first(fields.body).and_then(|v| v.as_str()).unwrap_or("");

        // Prefer multiple paragraph snippets if available; otherwise a single centered snippet.
        let mut snippets = crate::util::snippet::make_snippets(body, q, 400);
        if snippets.is_empty() {
            let one = crate::util::snippet::make_snippet(body, q, 400);
            if !one.is_empty() { snippets.push(one); }
        }

        for snippet in snippets {
            results.push(SearchResult { title: title.clone(), path: path.clone(), page, section: section.clone(), snippet, score: score as f32 });
            if results.len() >= limit { break 'outer; }
        }
    }
    let elapsed = t0.elapsed();
    eprintln!("quietlibrary: search_index q=\"{}\" n={} elapsed={}ms", q, results.len(), elapsed.as_millis());
    Ok(results)
}

// Return sorted distinct pages within a single document path that match the query.
pub fn search_pages_for_document(state: &AppState, path: &str, q: &str, limit: usize) -> Result<Vec<u32>, String> {
    let t0 = std::time::Instant::now();
    let dir = index_dir(state);
    if !dir.exists() { return Ok(vec![]); }
    let (_, fields) = schema();
    {
        let mut idx_lock = state.index.lock().map_err(|_| "index lock".to_string())?;
        if idx_lock.is_none() {
            let index = Index::open_in_dir(&dir).map_err(|e| e.to_string())?;
            *idx_lock = Some(index);
        }
    }
    {
        let mut reader_lock = state.reader.lock().map_err(|_| "reader lock".to_string())?;
        if reader_lock.is_none() {
            let idx_lock = state.index.lock().map_err(|_| "index lock".to_string())?;
            let index = idx_lock.as_ref().ok_or_else(|| "index not available".to_string())?;
            let reader = index.reader().map_err(|e| e.to_string())?;
            *reader_lock = Some(reader);
        }
    }
    let reader = {
        let r = state.reader.lock().map_err(|_| "reader lock".to_string())?;
        r.as_ref().unwrap().clone()
    };
    let _ = reader.reload();
    let searcher = reader.searcher();
    let idx_guard = state.index.lock().map_err(|_| "index lock".to_string())?;
    let index_ref = idx_guard.as_ref().ok_or_else(|| "index not available".to_string())?;

    // Query: path == {path} AND body matches {q}
    use tantivy::query::{BooleanQuery, Occur, QueryParser, TermQuery};
    let path_term = tantivy::Term::from_field_text(fields.path, path);
    let path_q = TermQuery::new(path_term, IndexRecordOption::Basic);
    let qp = QueryParser::for_index(index_ref, vec![fields.body]);
    let body_q = qp.parse_query(q).map_err(|e| e.to_string())?;
    let boolean = BooleanQuery::new(vec![
        (Occur::Must, Box::new(path_q) as Box<dyn tantivy::query::Query>),
        (Occur::Must, body_q),
    ]);

    let top_docs = searcher
        .search(&boolean, &tantivy::collector::TopDocs::with_limit(limit))
        .map_err(|e| e.to_string())?;
    use std::collections::BTreeSet;
    let mut pages: BTreeSet<u32> = BTreeSet::new();
    for (_score, addr) in top_docs {
        let document: TantivyDocument = searcher.doc::<TantivyDocument>(addr).map_err(|e| e.to_string())?;
        if let Some(p) = document.get_first(fields.page).and_then(|v| v.as_u64()) { pages.insert(p as u32); }
    }
    let pages_vec: Vec<u32> = pages.into_iter().collect();
    let elapsed = t0.elapsed();
    eprintln!("quietlibrary: search_pages_for_document file={} hits={} elapsed={}ms", path, pages_vec.len(), elapsed.as_millis());
    Ok(pages_vec)
}

// Drop cached index/reader after a rebuild
pub fn drop_cached_index(state: &AppState) {
    if let Ok(mut r) = state.reader.lock() { *r = None; }
    if let Ok(mut i) = state.index.lock() { *i = None; }
}
