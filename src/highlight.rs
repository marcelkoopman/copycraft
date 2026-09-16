use crate::format::FormatKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Key,
    String,
    Number,
    Keyword,
    Function,
    Type,
    Macro,
    Comment,
    Punct,
    Text,
}

pub fn tokens(source: &str, kind: FormatKind) -> Vec<(TokenKind, String)> {
    match kind {
        FormatKind::Json => tokenize_json(source),
        FormatKind::Yaml => tokenize_yaml(source),
        FormatKind::Rust => tokenize_rust(source),
        FormatKind::Java => tokenize_code(source, kind),
        FormatKind::Xml => tokenize_xml(source),
        FormatKind::Dataframe => tokenize_dataframe(source),
        _ if looks_redacted(source) => tokenize_redacted(source),
        _ => vec![(TokenKind::Text, source.to_string())],
    }
}

fn tokenize_xml(source: &str) -> Vec<(TokenKind, String)> {
    let chars: Vec<char> = source.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '<' {
            if starts_prefix(&chars, i, "<!--") {
                let mut j = i + 4;
                while j + 2 < chars.len()
                    && !(chars[j] == '-' && chars[j + 1] == '-' && chars[j + 2] == '>')
                {
                    j += 1;
                }
                j = (j + 3).min(chars.len());
                out.push((TokenKind::Comment, chars[i..j].iter().collect()));
                i = j;
                continue;
            }
            let mut j = i + 1;
            while j < chars.len() && chars[j] != '>' {
                j += 1;
            }
            if j >= chars.len() {
                out.push((TokenKind::Text, chars[i..].iter().collect()));
                break;
            }
            let tag: String = chars[i..=j].iter().collect();
            color_xml_tag(&mut out, &tag);
            i = j + 1;
            continue;
        }
        let mut j = i;
        while j < chars.len() && chars[j] != '<' {
            j += 1;
        }
        out.push((TokenKind::Text, chars[i..j].iter().collect()));
        i = j;
    }
    out
}

fn color_xml_tag(out: &mut Vec<(TokenKind, String)>, tag: &str) {
    let chars: Vec<char> = tag.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '"' || ch == '\'' {
            let quote = ch;
            let mut j = i + 1;
            while j < chars.len() && chars[j] != quote {
                j += 1;
            }
            if j < chars.len() {
                j += 1;
            }
            out.push((TokenKind::String, chars[i..j].iter().collect()));
            i = j;
            continue;
        }
        if ch == '<' || ch == '>' || ch == '/' || ch == '?' || ch == '!' {
            out.push((TokenKind::Punct, ch.to_string()));
            i += 1;
            continue;
        }
        if ch.is_ascii_alphabetic() || ch == '_' || ch == ':' {
            let (token, next) = take_while(&chars, i, |c| {
                c.is_ascii_alphanumeric() || matches!(c, '_' | ':' | '-')
            });
            let kind = if (i > 0 && chars[i - 1] == '<')
                || (i > 1 && chars[i - 1] == '/' && chars[i - 2] == '<')
            {
                TokenKind::Keyword
            } else {
                TokenKind::Key
            };
            out.push((kind, token));
            i = next;
            continue;
        }
        out.push((TokenKind::Text, ch.to_string()));
        i += 1;
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

fn tokenize_rust(source: &str) -> Vec<(TokenKind, String)> {
    let keywords = [
        "fn", "let", "mut", "pub", "impl", "struct", "enum", "match", "if", "else", "use", "mod",
        "return", "async", "await", "self", "Self", "crate", "const", "static", "as", "where",
        "for", "in", "loop", "while", "break", "continue", "ref", "move",
    ];
    let mut out = Vec::new();
    let chars: Vec<char> = source.chars().collect();
    let mut i = 0;
    let mut after_fn = false;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
            let (token, next) = take_while(&chars, i, |c| c != '\n');
            out.push((TokenKind::Comment, token));
            i = next;
            continue;
        }
        if ch == '"' {
            let (token, next) = take_string(&chars, i);
            out.push((TokenKind::String, token));
            i = next;
            after_fn = false;
            continue;
        }
        if ch.is_ascii_alphabetic() || ch == '_' {
            let (mut token, mut next) =
                take_while(&chars, i, |c| c.is_ascii_alphanumeric() || c == '_');
            if next < chars.len() && chars[next] == '!' {
                token.push('!');
                next += 1;
                out.push((TokenKind::Macro, token));
                after_fn = false;
                i = next;
                continue;
            }
            let kind = if after_fn {
                after_fn = false;
                TokenKind::Function
            } else if keywords.contains(&token.as_str()) {
                after_fn = token == "fn";
                TokenKind::Keyword
            } else if token.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
                TokenKind::Type
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
            after_fn = false;
            continue;
        }
        if !ch.is_whitespace() {
            after_fn = false;
        }
        out.push((TokenKind::Text, ch.to_string()));
        i += 1;
    }
    out
}

