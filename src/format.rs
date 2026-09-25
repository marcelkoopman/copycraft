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
    Csv,
    Tsv,
    Dataframe,
    Image,
    Text,
    Plain,
}

impl FormatKind {
    pub fn menu_symbol(self) -> &'static str {
        match self {
            Self::Json => "{}",
            Self::Yaml => "---",
            Self::Rust => "fn",
            Self::Java => "Jv",
            Self::Url => "://",
            Self::Xml => "</>",
            Self::Csv => "csv",
            Self::Tsv => "tsv",
            Self::Dataframe => "DF",
            Self::Image => "img",
            Self::Text => "¶",
            Self::Plain => "Aa",
        }
    }

    pub fn source_heading(self) -> &'static str {
        match self {
            Self::Json => "JSON",
            Self::Yaml => "YAML",
            Self::Rust => "Rust",
            Self::Java => "Java",
            Self::Url => "URL",
            Self::Xml => "XML",
            Self::Csv => "CSV",
            Self::Tsv => "TSV",
            Self::Dataframe => "Dataframe",
            Self::Image => "Image",
            Self::Text | Self::Plain => "Content",
        }
    }

    pub fn preview_heading(self) -> &'static str {
        match self {
            Self::Json => "Formatted JSON",
            Self::Yaml => "YAML",
            Self::Rust => "Formatted Rust",
            Self::Java => "Formatted Java",
            Self::Xml => "Formatted XML",
            Self::Csv => "CSV",
            Self::Tsv => "TSV",
            Self::Dataframe => "Dataframe",
            Self::Image => "Image",
            _ => "Content",
        }
    }

    pub fn suggested_extension(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Yaml => "yaml",
            Self::Rust => "rs",
            Self::Java => "java",
            Self::Url => "txt",
            Self::Xml => "xml",
            Self::Csv => "csv",
            Self::Tsv => "tsv",
            Self::Dataframe => "parquet",
            Self::Image => "png",
            Self::Text | Self::Plain => "txt",
        }
    }

    pub fn suggested_filename(self) -> String {
        format!("clipboard.{}", self.suggested_extension())
    }

    pub fn accent_rgba(self) -> Option<[u8; 4]> {
        match self {
            Self::Json => Some([245, 197, 66, 255]),
            Self::Yaml => Some([203, 123, 239, 255]),
            Self::Rust => Some([222, 165, 132, 255]),
            Self::Java => Some([231, 111, 0, 255]),
            Self::Url => Some([90, 200, 250, 255]),
            Self::Xml => Some([52, 199, 89, 255]),
            Self::Csv => Some([100, 210, 255, 255]),
            Self::Tsv => Some([64, 186, 232, 255]),
            Self::Dataframe => Some([100, 210, 255, 255]),
            Self::Image => Some([255, 126, 182, 255]),
            Self::Text | Self::Plain => None,
        }
    }
}

pub fn detect(text: &str) -> FormatKind {
    if crate::clipboard::try_format_json(text).is_some() {
        return FormatKind::Json;
    }
    if looks_like_rust(text) {
        return FormatKind::Rust;
    }
    if looks_like_java(text) {
        return FormatKind::Java;
    }
    if crate::dataframe::looks_like_tsv(text) {
        return FormatKind::Tsv;
    }
    if crate::dataframe::looks_like_csv(text) {
        return FormatKind::Csv;
    }
    if crate::transform::looks_like_yaml(text) {
        return FormatKind::Yaml;
    }
    if looks_like_url(text) {
        return FormatKind::Url;
    }
    if looks_like_xml(text) {
        return FormatKind::Xml;
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
        FormatKind::Url => format_url(text),
        FormatKind::Csv
        | FormatKind::Tsv
        | FormatKind::Dataframe
        | FormatKind::Image
        | FormatKind::Text
        | FormatKind::Plain => text.to_string(),
    }
}

pub(crate) fn looks_like_url(text: &str) -> bool {
    let text = text.trim();
    if text.is_empty() || text.contains(['\n', '\t']) {
        return false;
    }
    let rest = strip_http_scheme(text).unwrap_or(text);
    if rest.is_empty() || rest.starts_with('/') {
        return false;
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    if authority_host(authority).is_none() {
        return false;
    }
    let bare = strip_http_scheme(text).is_none() && !rest.contains('/') && !rest.contains('?');
    if bare && is_filename(authority) {
        return false;
    }
    true
}

fn strip_http_scheme(text: &str) -> Option<&str> {
    let (scheme, rest) = text.split_once("://")?;
    if scheme.eq_ignore_ascii_case("https") || scheme.eq_ignore_ascii_case("http") {
        Some(rest)
    } else {
        None
    }
}

fn authority_host(authority: &str) -> Option<&str> {
    if authority.is_empty() || authority.contains(' ') {
        return None;
    }
    let hostport = authority.rsplit('@').next().unwrap_or(authority);
    let host = match hostport.rsplit_once(':') {
        Some((host, port))
            if !host.is_empty()
                && !port.is_empty()
                && port.chars().all(|ch| ch.is_ascii_digit()) =>
        {
            host
        }
        Some(_) => return None,
        None => hostport,
    };
    is_url_host(host).then_some(host)
}

fn is_url_host(host: &str) -> bool {
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() < 2 {
        return false;
    }
    let tld = labels[labels.len() - 1];
    labels.iter().all(|label| {
        !label.is_empty()
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    }) && tld.len() >= 2
        && tld.chars().all(|ch| ch.is_ascii_alphabetic())
}

fn is_filename(name: &str) -> bool {
    let Some((_, ext)) = name.rsplit_once('.') else {
        return false;
    };
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "webp"
            | "pdf"
            | "zip"
            | "txt"
            | "json"
            | "xml"
            | "csv"
            | "mp4"
            | "mp3"
    )
}

