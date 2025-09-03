#[tauri::command]
pub fn reveal_in_os(_window: tauri::Window, path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        let p = std::path::Path::new(&path);
        let arg = if p.exists() { format!("/select,{}", path) } else { path.clone() };
        let _ = Command::new("explorer").arg(arg).status();
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let _ = Command::new("open").arg("-R").arg(&path).status();
        return Ok(());
    }
    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        let folder = std::path::Path::new(&path).parent().unwrap_or(std::path::Path::new("/"));
        let _ = Command::new("xdg-open").arg(folder).status();
        return Ok(());
    }
}
// Intentionally minimal: PDF rendering is done client-side with PDF.js now.
