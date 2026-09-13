use crate::format::FormatKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Key,
    String,
    Number,
    Keyword,
    Punct,
    Text,
}

pub fn tokens(source: &str, kind: FormatKind) -> Vec<(TokenKind, String)> {
    match kind {
        FormatKind::Json => tokenize_json(source),
        FormatKind::Rust | FormatKind::Java => tokenize_code(source, kind),
        _ => vec![(TokenKind::Text, source.to_string())],
    }
}

fn tokenize_json(source: &str) -> Vec<(TokenKind, String)> {
    let mut out = Vec::new();
    let chars: Vec<char> = source.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '"' {
            let (token, next) = take_string(&chars, i);
            let after = skip_ws(&chars, next);
            let kind = if after < chars.len() && chars[after] == ':' {
                TokenKind::Key
            } else {
                TokenKind::String
            };
            out.push((kind, token));
            i = next;
            continue;
        }
        if ch.is_ascii_digit()
            || (ch == '-' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit())
        {
            let (token, next) = take_while(&chars, i, |c| {
                c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E')
            });
            out.push((TokenKind::Number, token));
            i = next;
            continue;
        }
        if starts_with(&chars, i, "true")
            || starts_with(&chars, i, "false")
            || starts_with(&chars, i, "null")
        {
            let word = if starts_with(&chars, i, "true") {
                "true"
            } else if starts_with(&chars, i, "false") {
                "false"
            } else {
                "null"
            };
            out.push((TokenKind::Keyword, word.to_string()));
            i += word.chars().count();
            continue;
        }
        if matches!(ch, '{' | '}' | '[' | ']' | ':' | ',') {
            out.push((TokenKind::Punct, ch.to_string()));
            i += 1;
            continue;
        }
        out.push((TokenKind::Text, ch.to_string()));
        i += 1;
    }
    out
}

fn tokenize_code(source: &str, kind: FormatKind) -> Vec<(TokenKind, String)> {
    let keywords: &[&str] = match kind {
        FormatKind::Rust => &[
            "fn", "let", "mut", "pub", "impl", "struct", "enum", "match", "if", "else", "use",
            "mod", "return", "async", "await", "self", "Self", "crate", "const", "static",
        ],
        _ => &[
            "public", "private", "protected", "class", "static", "void", "int", "long",
            "boolean", "return", "if", "else", "new", "package", "import", "final", "this",
        ],
    };
    let mut out = Vec::new();
    let chars: Vec<char> = source.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '"' {
            let (token, next) = take_string(&chars, i);
            out.push((TokenKind::String, token));
            i = next;
            continue;
        }
        if ch.is_ascii_alphabetic() || ch == '_' {
            let (token, next) = take_while(&chars, i, |c| c.is_ascii_alphanumeric() || c == '_');
            let kind = if keywords.contains(&token.as_str()) {
                TokenKind::Keyword
            } else {
                TokenKind::Text
            };
            out.push((kind, token));
            i = next;
            continue;
        }
        if ch.is_ascii_digit() {
            let (token, next) = take_while(&chars, i, |c| c.is_ascii_digit() || c == '.');
            out.push((TokenKind::Number, token));
            i = next;
            continue;
        }
        out.push((TokenKind::Text, ch.to_string()));
        i += 1;
    }
    out
}

fn take_string(chars: &[char], start: usize) -> (String, usize) {
    let mut i = start + 1;
    let mut token = String::from("\"");
    while i < chars.len() {
        let ch = chars[i];
        token.push(ch);
        if ch == '\\' && i + 1 < chars.len() {
            token.push(chars[i + 1]);
            i += 2;
            continue;
        }
        if ch == '"' {
            return (token, i + 1);
        }
        i += 1;
    }
    (token, i)
}

fn take_while(chars: &[char], start: usize, pred: impl Fn(char) -> bool) -> (String, usize) {
    let mut i = start;
    let mut token = String::new();
    while i < chars.len() && pred(chars[i]) {
        token.push(chars[i]);
        i += 1;
    }
    (token, i)
}

fn skip_ws(chars: &[char], mut i: usize) -> usize {
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    i
}

fn starts_with(chars: &[char], i: usize, word: &str) -> bool {
    let w: Vec<char> = word.chars().collect();
    if i + w.len() > chars.len() {
        return false;
    }
    chars[i..i + w.len()] == w[..]
        && (i + w.len() == chars.len() || !chars[i + w.len()].is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::{TokenKind, tokens};
    use crate::format::FormatKind;

    #[test]
    fn json_marks_keys_and_numbers() {
        let toks = tokens("{\"name\":1}", FormatKind::Json);
        assert!(toks.iter().any(|(k, v)| *k == TokenKind::Key && v.contains("name")));
        assert!(toks.iter().any(|(k, v)| *k == TokenKind::Number && v == "1"));
    }
}
