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
    let width = delimited_field_count(lines[0], sep);
    if width < 2 {
        return false;
    }
    let sample_len = lines.len().min(20);
    let consistent = lines
        .iter()
        .take(20)
        .filter(|line| delimited_field_count(line, sep) == width)
        .count();
    consistent * 2 >= sample_len && !looks_like_key_value_blob(text)
}

fn delimited_field_count(line: &str, sep: char) -> usize {
    let mut fields = 1usize;
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '"' {
            if in_quotes && chars.peek() == Some(&'"') {
                chars.next();
            } else {
                in_quotes = !in_quotes;
            }
        } else if ch == sep && !in_quotes {
            fields += 1;
        }
    }
    fields
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
    fn formats_comma_csv() {
        let src = "name,age\nalice,30\nbob,40";
        let out = try_format(src).expect("csv");
        assert!(out.contains("name"));
        assert!(out.contains("alice"));
        assert!(super::looks_like_csv(src));
        assert!(!super::looks_like_tsv(src));
    }

    #[test]
    fn formats_csv_with_quoted_commas() {
        let src = "\
Id,Naam,Geboortedatum,Adres,Telefoonnummer,Salaris
1,Jan de Vries,1984-05-12,\"Hoofdstraat 45, Groningen\",06-12345678,3450
2,Anja Bakker,1991-11-23,\"Kerkplein 2, Utrecht\",06-87654321,2900
3,Mohammed El Amin,1978-02-05,\"Stationstraat 120, Rotterdam\",06-11223344,4200";
        let out = try_format(src).expect("quoted csv");
        assert!(out.contains("Naam"));
        assert!(out.contains("Jan de Vries"));
        assert!(out.contains("Groningen"));
        assert!(super::looks_like_csv(src));
        assert_eq!(
            super::delimited_field_count(src.lines().nth(1).unwrap(), ','),
            6
        );
    }

    #[test]
    fn formats_tsv() {
        let src = "name\tage\nalice\t30\nbob\t40";
        let out = try_format(src).expect("tsv");
        assert!(out.contains("name"));
        assert!(out.contains("alice"));
        assert!(super::looks_like_tsv(src));
        assert!(!super::looks_like_csv(src));
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
    fn exports_csv_as_json_rows() {
        let out = super::try_json_text("name,age\nalice,30\nbob,40").expect("json");
        assert!(out.contains("alice"));
        assert!(out.contains("name"));
        assert!(out.trim_start().starts_with('['));
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
