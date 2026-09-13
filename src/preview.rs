use std::io::Write;
use std::process::{Command, Stdio};

use crate::format::FormatKind;

pub fn show(formatted: &str, kind: FormatKind) -> Result<(), String> {
    if !cfg!(target_os = "macos") {
        return Err("native preview is macOS-only".into());
    }
    let script = jxa_script(formatted, kind);
    let mut child = Command::new("osascript")
        .args(["-l", "JavaScript"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    child
        .stdin
        .as_mut()
        .ok_or_else(|| "osascript stdin".to_string())?
        .write_all(script.as_bytes())
        .map_err(|e| e.to_string())?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

pub fn jxa_script(source: &str, kind: FormatKind) -> String {
    let title = kind.preview_heading();
    let html = format!(
        "<pre style=\"font:13px/1.5 Menlo,monospace;color:#e8e8ed;background:#1e1e22;\">{}</pre>",
        highlight(source, kind)
    );
    format!(
        r#"
ObjC.import(