fn tokenize_code(source: &str, kind: FormatKind) -> Vec<(TokenKind, String)> {
    let keywords: &[&str] = match kind {
        FormatKind::Java => &[
            "public",
            "private",
            "protected",
            "class",
            "static",
            "void",
            "int",
            "long",
            "boolean",
            "return",
            "if",
            "else",
            "new",
            "package",
            "import",
            "final",
            "this",
        ],
        _ => &[],
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
            } else if token.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
                TokenKind::Type
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

fn starts_prefix(chars: &[char], i: usize, prefix: &str) -> bool {
    let w: Vec<char> = prefix.chars().collect();
    i + w.len() <= chars.len() && chars[i..i + w.len()] == w[..]
}

fn starts_with(chars: &[char], i: usize, word: &str) -> bool {
    let w: Vec<char> = word.chars().collect();
    if i + w.len() > chars.len() {
        return false;
    }
    chars[i..i + w.len()] == w[..]
        && (i + w.len() == chars.len() || !chars[i + w.len()].is_ascii_alphanumeric())
}

include!("highlight_extra.rs");

#[cfg(test)]
mod tests {
    use super::{TokenKind, tokens};
    use crate::format::FormatKind;

    #[test]
    fn json_marks_keys_and_numbers() {
        let toks = tokens("{\"name\":1}", FormatKind::Json);
        assert!(
            toks.iter()
                .any(|(k, v)| *k == TokenKind::Key && v.contains("name"))
        );
        assert!(
            toks.iter()
                .any(|(k, v)| *k == TokenKind::Number && v == "1")
        );
    }

    #[test]
    fn rust_marks_fn_name_and_macro() {
        let toks = tokens("fn main() { eprintln!(\"x\"); }", FormatKind::Rust);
        assert!(
            toks.iter()
                .any(|(k, v)| *k == TokenKind::Function && v == "main")
        );
        assert!(
            toks.iter()
                .any(|(k, v)| *k == TokenKind::Macro && v == "eprintln!")
        );
    }

    #[test]
    fn xml_marks_tags_and_attrs() {
        let toks = tokens("<root id=\"1\"><!--x--></root>", FormatKind::Xml);
        assert!(
            toks.iter()
                .any(|(k, v)| *k == TokenKind::Keyword && v == "root")
        );
        assert!(toks.iter().any(|(k, v)| *k == TokenKind::Key && v == "id"));
        assert!(
            toks.iter()
                .any(|(k, v)| *k == TokenKind::String && v.contains('1'))
        );
        assert!(toks.iter().any(|(k, _)| *k == TokenKind::Comment));
    }

    #[test]
    fn yaml_marks_keys() {
        let toks = tokens("name: copycraft\ncount: 2\n", FormatKind::Yaml);
        assert!(
            toks.iter()
                .any(|(k, v)| *k == TokenKind::Key && v == "name")
        );
        assert!(
            toks.iter()
                .any(|(k, v)| *k == TokenKind::Number && v == "2")
        );
    }

    #[test]
    fn dataframe_marks_shape_and_numbers() {
        let src =
            "shape: (2, 2)\n\u{2502} Id \u{2502} n \u{2502}\n\u{2502} 1 \u{2502} 9 \u{2502}\n";
        let toks = tokens(src, FormatKind::Dataframe);
        assert!(
            toks.iter()
                .any(|(k, v)| *k == TokenKind::Keyword && v.starts_with("shape"))
        );
        assert!(
            toks.iter()
                .any(|(k, v)| *k == TokenKind::Number && v.trim() == "1")
        );
    }

    #[test]
    fn redacted_marks_placeholders() {
        let toks = tokens("Naam: [PERSON]\nmail: [EMAIL_ADDRESS]\n", FormatKind::Plain);
        assert!(
            toks.iter()
                .any(|(k, v)| *k == TokenKind::Keyword && v == "[PERSON]")
        );
        assert!(
            toks.iter()
                .any(|(k, v)| *k == TokenKind::Keyword && v == "[EMAIL_ADDRESS]")
        );
        assert!(
            toks.iter()
                .any(|(k, v)| *k == TokenKind::Key && v == "Naam")
        );
    }
}
