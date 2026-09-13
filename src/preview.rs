use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::format::FormatKind;

pub fn show(formatted: &str, kind: FormatKind) -> Result<(), String> {
    let html = render_html(formatted, kind);
    let path = preview_path();
    fs::write(&path, html).map_err(|e| e.to_string())?;
    open_file(&path)
}

fn preview_path() -> PathBuf {
    std::env::temp_dir().join("copycraft-preview.html")
}

fn open_file(path: &PathBuf) -> Result<(), String> {
    let status = if cfg!(target_os = "macos") {
        Command::new("open").arg(path).status()
    } else if cfg!(target_os = "windows") {
        Command::new("cmd").args(["/C", "start", ""]).arg(path).status()
    } else {
        Command::new("xdg-open").arg(path).status()
    };
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!("open failed: {s}")),
        Err(e) => Err(e.to_string()),
    }
}

pub fn render_html(source: &str, kind: FormatKind) -> String {
    let highlighted = highlight(source, kind);
    let escaped_raw = escape_js_string(source);
    let title = kind.preview_heading();
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>{title} — Copycraft</title>
<style>
  :root {{
    --bg: #1e1e22;
    --panel: #25252b;
    --text: #e8e8ed;
    --muted: #9a9aa8;
    --key: #7dd3fc;
    --str: #86efac;
    --num: #fbbf24;
    --kw: #c4b5fd;
    --punct: #cbd5e1;
    --btn: #3b82f6;
  }}
  * {{ box-sizing: border-box; }}
  body {{
    margin: 0;
    font: 13px/1.5 ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    background: var(--bg);
    color: var(--text);
  }}
  header {{
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 12px 16px;
    background: var(--panel);
    border-bottom: 1px solid #333;
  }}
  h1 {{ margin: 0; font-size: 14px; font-weight: 600; }}
  button {{
    border: 0;
    border-radius: 8px;
    padding: 8px 12px;
    background: var(--btn);
    color: white;
    font: inherit;
    cursor: pointer;
  }}
  button:hover {{ filter: brightness(1.1); }}
  pre {{
    margin: 0;
    padding: 16px;
    overflow: auto;
    min-height: 240px;
    white-space: pre;
  }}
  .k {{ color: var(--key); }}
  .s {{ color: var(--str); }}
  .n {{ color: var(--num); }}
  .w {{ color: var(--kw); }}
  .p {{ color: var(--punct); }}
  .status {{ color: var(--muted); font-size: 12px; }}
</style>
</head>
<body>
<header>
  <h1>{title}</h1>
  <div>
    <span id="status" class="status"></span>
    <button id="copy">Copy to clipboard</button>
  </div>
</header>
<pre id="view">{highlighted}</pre>
<script>
const raw = {escaped_raw};
document.getElementById("copy").addEventListener("click", async () => {{
  try {{
    await navigator.clipboard.writeText(raw);
    document.getElementById("status").textContent = "Copied";
  }} catch (err) {{
    document.getElementById("status").textContent = "Copy failed";
  }}
}});
</script>
</body>
</html>
"#
    )
}

fn highlight(source: &str, kind: FormatKind) -> String {
    match kind {
        FormatKind::Json => highlight_json(source),
        FormatKind::Rust | FormatKind::Java => highlight_code(source, kind),
        _ => escape_html(source),
    }
}

fn highlight_json(source: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = source.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '"' {
            let (token, next) = take_string(&chars, i);
            let after = skip_ws(&chars, next);
            let class = if after < chars.len() && chars[after] == ':' {
                "k"
            } else {
                "s"
            };
            out.push_str(&span(class, &token));
            i = next;
            continue;
        }
        if ch.is_ascii_digit() || (ch == '-' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit())
        {
            let (token, next) = take_while(&chars, i, |c| {
                c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E')
            });
            out.push_str(&span("n", &token));
            i = next;
            continue;
        }
        if starts_with(&chars, i, "true") || starts_with(&chars, i, "false") || starts_with(&chars, i, "null")
        {
            let word = if starts_with(&chars, i, "true") {
                "true"
            } else if starts_with(&chars, i, "false") {
                "false"
            } else {
                "null"
            };
            out.push_str(&span("w", word));
            i += word.chars().count();
            continue;
        }
        if matches!(ch, '{' | '}' | '[' | ']' | ':' | ',') {
            out.push_str(&span("p", &ch.to_string()));
            i += 1;
            continue;
        }
        out.push_str(&escape_html(&ch.to_string()));
        i += 1;
    }
    out
}