/// Adds `https://` when the scheme is missing and percent-encodes characters
/// that are not allowed in a URI path or query parameter.
fn format_url(text: &str) -> String {
    let text = text.trim();
    let (scheme, rest) = match strip_http_scheme(text) {
        Some(rest) => {
            let scheme = if text.to_ascii_lowercase().starts_with("http://") {
                "http"
            } else {
                "https"
            };
            (scheme, rest)
        }
        None => ("https", text),
    };
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    let after = &rest[authority_end..];
    let (before_fragment, fragment) = split_marker(after, '#');
    let (path, query) = split_marker(before_fragment, '?');
    let mut out = String::new();
    out.push_str(scheme);
    out.push_str("://");
    out.push_str(authority);
    out.push_str(&encode_path(path));
    if let Some(query) = query {
        out.push('?');
        out.push_str(&encode_query(query));
    }
    if let Some(fragment) = fragment {
        out.push('#');
        out.push_str(&encode_component(fragment, fragment_byte));
    }
    out
}

fn split_marker(text: &str, marker: char) -> (&str, Option<&str>) {
    match text.split_once(marker) {
        Some((head, tail)) => (head, Some(tail)),
        None => (text, None),
    }
}

fn encode_path(path: &str) -> String {
    path.split('/')
        .map(|segment| encode_component(segment, query_byte))
        .collect::<Vec<_>>()
        .join("/")
}

fn encode_query(query: &str) -> String {
    query
        .split('&')
        .map(|part| match part.split_once('=') {
            Some((key, value)) => format!(
                "{}={}",
                encode_component(key, query_byte),
                encode_component(value, query_byte)
            ),
            None => encode_component(part, query_byte),
        })
        .collect::<Vec<_>>()
        .join("&")
}

fn encode_component(text: &str, allow: fn(u8) -> bool) -> String {
    let bytes = text.as_bytes();
    let mut out = String::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && bytes[index + 1].is_ascii_hexdigit()
            && bytes[index + 2].is_ascii_hexdigit()
        {
            out.push('%');
            out.push(bytes[index + 1] as char);
            out.push(bytes[index + 2] as char);
            index += 3;
            continue;
        }
        let byte = bytes[index];
        if allow(byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
        index += 1;
    }
    out
}

fn query_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'-' | b'.'
                | b'_'
                | b'~'
                | b'!'
                | b'$'
                | b'\''
                | b'('
                | b')'
                | b'*'
                | b'+'
                | b','
                | b';'
                | b':'
                | b'@'
        )
}

