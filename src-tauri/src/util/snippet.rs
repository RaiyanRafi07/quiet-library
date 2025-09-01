pub fn make_snippet(text: &str, query: &str, max_len: usize) -> String {
    if text.is_empty() || query.trim().is_empty() { return String::new(); }
    let lc_text = text.to_lowercase();
    let lc_query = query.to_lowercase();
    if let Some(pos) = lc_text.find(&lc_query) {
        // center around first match, clamped to valid char boundaries
        let raw_start = pos.saturating_sub(max_len / 2);
        let raw_end = (pos + lc_query.len() + max_len / 2).min(text.len());
        let start = prev_char_boundary(text, raw_start);
        let end = next_char_boundary(text, raw_end);
        let end = end.max(start).min(text.len());
        let snippet = &text[start..end];
        trim_to_word_boundaries(snippet)
    } else {
        // fallback to head
        let end = next_char_boundary(text, max_len.min(text.len()));
        trim_to_word_boundaries(&text[..end])
    }
}

pub fn make_snippets(text: &str, query: &str, max_len: usize) -> Vec<String> {
    if text.is_empty() || query.trim().is_empty() { return vec![]; }
    let lc_query = query.to_lowercase();
    let mut snippets = Vec::new();
    for paragraph in text.split("\n\n") {
        let lc_paragraph = paragraph.to_lowercase();
        if lc_paragraph.contains(&lc_query) {
            let snippet = make_snippet(paragraph, query, max_len);
            snippets.push(snippet);
        }
    }
    snippets
}

fn prev_char_boundary(s: &str, mut idx: usize) -> usize {
    if idx > s.len() { idx = s.len(); }
    while idx > 0 && !s.is_char_boundary(idx) { idx -= 1; }
    idx
}

fn next_char_boundary(s: &str, mut idx: usize) -> usize {
    if idx > s.len() { return s.len(); }
    while idx < s.len() && !s.is_char_boundary(idx) { idx += 1; }
    idx
}

fn trim_to_word_boundaries(s: &str) -> String {
    s.trim().to_string()
}
