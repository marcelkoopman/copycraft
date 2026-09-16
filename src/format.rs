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
