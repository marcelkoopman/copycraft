use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatKind {
    Json,
    Yaml,
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
            Self::Yaml => "yaml",
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
            Self::Yaml => "YAML",
            Self::Rust => "Formatted Rust",
            Self::Java => "Formatted Java",
            Self::Xml => "Formatted XML",
            _ => "Content",
        }
    }
}

pub fn detect(text: &str) -> FormatKind {
    if crate::clipboard::try_format_json(text).is_some() {
        return FormatKind::Json;
    }
    if crate::transform::looks_like_yaml(text) {
        return FormatKind::Yaml;
    }
    let trimmed = text.trim_start();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return FormatKind::Url;
    }
    if looks_like_xml(text) {
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
        FormatKind::Yaml => {
            crate::transform::pretty_yaml(text).unwrap_or_else(|_| text.to_string())
        }
        FormatKind::Rust => format_rust(text),
        FormatKind::Java => indent_braces(text),
        FormatKind::Xml => pretty_xml(text),
        FormatKind::Url | FormatKind::Text | FormatKind::Plain => text.to_string(),
    }
}

pub fn looks_like_xml(text: &str) -> bool {
    let trimmed = text.trim_start();
    if !(trimmed.starts_with('<') && trimmed.contains('>')) {
        return false;
    }
    let lower = trimmed.to_ascii_lowercase();
    lower.starts_with("<?xml")
        || lower.starts_with("<!doctype")
        || lower.starts_with("<svg")
        || lower.starts_with("<html")
        || tag_balance(trimmed)
}

fn tag_balance(text: &str) -> bool {
    let mut depth = 0i32;
    let mut i = 0;
    let chars: Vec<char> = text.chars().collect();
    let mut saw_tag = false;
    while i < chars.len() {
        if chars[i] != '<' {
            i += 1;
            continue;
        }
        if starts_at(&chars, i, "<!--") {
            i += 4;
            while i + 2 < chars.len()
                && !(chars[i] == '-' && chars[i + 1] == '-' && chars[i + 2] == '>')
            {
                i += 1;
            }
            i = (i + 3).min(chars.len());
            continue;
        }
        if starts_at(&chars, i, "<?") || starts_at(&chars, i, "<!") {
            while i < chars.len() && chars[i] != '>' {
                i += 1;
            }
            if i < chars.len() {
                i += 1;
            }
            continue;
        }
        let closing = i + 1 < chars.len() && chars[i + 1] == '/';
        let mut j = i + 1 + usize::from(closing);
        while j < chars.len() && chars[j] != '>' {
            j += 1;
        }
        if j >= chars.len() {
            return false;
        }
        saw_tag = true;
        let self_close = j > 0 && chars[j - 1] == '/';
        if closing {
            depth -= 1;
        } else if !self_close {
            depth += 1;
        }
        i = j + 1;
    }
    saw_tag && depth >= 0
}

fn starts_at(chars: &[char], i: usize, s: &str) -> bool {
    let w: Vec<char> = s.chars().collect();
    i + w.len() <= chars.len() && chars[i..i + w.len()] == w[..]
}

pub fn pretty_xml(src: &str) -> String {
    let tokens = xml_tokens(src.trim());
    if tokens.is_empty() {
        return src.to_string();
    }
    let mut out = String::new();
    let mut indent: i32 = 0;
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            XmlToken::Decl(s) | XmlToken::Comment(s) | XmlToken::Empty(s) => {
                push_indent(&mut out, indent);
                out.push_str(s);
                out.push('\n');
                i += 1;
            }
            XmlToken::Open(open) => {
                if let Some((line, consumed)) = inline_leaf(&tokens, i) {
                    push_indent(&mut out, indent);
                    out.push_str(&line);
                    out.push('\n');
                    i += consumed;
                    continue;
                }
                push_indent(&mut out, indent);
                out.push_str(open);
                out.push('\n');
                indent += 1;
                i += 1;
            }
            XmlToken::Close(s) => {
                indent = (indent - 1).max(0);
                push_indent(&mut out, indent);
                out.push_str(s);
                out.push('\n');
                i += 1;
            }
            XmlToken::Text(s) => {
                let text = collapse_xml_text(s);
                if !text.is_empty() {
                    push_indent(&mut out, indent);
                    out.push_str(&text);
                    out.push('\n');
                }
                i += 1;
            }
        }
    }
    if out.ends_with('\n') {
        out.pop();
    }
    if out.trim().is_empty() {
        src.to_string()
    } else {
        out
    }
}

