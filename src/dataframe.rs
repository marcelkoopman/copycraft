use std::io::Cursor;

use polars::prelude::*;

pub fn try_format(text: &str) -> Option<String> {
    parse(text).and_then(render)
}

pub fn try_csv_text(text: &str) -> Option<String> {
    parse(text).and_then(write_csv)
}

pub fn looks_like_csv(text: &str) -> bool {
    matches!(delimited_separator(text), Some(b',') | Some(b';'))
}

pub fn looks_like_tsv(text: &str) -> bool {
    delimited_separator(text) == Some(b'\t')
}

fn delimited_separator(text: &str) -> Option<u8> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let separator = detect_separator(trimmed)?;
    try_csv(trimmed)?;
    Some(separator)
}

fn parse(text: &str) -> Option<DataFrame> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    try_csv(trimmed)
        .or_else(|| try_json(trimmed))
        .or_else(|| try_xml(trimmed))
}

fn write_csv(mut df: DataFrame) -> Option<String> {
    if df.width() == 0 || df.height() == 0 {
        return None;
    }
    let mut buf = Vec::new();
    CsvWriter::new(&mut buf)
        .include_header(true)
        .finish(&mut df)
        .ok()?;
    String::from_utf8(buf).ok()
}

fn render(df: DataFrame) -> Option<String> {
    if df.width() == 0 || df.height() == 0 {
        return None;
    }
    Some(df.to_string())
}

include!("dataframe_parse.rs");
