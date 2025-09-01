use std::{fs, io::Read, path::Path};

fn read_prefix(path: &Path, max_bytes: usize) -> Result<String, String> {
    let mut f = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut buf = Vec::with_capacity(max_bytes);
    let _ = (&mut f).take(max_bytes as u64).read_to_end(&mut buf);
    // Try UTF-8; fall back to lossily decoding
    Ok(String::from_utf8(buf).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned()))
}

pub fn html_to_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => { in_tag = true; }
            '>' => { in_tag = false; out.push(' '); }
            _ => {
                if !in_tag { out.push(ch); }
            }
        }
    }
    normalize_ws(&out)
}

fn strip_markdown(input: &str) -> String {
    // Minimal: remove leading '#' from headings, inline formatting (* _ `), link targets keep text
    let mut out = String::with_capacity(input.len());
    let mut in_code = false;
    let mut i = 0;
    let bytes = input.as_bytes();
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c == '`' { in_code = !in_code; i += 1; continue; }
        if in_code { out.push(c); i += 1; continue; }
        if c == '[' {
            // Copy link text [text](url) -> text
            i += 1;
            while i < bytes.len() && bytes[i] as char != ']' { out.push(bytes[i] as char); i += 1; }
            // skip ]( ... )
            while i < bytes.len() && bytes[i] as char != ')' { i += 1; }
            if i < bytes.len() { i += 1; }
            continue;
        }
        if c == '#' {
            // skip leading hashes in a heading
            while i < bytes.len() && (bytes[i] as char == '#' || bytes[i] as char == ' ') { i += 1; }
            out.push('\n');
            continue;
        }
        if c == '*' || c == '_' { i += 1; continue; }
        out.push(c);
        i += 1;
    }
    normalize_ws(&out)
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

pub fn extract_title_and_text(path: &Path) -> Result<(String, String), String> {
    let max_bytes = 2 * 1024 * 1024; // 2MB cap for MVP
    let raw = read_prefix(path, max_bytes)?;
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
    if ext == "html" || ext == "htm" {
        let text = html_to_text(&raw);
        // naive <title> extraction
        let title = raw
            .to_lowercase()
            .find("<title>")
            .and_then(|start| raw[start + 7..].find("</title>").map(|end| raw[start + 7..start + 7 + end].trim().to_string()))
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| name.clone());
        Ok((title, text))
    } else if ext == "md" || ext == "markdown" {
        let text = strip_markdown(&raw);
        let title = raw
            .lines()
            .map(|l| l.trim())
            .find(|l| l.starts_with('#'))
            .map(|l| l.trim_start_matches('#').trim().to_string())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| name.clone());
        Ok((title, text))
    } else {
        // treat as plain text
        let title = raw.lines().next().map(|l| l.trim().to_string()).filter(|t| !t.is_empty()).unwrap_or_else(|| name.clone());
        Ok((title, normalize_ws(&raw)))
    }
}

pub fn is_supported_text(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        match ext.to_ascii_lowercase().as_str() {
            "txt" | "md" | "markdown" | "html" | "htm" => true,
            _ => false,
        }
    } else { false }
}