fn highlight_code(source: &str, kind: FormatKind) -> String {
    let keywords: &[&str] = match kind {
        FormatKind::Rust => &[
            "fn", "let", "mut", "pub", "impl", "struct", "enum", "match", "if", "else", "use",
            "mod", "return", "async", "await", "self", "Self", "crate", "const", "static",
        ],
        _ => &[
            "public", "private", "protected", "class", "static", "void", "int", "long",
            "boolean", "return", "if", "else", "new", "package", "import", "final",
            "override", "this",
        ],
    };
    let mut out = String::new();
    let chars: Vec<char> = source.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '"' {
            let (token, next) = take_string(&chars, i);
            out.push_str(&span("s", &token));
            i = next;
            continue;
        }
        if ch.is_ascii_alphabetic() || ch == '_' {
            let (token, next) = take_while(&chars, i, |c| c.is_ascii_alphanumeric() || c == '_');
            if keywords.contains(&token.as_str()) {
                out.push_str(&span("w", &token));
            } else {
                out.push_str(&escape_html(&token));
            }
            i = next;
            continue;
        }
        if ch.is_ascii_digit() {
            let (token, next) = take_while(&chars, i, |c| c.is_ascii_digit() || c == '.');
            out.push_str(&span("n", &token));
            i = next;
            continue;
        }
        out.push_str(&escape_html(&ch.to_string()));
        i += 1;
    }
    out
}

fn take_string(chars: &[char], start: usize) -> (String, usize) {
    let mut i = start + 1;
    let mut token = String::from("\"");
    while i < chars.len() {
        let ch = chars[i];
        token.push(ch);
        if ch == '\\' && i + 1 < chars.len() {
            token.push(chars[i + 1]);
            i += 2;
            continue;
        }
        if ch == '"' {
            return (token, i + 1);
        }
        i += 1;
    }
    (token, i)
}

fn take_while(chars: &[char], start: usize, pred: impl Fn(char) -> bool) -> (String, usize) {
    let mut i = start;
    let mut token = String::new();
    while i < chars.len() && pred(chars[i]) {
        token.push(chars[i]);
        i += 1;
    }
    (token, i)
}

fn skip_ws(chars: &[char], mut i: usize) -> usize {
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    i
}

fn starts_with(chars: &[char], i: usize, word: &str) -> bool {
    let w: Vec<char> = word.chars().collect();
    if i + w.len() > chars.len() {
        return false;
    }
    chars[i..i + w.len()] == w[..]
        && (i + w.len() == chars.len() || !chars[i + w.len()].is_ascii_alphanumeric())
}

fn span(class: &str, text: &str) -> String {
    format!("<span class=\"{class}\">{}</span>", escape_html(text))
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_js_string(text: &str) -> String {
    format!("`{}`", text.replace('\\', "\\\\").replace('`', "\\`").replace("${", "\\${"))
}

#[cfg(test)]
mod tests {
    use super::{escape_html, highlight_json, render_html};
    use crate::format::FormatKind;

    #[test]
    fn escapes_html() {
        assert_eq!(escape_html("<tag>"), "&lt;tag&gt;");
    }

    #[test]
    fn colors_json_keys() {
        let html = highlight_json("{\"name\":1}");
        assert!(html.contains("class=\"k\""));
        assert!(html.contains("class=\"n\""));
    }

    #[test]
    fn page_has_copy_button() {
        let page = render_html("{\"a\":1}", FormatKind::Json);
        assert!(page.contains("Copy to clipboard"));
        assert!(page.contains("navigator.clipboard.writeText"));
    }
}
