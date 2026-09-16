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
    let consistent = lines
        .iter()
        .take(20)
        .filter(|line| line.split(sep).count() == width)
        .count();
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
    fn exports_semicolon_csv() {
        let src = "\
Id;Naam;Salaris
1;Jan;3450
2;Anja;2900";
        let out = super::try_csv_text(src).expect("csv");
        assert!(out.contains("Naam"));
        assert!(out.contains("Jan"));
        assert!(out.contains(',') || out.contains(';'));
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