fn inline_leaf(tokens: &[XmlToken], i: usize) -> Option<(String, usize)> {
    let XmlToken::Open(open) = tokens.get(i)? else {
        return None;
    };
    match tokens.get(i + 1) {
        Some(XmlToken::Close(close)) => Some((format!("{open}{close}"), 2)),
        Some(XmlToken::Text(text)) => match tokens.get(i + 2) {
            Some(XmlToken::Close(close)) => {
                let t = collapse_xml_text(text);
                Some((format!("{open}{t}{close}"), 3))
            }
            _ => None,
        },
        _ => None,
    }
}

fn collapse_xml_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<&str>>().join(" ")
}

enum XmlToken {
    Decl(String),
    Comment(String),
    Open(String),
    Close(String),
    Empty(String),
    Text(String),
}

fn xml_tokens(src: &str) -> Vec<XmlToken> {
    let chars: Vec<char> = src.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '<' {
            if starts_at(&chars, i, "<!--") {
                let mut j = i + 4;
                while j + 2 < chars.len()
                    && !(chars[j] == '-' && chars[j + 1] == '-' && chars[j + 2] == '>')
                {
                    j += 1;
                }
                j = (j + 3).min(chars.len());
                tokens.push(XmlToken::Comment(chars[i..j].iter().collect()));
                i = j;
                continue;
            }
            let mut j = i + 1;
            while j < chars.len() && chars[j] != '>' {
                j += 1;
            }
            if j >= chars.len() {
                tokens.push(XmlToken::Text(chars[i..].iter().collect()));
                break;
            }
            j += 1;
            let tag: String = chars[i..j].iter().collect();
            if tag.starts_with("<?") || tag.starts_with("<!") {
                tokens.push(XmlToken::Decl(tag));
            } else if tag.starts_with("</") {
                tokens.push(XmlToken::Close(tag));
            } else if tag.ends_with("/>") {
                tokens.push(XmlToken::Empty(tag));
            } else {
                tokens.push(XmlToken::Open(tag));
            }
            i = j;
        } else {
            let mut j = i;
            while j < chars.len() && chars[j] != '<' {
                j += 1;
            }
            tokens.push(XmlToken::Text(chars[i..j].iter().collect()));
            i = j;
        }
    }
    tokens
}

fn push_indent(out: &mut String, indent: i32) {
    for _ in 0..indent {
        out.push_str("    ");
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
    use super::{FormatKind, detect, indent_braces, pretty_xml};

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
    fn detects_yaml() {
        let src = "name: copycraft\nitems:\n  - one\n";
        assert_eq!(detect(src), FormatKind::Yaml);
    }

    #[test]
    fn detects_xml() {
        assert_eq!(detect("<root><item/></root>"), FormatKind::Xml);
        assert_eq!(detect("<?xml version=\"1.0\"?><a></a>"), FormatKind::Xml);
    }

    #[test]
    fn pretty_prints_xml() {
        let out = pretty_xml("<root><item id=\"1\">hi</item><empty/></root>");
        assert!(out.contains("    <item id=\"1\">hi</item>"));
        assert!(out.contains("    <empty/>"));
        assert_eq!(out.lines().next().unwrap(), "<root>");
        assert!(!out.contains("\n        hi\n"));
    }

    #[test]
    fn pretty_prints_xml_leaf_tags_inline() {
        let src = r#"<?xml version="1.0" encoding="UTF-8"?>
<PensioenAangifteResponse>
    <Bericht>
        <RespSrt>
            ACK
        </RespSrt>
        <IdBer>
            123456
        </IdBer>
        <LhNr>
            012345678L01
        </LhNr>
        <Empty></Empty>
    </Bericht>
    <SysteemMelding>
        Het bestand heeft geen geldige extentie.
    </SysteemMelding>
</PensioenAangifteResponse>"#;
        let out = pretty_xml(src);
        assert!(out.contains("<RespSrt>ACK</RespSrt>"));
        assert!(out.contains("<IdBer>123456</IdBer>"));
        assert!(out.contains("<LhNr>012345678L01</LhNr>"));
        assert!(out.contains("<Empty></Empty>"));
        assert!(
            out.contains(
                "<SysteemMelding>Het bestand heeft geen geldige extentie.</SysteemMelding>"
            )
        );
        assert!(out.contains("    <Bericht>"));
        assert!(!out.contains("\n            ACK\n"));
        assert!(!out.contains("\n            012345678L01\n"));
        assert!(out.lines().next().unwrap().starts_with("<?xml"));
    }

    #[test]
    fn indents_braces() {
        let src = "fn main(){\nlet x=1;\n}";
        let out = indent_braces(src);
        assert!(out.contains("    let x=1;"));
        assert!(out.lines().last().unwrap().starts_with('}'));
    }
}
