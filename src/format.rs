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
    Dataframe,
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
            Self::Dataframe => "DF",
            Self::Text => "¶",
            Self::Plain => "Aa",
        }
    }

    pub fn preview_heading(self) -> &'static str {
        match self {
            Self::Json => "Formatted JSON",
            Self::Yaml => "YAML",
            Self::Rust => "Formatted Rust",
            Self::Java => "Formatted Java",
            Self::Xml => "Formatted XML",
            Self::Dataframe => "Dataframe",
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
            Self::Dataframe => "csv",
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
            Self::Dataframe => Some([100, 210, 255, 255]),
            Self::Text | Self::Plain => None,
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
        FormatKind::Url | FormatKind::Dataframe | FormatKind::Text | FormatKind::Plain => {
            text.to_string()
        }
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
