use std::{fs, path::{Path, PathBuf}};

use tantivy::{
    schema::{Schema, SchemaBuilder, Field, TextOptions, TextFieldIndexing, IndexRecordOption, STORED, STRING, INDEXED},
    Index, TantivyDocument, doc
};
use tantivy::schema::Value;

use crate::{AppState, commands::library, util::{extract_text::{extract_title_and_text, is_supported_text}, extract_pdf::extract_pdf_pages_cached, snippet::make_snippet}, models::SearchResult};
use rayon::prelude::*;

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

    // Extract contents in parallel
    let cache_root = state.app_dir.join("cache");
    let docs: Vec<(String, String, Option<u32>, Option<String>, String)> = all_files
        .par_iter()
        .flat_map_iter(|path| {
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
            if is_supported_text(path) {
                if let Ok((title, text)) = extract_title_and_text(path) {
                    return vec![(title, path.to_string_lossy().to_string(), None, None, text)];
                }
            } else if ext == "pdf" {
                if let Ok((title, pages, which)) = extract_pdf_pages_cached(path, &cache_root, 2_000) {
                    return pages
                        .into_iter()
                        .map(|(page, body)| (title.clone(), path.to_string_lossy().to_string(), Some(page), Some(which.clone()), body))
                        .collect();
                }
            }
            Vec::new()
        })
        .collect();

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
    let dir = index_dir(state);
    if !dir.exists() { return Ok(vec![]); }
    let (_, fields) = schema();
    let index = Index::open_in_dir(&dir).map_err(|e| e.to_string())?;
    let reader = index.reader().map_err(|e| e.to_string())?;
    let searcher = reader.searcher();
    let qp = tantivy::query::QueryParser::for_index(&index, vec![fields.title, fields.body]);
    let query = qp.parse_query(q).map_err(|e| e.to_string())?;
    let top_docs = searcher.search(&query, &tantivy::collector::TopDocs::with_limit(limit)).map_err(|e| e.to_string())?;

    let mut results = Vec::with_capacity(top_docs.len());
    for (score, addr) in top_docs {
        let document: TantivyDocument = searcher.doc::<TantivyDocument>(addr).map_err(|e| e.to_string())?;
        let title = document.get_first(fields.title).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let path = document.get_first(fields.path).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let page = document.get_first(fields.page).and_then(|v| v.as_u64()).map(|v| v as u32);
        let section = document.get_first(fields.section).and_then(|v| v.as_str()).map(|s| s.to_string());
        let body = document.get_first(fields.body).and_then(|v| v.as_str()).unwrap_or("");
        let snippet = make_snippet(body, q, 400);
        results.push(SearchResult { title, path, page, section, snippet, score: score as f32 });
    }
    Ok(results)
}
