use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatKind {
    Json,
    Rust,
    Java,
    Url,
    Xml,
    Text,
    Plain,
}

impl FormatKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Rust => "rust",
            Self::Java => "java",
            Self::Url => "url",
            Self::Xml => "xml",
            Self::Text | Self::Plain => "",
        }
    }

    pub fn preview_heading(self) -> &'static str {
        match self {
            Self::Json => "Formatted JSON",
            Self::Rust => "Formatted Rust",
            Self::Java => "Formatted Java",
            _ => "Content",
        }
    }
}

pub fn detect(text: &str) -> FormatKind {
    if crate::clipboard::try_format_json(text).is_some() {
        return FormatKind::Json;
    }
    let trimmed = text.trim_start();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return FormatKind::Url;
    }
    if trimmed.starts_with('<') {
        return FormatKind::Xml;
    }
    if looks_like_rust(text) {
        return FormatKind::Rust;
    }
    if looks_like_java(text) {
        return FormatKind::Java;
    }
    if text.contains('\n') {
        FormatKind::Text
    } else {
        FormatKind::Plain
    }
}

pub fn format_text(text: &str) -> String {
    match detect(text) {
        FormatKind::Json => {
            crate::clipboard::try_format_json(text).unwrap_or_else(|| text.to_string())
        }
        FormatKind::Rust => format_rust(text),
        FormatKind::Java => indent_braces(text),
        _ => text.to_string(),
    }
}

fn looks_like_rust(text: &str) -> bool {
    let rust_hits = [
        "fn ",
        "impl ",
        "pub fn",
        "let mut",
        "match ",
        "use crate",
        "#[derive",
    ];
    score(text, &rust_hits) >= 2
        || (text.contains("fn ") && (text.contains('{') || text.contains("->")))
}

fn looks_like_java(text: &str) -> bool {
    let java_hits = [
        "public class",
        "private class",
        "package ",
        "import java",
        "System.out",
        "public static void",
        "@Override",
    ];
    score(text, &java_hits) >= 1 && text.contains('{')
}

fn score(text: &str, needles: &[&str]) -> usize {
    needles.iter().filter(|n| text.contains(*n)).count()
}

fn format_rust(text: &str) -> String {
    rustfmt(text).unwrap_or_else(|| indent_braces(text))
}

fn rustfmt(text: &str) -> Option<String> {
    let mut child = Command::new("rustfmt")
        .args(["--emit", "stdout", "--edition", "2024", "--quiet"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    child.stdin.as_mut()?.write_all(text.as_bytes()).ok()?;
    let output = child.wait_with_output().ok()?;
    if !output.status.success() {
        return None;
    }
    let formatted = String::from_utf8(output.stdout).ok()?;
    if formatted.trim().is_empty() {
        None
    } else {
        Some(formatted)
    }
}

pub fn indent_braces(src: &str) -> String {
    let mut indent: i32 = 0;
    let mut out = String::new();
    for raw in src.lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            out.push('\n');
            continue;
        }
        let leading_close = trimmed.starts_with('}')
            || trimmed.starts_with(']')
            || trimmed.starts_with(");")
            || trimmed.starts_with(')');
        if leading_close {
            indent = (indent - 1).max(0);
        }
        for _ in 0..indent {
            out.push_str("    ");
        }
        out.push_str(trimmed);
        out.push('\n');
        let opens = count_open(trimmed);
        indent = (indent + opens).max(0);
    }
    if out.ends_with('\n') {
        out.pop();
    }
    out
}

fn count_open(line: &str) -> i32 {
    let mut depth = 0i32;
    let mut in_string = false;
    let mut prev = '\0';
    for ch in line.chars() {
        if ch == '"' && prev != '\\' {
            in_string = !in_string;
        } else if !in_string {
            match ch {
                '{' | '[' => depth += 1,
                '}' | ']' => depth -= 1,
                _ => {}
            }
        }
        prev = ch;
    }
    depth
}

#[cfg(test)]
mod tests {
    use super::{FormatKind, detect, indent_braces};

    #[test]
    fn detects_rust() {
        let src = "fn main() { let x = 1; }";
        assert_eq!(detect(src), FormatKind::Rust);
    }

    #[test]
    fn detects_java() {
        let src = "public class App { public static void main(String[] args) { } }";
        assert_eq!(detect(src), FormatKind::Java);
    }

    #[test]
    fn indents_braces() {
        let src = "fn main(){\nlet x=1;\n}";
        let out = indent_braces(src);
        assert!(out.contains("    let x=1;"));
        assert!(out.lines().last().unwrap().starts_with('}'));
    }
}
