use std::io::Cursor;

use polars::prelude::*;

pub fn try_format(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    try_csv(trimmed)
        .or_else(|| try_json(trimmed))
        .or_else(|| try_xml(trimmed))
        .and_then(render)
}

fn render(df: DataFrame) -> Option<String> {
    if df.width() == 0 || df.height() == 0 {
        return None;
    }
    Some(df.to_string())
}

fn try_csv(text: &str) -> Option<DataFrame> {
    let separator = detect_separator(text)?;
    if !looks_like_delimited_table(text, separator) {
        return None;
    }
    let mut cursor = Cursor::new(text.as_bytes());
    let parse = CsvParseOptions::default().with_separator(separator);
    CsvReadOptions::default()
        .with_has_header(true)
        .with_parse_options(parse)
        .into_reader_with_file_handle(&mut cursor)
        .finish()
        .ok()
        .filter(|df| df.width() >= 2 && df.height() >= 1)
}

fn try_json(text: &str) -> Option<DataFrame> {
    let trimmed = text.trim_start();
    if !(trimmed.starts_with('[') || trimmed.starts_with('{')) {
        return None;
    }
    if trimmed.starts_with('{') && !trimmed.contains("[") {
        return None;
    }
    let mut cursor = Cursor::new(text.as_bytes());
    JsonReader::new(&mut cursor)
        .finish()
        .ok()
        .filter(|df| df.width() >= 1 && df.height() >= 1)
        .or_else(|| try_json_object_of_arrays(text))
}

fn try_json_object_of_arrays(text: &str) -> Option<DataFrame> {
    let value: serde_json::Value = serde_json::from_str(text.trim()).ok()?;
    let serde_json::Value::Object(map) = value else {
        return None;
    };
    if map.is_empty() {
        return None;
    }
    let mut columns = Vec::new();
    let mut height = None;
    for (key, val) in map {
        let serde_json::Value::Array(items) = val else {
            return None;
        };
        if let Some(expected) = height {
            if items.len() != expected {
                return None;
            }
        } else {
            height = Some(items.len());
        }
        let series = series_from_json_values(&key, &items)?;
        columns.push(series.into_column());
    }
    let height = height?;
    DataFrame::new(height, columns).ok().filter(|df| df.height() >= 1)
}

fn series_from_json_values(name: &str, items: &[serde_json::Value]) -> Option<Series> {
    if items.iter().all(|v| v.is_i64() || v.is_u64() || v.is_null()) {
        let values: Vec<Option<i64>> = items
            .iter()
            .map(|v| {
                if v.is_null() {
                    None
                } else {
                    v.as_i64().or_else(|| v.as_u64().map(|n| n as i64))
                }
            })
            .collect();
        return Some(Series::new(name.into(), values));
    }
    if items.iter().all(|v| v.is_f64() || v.is_i64() || v.is_u64() || v.is_null()) {
        let values: Vec<Option<f64>> = items
            .iter()
            .map(|v| {
                if v.is_null() {
                    None
                } else {
                    v.as_f64()
                        .or_else(|| v.as_i64().map(|n| n as f64))
                        .or_else(|| v.as_u64().map(|n| n as f64))
                }
            })
            .collect();
        return Some(Series::new(name.into(), values));
    }
    if items.iter().all(|v| v.is_boolean() || v.is_null()) {
        let values: Vec<Option<bool>> = items.iter().map(|v| v.as_bool()).collect();
        return Some(Series::new(name.into(), values));
    }
    let values: Vec<Option<String>> = items
        .iter()
        .map(|v| {
            if v.is_null() {
                None
            } else if let Some(s) = v.as_str() {
                Some(s.to_string())
            } else {
                Some(v.to_string())
            }
        })
        .collect();
    Some(Series::new(name.into(), values))
}

fn try_xml(text: &str) -> Option<DataFrame> {
    if !crate::format::looks_like_xml(text) {
        return None;
    }
    let rows = xml_rows(text)?;
    if rows.len() < 2 {
        return None;
    }
    let headers = rows[0].clone();
    if headers.len() < 2 {
        return None;
    }
    let csv = rows_to_csv(&rows);
    try_csv(&csv)
}

fn xml_rows(text: &str) -> Option<Vec<Vec<String>>> {
    let body = text.trim();
    let children = top_level_children(body)?;
    if children.len() < 2 {
        return None;
    }
    let row_tag = most_common_tag(&children)?;
    let row_blocks: Vec<&str> = children
        .into_iter()
        .filter(|(tag, _)| *tag == row_tag)
        .map(|(_, block)| block)
        .collect();
    if row_blocks.len() < 2 {
        return None;
    }
    let mut headers: Vec<String> = Vec::new();
    let mut values: Vec<Vec<String>> = Vec::new();
    for block in row_blocks {
        let leaves = leaf_fields(block);
        if leaves.is_empty() {
            return None;
        }
        if headers.is_empty() {
            headers = leaves.iter().map(|(k, _)| k.clone()).collect();
        }
        let row: Vec<String> = headers
            .iter()
            .map(|header| {
                leaves
                    .iter()
                    .find(|(k, _)| k == header)
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default()
            })
            .collect();
        values.push(row);
    }
    let mut rows = vec![headers];
    rows.extend(values);
    Some(rows)
}

