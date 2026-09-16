use super::{TokenKind, take_string, take_while};

pub(crate) fn looks_redacted(source: &str) -> bool {
    source.contains('[') && source.contains(']') && redact_tag_at(source, source.find('[').unwrap_or(0)).is_some()
}

pub(crate) fn tokenize_yaml(source: &str) -> Vec<(TokenKind, String)> {
    let mut out = Vec::new();
    let chars: Vec<char> = source.chars().collect();
    let mut i = 0;
    let mut line_start = true;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '\n' {
            out.push((TokenKind::Text, "\n".into()));
            i += 1;
            line_start = true;
            continue;
        }
        if line_start && starts_at(&chars, i, "---") {
            out.push((TokenKind::Punct, "---".into()));
            i += 3;
            line_start = false;
            continue;
        }
        if ch == '#' {
            let (token, next) = take_while(&chars, i, |c| c != '\n');
            out.push((TokenKind::Comment, token));
            i = next;
            continue;
        }
        if ch == '"' || ch == '\'' {
            let quote = ch;
            let mut j = i + 1;
            let mut token = String::from(ch);
            while j < chars.len() && chars[j] != quote {
                token.push(chars[j]);
                j += 1;
            }
            if j < chars.len() {
                token.push(chars[j]);
                j += 1;
            }
            out.push((TokenKind::String, token));
            i = j;
            line_start = false;
            continue;
        }
        if ch.is_ascii_digit() || (ch == '-' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit()) {
            let (token, next) = take_while(&chars, i, |c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+'));
            out.push((TokenKind::Number, token));
            i = next;
            line_start = false;
            continue;
        }
        if ch.is_ascii_alphabetic() || ch == '_' {
            let (token, next) = take_while(&chars, i, |c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'));
            let after = skip_ws(&chars, next);
            let kind = if after < chars.len() && chars[after] == ':' {
                TokenKind::Key
            } else if matches!(token.as_str(), "true" | "false" | "null" | "yes" | "no") {
                TokenKind::Keyword
            } else {
                TokenKind::Text
            };
            out.push((kind, token));
            i = next;
            line_start = false;
            continue;
        }
        if matches!(ch, ':' | '-' | '[' | ']' | '{' | '}' | ',') {
            out.push((TokenKind::Punct, ch.to_string()));
            i += 1;
            line_start = false;
            continue;
        }
        out.push((TokenKind::Text, ch.to_string()));
        i += 1;
        if !ch.is_whitespace() {
            line_start = false;
        }
    }
    out
}

pub(crate) fn tokenize_dataframe(source: &str) -> Vec<(TokenKind, String)> {
    let mut out = Vec::new();
    for (idx, line) in source.split_inclusive('\n').enumerate() {
        tokenize_df_line(&mut out, line, idx == 0);
    }
    if out.is_empty() {
        out.push((TokenKind::Text, source.to_string()));
    }
    out
}

fn tokenize_df_line(out: &mut Vec<(TokenKind, String)>, line: &str, first: bool) {
    if first && line.starts_with("shape:") {
        out.push((TokenKind::Keyword, "shape:".into()));
        out.push((TokenKind::Number, line["shape:".len()..].to_string()));
        return;
    }
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    let mut col = 0usize;
    while i < chars.len() {
        let ch = chars[i];
        if is_box(ch) {
            out.push((TokenKind::Punct, ch.to_string()));
            i += 1;
            continue;
        }
        if ch == '"' {
            let (token, next) = take_string(&chars, i);
            out.push((TokenKind::String, token));
            i = next;
            continue;
        }
        if ch.is_ascii_digit() || (ch == '-' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit()) {
            let (token, next) = take_while(&chars, i, |c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+'));
            out.push((TokenKind::Number, token));
            i = next;
            continue;
        }
        if ch.is_ascii_alphabetic() || ch == '_' {
            let (token, next) = take_while(&chars, i, |c| c.is_ascii_alphanumeric() || matches!(c, '_' | ':'));
            let kind = if matches!(token.as_str(), "str" | "i64" | "u64" | "i32" | "f64" | "f32" | "bool" | "date" | "datetime" | "null") {
                TokenKind::Type
            } else if col == 0 || looks_header_row(line) {
                TokenKind::Key
            } else {
                TokenKind::String
            };
            out.push((kind, token));
            i = next;
            col += 1;
            continue;
        }
        out.push((TokenKind::Text, ch.to_string()));
        i += 1;
    }
}

fn looks_header_row(line: &str) -> bool {
    line.contains('\u{2500}') || line.contains('\u{2502}') || line.contains('\u{253c}')
        || (line.contains("---") && line.contains('\u{2502}'))
}

fn is_box(ch: char) -> bool {
    matches!(ch, '‘─'..='╿' | '‘═'..='╬')
        || matches!(ch, '‘│' | '‘─' | '‘┼' | '‘┬' | '‘┴' | '‘├' | '‘┤' | '‘┌' | '‘┐' | '‘└' | '‘┘' | '‘║' | '‘═' | '‘╠' | '‘╣' | '‘╦' | '‘╩' | '‘╔' | '‘╗' | '‘╚' | '‘╝')
        || ch == '‘│' || ch == 'τ' || ch == '‘│'
}

// τ is greek tau used by polars as column sep in some fonts; also ascii '|'
// Fix is_box: include '|'

pub(crate) fn tokenize_redacted(source: &str) -> Vec<(TokenKind, String)> {
    let chars: Vec<char> = source.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '[' {
            if let Some((tag, end)) = redact_tag_chars(&chars, i) {
                out.push((TokenKind::Keyword, tag));
                i = end;
                continue;
            }
        }
        if chars[i] == '"' {
            let (token, next) = take_string(&chars, i);
            out.push((TokenKind::String, token));
            i = next;
            continue;
        }
        if chars[i].is_ascii_alphabetic() || chars[i] == '_' {
            let (token, next) = take_while(&chars, i, |c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'));
            let after = skip_ws(&chars, next);
            let kind = if after < chars.len() && chars[after] == ':' {
                TokenKind::Key
            } else {
                TokenKind::Text
            };
            out.push((kind, token));
            i = next;
            continue;
        }
        if chars[i].is_ascii_digit() {
            let (token, next) = take_while(&chars, i, |c| c.is_ascii_digit() || c == '.');
            out.push((TokenKind::Number, token));
            i = next;
            continue;
        }
        out.push((TokenKind::Text, chars[i].to_string()));
        i += 1;
    }
    out
}

fn redact_tag_at(source: &str, start: usize) -> Option<&str> {
    let rest = source.get(start..)?;
    let end = rest.find(']')?;
    let tag = &rest[..=end];
    if tag.len() >= 3 && tag.chars().all(|c| c == '[' || c == ']' || c.is_ascii_uppercase() || c == '_') {
        Some(tag)
    } else {
        None
    }
}

fn redact_tag_chars(chars: &[char], i: usize) -> Option<(String, usize)> {
    if chars[i] != '[' {
        return None;
    }
    let mut j = i + 1;
    while j < chars.len() && chars[j] != ']' {
        if !(chars[j].is_ascii_uppercase() || chars[j] == '_') {
            return None;
        }
        j += 1;
    }
    if j >= chars.len() || j == i + 1 {
        return None;
    }
    Some((chars[i..=j].iter().collect(), j + 1))
}

fn starts_at(chars: &[char], i: usize, s: &str) -> bool {
    let w: Vec<char> = s.chars().collect();
    i + w.len() <= chars.len() && chars[i..i + w.len()] == w[..]
}

fn skip_ws(chars: &[char], mut i: usize) -> usize {
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    i
}