fn fragment_byte(byte: u8) -> bool {
    query_byte(byte) || byte == b'/' || byte == b'?'
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
    fn menu_symbols_are_type_marks() {
        use super::FormatKind::*;
        assert_eq!(Json.menu_symbol(), "{}");
        assert_eq!(Yaml.menu_symbol(), "---");
        assert_eq!(Rust.menu_symbol(), "fn");
        assert_eq!(Java.menu_symbol(), "Jv");
        assert_eq!(Url.menu_symbol(), "://");
        assert_eq!(Xml.menu_symbol(), "</>");
        assert_eq!(Csv.menu_symbol(), "csv");
        assert_eq!(Tsv.menu_symbol(), "tsv");
        assert_eq!(Dataframe.menu_symbol(), "DF");
        assert_eq!(Image.menu_symbol(), "img");
        assert_eq!(Text.menu_symbol(), "¶");
        assert_eq!(Plain.menu_symbol(), "Aa");
    }

    #[test]
    fn suggested_extensions_match_kind() {
        assert_eq!(FormatKind::Json.suggested_extension(), "json");
        assert_eq!(FormatKind::Yaml.suggested_extension(), "yaml");
        assert_eq!(FormatKind::Rust.suggested_extension(), "rs");
        assert_eq!(FormatKind::Java.suggested_extension(), "java");
        assert_eq!(FormatKind::Xml.suggested_extension(), "xml");
        assert_eq!(FormatKind::Csv.suggested_extension(), "csv");
        assert_eq!(FormatKind::Tsv.suggested_extension(), "tsv");
        assert_eq!(FormatKind::Dataframe.suggested_extension(), "parquet");
        assert_eq!(FormatKind::Image.suggested_extension(), "png");
        assert_eq!(FormatKind::Plain.suggested_extension(), "txt");
        assert_eq!(FormatKind::Csv.suggested_filename(), "clipboard.csv");
        assert_eq!(FormatKind::Tsv.suggested_filename(), "clipboard.tsv");
        assert_eq!(
            FormatKind::Dataframe.suggested_filename(),
            "clipboard.parquet"
        );
    }

    #[test]
    fn accent_rgba_for_typed_kinds() {
        assert_eq!(FormatKind::Rust.accent_rgba(), Some([222, 165, 132, 255]));
        assert_eq!(FormatKind::Image.accent_rgba(), Some([255, 126, 182, 255]));
        assert_eq!(FormatKind::Plain.accent_rgba(), None);
    }

    #[test]
    fn formats_a_url_with_a_scheme_and_encoded_parameters() {
        assert_eq!(
            super::format_text("example.com/search?q=hello world&city=New York"),
            "https://example.com/search?q=hello%20world&city=New%20York"
        );
        assert_eq!(
            super::format_text("http://example.com/my file?x=a/b"),
            "http://example.com/my%20file?x=a%2Fb"
        );
        assert_eq!(
            super::format_text("https://example.com/search?q=hello%20world"),
            "https://example.com/search?q=hello%20world"
        );
        assert_eq!(
            super::format_text("HTTPS://example.com/path"),
            "https://example.com/path"
        );
        assert_eq!(detect("www.example.com/a"), FormatKind::Url);
        assert_eq!(detect("notes.txt"), FormatKind::Plain);
        assert_eq!(detect("https://example.com/path"), FormatKind::Url);
    }

    #[test]
    fn detects_rust() {
        let src = "fn main() { let x = 1; }";
        assert_eq!(detect(src), FormatKind::Rust);
    }

    #[test]
    fn rust_module_list_is_not_csv() {
        let src = "\
mod appearance;
mod clipboard;
mod compress;
mod convert;
mod dataframe;
mod decode;
mod format;
mod highlight;
mod icon;
mod menubar;
mod preview;
mod redact;
mod settings;
mod toolbar_visibility;
mod transform;

#[cfg(target_os = \"macos\")]
mod macos_preview_text;
#[cfg(target_os = \"macos\")]
mod macos_window;

fn main() {
    if let Err(e) = menubar::run() {
        eprintln!(\"copycraft failed: {e}\");
        std::process::exit(1);
    }
}
";
        assert_eq!(detect(src), FormatKind::Rust);
        assert!(!crate::dataframe::looks_like_csv(src));
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
    fn detects_csv_and_tsv() {
        assert_eq!(detect("name,age\nalice,30\nbob,40"), FormatKind::Csv);
        assert_eq!(detect("name\tage\nalice\t30\nbob\t40"), FormatKind::Tsv);
        assert_eq!(
            detect("Id;Naam;Salaris\n1;Jan;3450\n2;Anja;2900"),
            FormatKind::Csv
        );
        assert_eq!(
            detect(
                "Id,Naam,Geboortedatum,Adres,Telefoonnummer,Salaris\n\
1,Jan de Vries,1984-05-12,\"Hoofdstraat 45, Groningen\",06-12345678,3450\n\
2,Anja Bakker,1991-11-23,\"Kerkplein 2, Utrecht\",06-87654321,2900"
            ),
            FormatKind::Csv
        );
        assert_eq!(detect("just a sentence"), FormatKind::Plain);
        assert_eq!(detect("hello\nworld"), FormatKind::Text);
        assert_eq!(FormatKind::Csv.preview_heading(), "CSV");
        assert_eq!(FormatKind::Tsv.preview_heading(), "TSV");
        assert_eq!(FormatKind::Dataframe.preview_heading(), "Dataframe");
        assert_eq!(FormatKind::Image.source_heading(), "Image");
        assert_eq!(FormatKind::Image.preview_heading(), "Image");
        assert_eq!(FormatKind::Rust.source_heading(), "Rust");
        assert_eq!(FormatKind::Rust.preview_heading(), "Formatted Rust");
        assert_eq!(FormatKind::Json.source_heading(), "JSON");
        assert_eq!(FormatKind::Java.source_heading(), "Java");
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

    #[test]
    fn blank_line_record_stays_text_after_redact() {
        let src = "\
Naam: Jan de Vries

Adres: Hoofdstraat 45, 9711 AB Groningen

E-mailadres: jan.devries@email.nl

Telefoonnummer: 06-12345678

Geboortedatum: 12 mei 1984

Salaris: € 3.450";
        let redacted = crate::redact::redact(src);
        assert!(redacted.contains("Adres:"));
        assert!(redacted.contains("Salaris:"));
        assert!(!redacted.contains("jan.devries@email.nl"));
        assert_ne!(detect(&redacted), FormatKind::Yaml);
        let formatted = super::format_text(&redacted);
        assert!(formatted.contains("Adres:"));
        assert!(formatted.contains("Telefoonnummer:"));
        assert!(!formatted.contains("jan.devries@email.nl"));
    }
}