fn top_level_children(text: &str) -> Option<Vec<(&str, &str)>> {
    let start = text.find('<')?;
    let mut i = start;
    let bytes = text.as_bytes();
    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        if text[i..].starts_with("<?") || text[i..].starts_with("<!") {
            i = text[i..].find('>').map(|n| i + n + 1).unwrap_or(bytes.len());
            continue;
        }
        break;
    }
    let open_end = text[i..].find('>')? + i;
    let open = &text[i + 1..open_end];
    if open.starts_with('/') || open.ends_with('/') {
        return None;
    }
    let root_name = open.split_whitespace().next()?.trim_end_matches('/');
    let close = format!("</{root_name}>");
    let close_at = text.rfind(&close)?;
    let inner = &text[open_end + 1..close_at];
    extract_direct_children(inner)
}

fn extract_direct_children(inner: &str) -> Option<Vec<(&str, &str)>> {
    let mut out = Vec::new();
    let mut i = 0usize;
    let bytes = inner.as_bytes();
    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        if bytes[i] != b'<' {
            return None;
        }
        if inner[i..].starts_with("</") {
            break;
        }
        let tag_end = inner[i + 1..].find([' ', '>', '/'])? + i + 1;
        let tag = &inner[i + 1..tag_end];
        let open_close = inner[i..].find('>')? + i;
        if inner[..open_close].ends_with('/') {
            out.push((tag, &inner[i..open_close + 1]));
            i = open_close + 1;
            continue;
        }
        let close = format!("</{tag}>");
        let rel = inner[open_close + 1..].find(&close)?;
        let end = open_close + 1 + rel + close.len();
        out.push((tag, &inner[i..end]));
        i = end;
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn most_common_tag<'a>(children: &[(&'a str, &str)]) -> Option<&'a str> {
    let mut best: Option<(&str, usize)> = None;
    for (tag, _) in children {
        let count = children.iter().filter(|(t, _)| t == tag).count();
        if best.map(|(_, c)| count > c).unwrap_or(true) {
            best = Some((*tag, count));
        }
    }
    best.map(|(tag, _)| tag)
}

fn leaf_fields(block: &str) -> Vec<(String, String)> {
    let Some(children) = extract_direct_children(inner_of_element(block).unwrap_or("")) else {
        return Vec::new();
    };
    children
        .into_iter()
        .filter_map(|(tag, elem)| {
            let inner = inner_of_element(elem)?;
            if inner.contains('<') {
                return None;
            }
            Some((tag.to_string(), decode_basic_entities(inner.trim())))
        })
        .collect()
}

fn inner_of_element(elem: &str) -> Option<&str> {
    let open_end = elem.find('>')?;
    if elem[..open_end].ends_with('/') {
        return Some("");
    }
    let close_start = elem.rfind("</")?;
    Some(&elem[open_end + 1..close_start])
}

fn decode_basic_entities(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

fn rows_to_csv(rows: &[Vec<String>]) -> String {
    rows.iter()
        .map(|row| {
            row.iter()
                .map(|cell| escape_csv_cell(cell))
                .collect::<Vec<_>>()
                .join(",")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn escape_csv_cell(cell: &str) -> String {
    if cell.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", cell.replace('"', "\"\""))
    } else {
        cell.to_string()
    }
}

fn detect_separator(text: &str) -> Option<u8> {
    let header = text.lines().map(str::trim).find(|line| !line.is_empty())?;
    let semis = header.matches(';').count();
    let commas = header.matches(',').count();
    let tabs = header.matches('\t').count();
    if tabs > 0 && tabs >= semis && tabs >= commas {
        Some(b'\t')
    } else if semis >= 1 && semis >= commas {
        Some(b';')
    } else if commas >= 1 {
        Some(b',')
    } else {
        None
    }
}

fn looks_like_delimited_table(text: &str, separator: u8) -> bool {
    let sep = separator as char;
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    if lines.len() < 2 {
        return false;
    }
    let width = lines[0].split(sep).count();
    if width < 2 {
        return false;
    }
    let consistent = lines.iter().take(20).filter(|line| line.split(sep).count() == width).count();
    consistent * 2 >= lines.iter().take(20).count().min(lines.len())
        && !looks_like_key_value_blob(text)
}

fn looks_like_key_value_blob(text: &str) -> bool {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    if lines.len() < 2 {
        return false;
    }
    let labeled = lines
        .iter()
        .filter(|line| {
            line.split_once(':')
                .map(|(k, v)| !k.trim().is_empty() && !v.trim().is_empty() && !k.contains(';'))
                .unwrap_or(false)
        })
        .count();
    labeled * 2 >= lines.len()
}

#[cfg(test)]
mod tests {
    use super::try_format;

    #[test]
    fn formats_semicolon_csv() {
        let src = "\
Id;Naam;Salaris
1;Jan;3450
2;Anja;2900";
        let out = try_format(src).expect("df");
        assert!(out.contains("Naam"));
        assert!(out.contains("Jan"));
        assert!(out.contains("3450"));
    }

    #[test]
    fn formats_json_array() {
        let src = r#"[{"name":"a","n":1},{"name":"b","n":2}]"#;
        let out = try_format(src).expect("df");
        assert!(out.contains("name"));
        assert!(out.contains("a"));
    }

    #[test]
    fn rejects_plain_text() {
        assert!(try_format("just a sentence about nothing").is_none());
        assert!(try_format("Naam: Jan de Vries\nSalaris: 3450").is_none());
    }

    #[test]
    fn formats_simple_xml_rows() {
        let src = r#"
<root>
  <person><name>Jan</name><salary>3450</salary></person>
  <person><name>Anja</name><salary>2900</salary></person>
</root>"#;
        let out = try_format(src).expect("df");
        assert!(out.contains("name"));
        assert!(out.contains("Jan"));
        assert!(out.contains("3450"));
    }
}
