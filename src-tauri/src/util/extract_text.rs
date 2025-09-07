use std::{fs, io::Read, path::Path};
use pulldown_cmark::{Event, Options, Parser};

fn read_prefix(path: &Path, max_bytes: usize) -> Result<String, String> {
    let mut f = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut buf = Vec::with_capacity(max_bytes);
    let _ = (&mut f).take(max_bytes as u64).read_to_end(&mut buf);
    // Try UTF-8; fall back to lossily decoding using the original bytes
    let text = String::from_utf8(buf).unwrap_or_else(|e| {
        let bytes = e.into_bytes();
        String::from_utf8_lossy(&bytes).into_owned()
    });
    Ok(text)
}

fn markdown_to_text(input: &str) -> String {
    let mut text_content = String::new();
    let parser = Parser::new_ext(input, Options::empty());
    for event in parser {
        if let Event::Text(text) = event {
            text_content.push_str(&text);
        }
    }
    text_content
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
        let text = html2text::from_read(raw.as_bytes(), 80);
        // naive <title> extraction
        let title = raw
            .to_lowercase()
            .find("<title>")
            .and_then(|start| raw[start + 7..].find("</title>").map(|end| raw[start + 7..start + 7 + end].trim().to_string()))
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| name.clone());
        Ok((title, text))
    } else if ext == "md" || ext == "markdown" {
        let text = markdown_to_text(&raw);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn test_normalize_ws() {
        let s = "  hello\t\tworld\nnew\r\nline  ";
        assert_eq!(normalize_ws(s), "hello world new line");
    }

    #[test]
    fn test_markdown_to_text_basic() {
        let md = "# Title\n\nHello **world**";
        let txt = markdown_to_text(md);
        // pulldown_cmark Text events concatenate without markup
        assert!(txt.contains("Title"));
        assert!(txt.contains("Hello"));
        assert!(txt.contains("world"));
    }

    #[test]
    fn test_is_supported_text() {
        let cases = [
            ("a.txt", true), ("b.md", true), ("c.markdown", true), ("d.html", true), ("e.htm", true), ("f.pdf", false)
        ];
        for (name, want) in cases {
            assert_eq!(is_supported_text(Path::new(name)), want, "{}", name);
        }
    }

    #[test]
    fn test_extract_plain_title_and_text() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("note.txt");
        let content = "My Title\nThis is the body.";
        std::fs::write(&path, content).unwrap();
        let (title, text) = extract_title_and_text(&path).unwrap();
        assert_eq!(title, "My Title");
        assert!(text.contains("This is the body."));
    }

    #[test]
    fn test_extract_markdown_title() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("doc.md");
        let content = "# Heading\n\nParagraph text";
        std::fs::write(&path, content).unwrap();
        let (title, text) = extract_title_and_text(&path).unwrap();
        assert_eq!(title, "Heading");
        assert!(text.contains("Heading"));
        assert!(text.contains("Paragraph text"));
    }

    #[test]
    fn test_extract_html_title() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("page.html");
        let content = r#"<html><head><title>Test Title</title></head><body><p>Hello</p></body></html>"#;
        std::fs::write(&path, content).unwrap();
        let (title, text) = extract_title_and_text(&path).unwrap();
        assert_eq!(title, "Test Title");
        assert!(text.contains("Hello"));
    }

    #[test]
    fn test_read_prefix_lossy_non_utf8() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("bin.txt");
        // Invalid UTF-8 sequence
        let bytes = b"Title\n\xFF\xFE\xFA";
        std::fs::write(&path, bytes).unwrap();
        let s = read_prefix(&path, 1024).unwrap();
        // Replacement char appears
        assert!(s.contains("Title"));
        assert!(s.chars().any(|c| c == '\u{FFFD}'));
    }
}
