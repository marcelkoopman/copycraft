const MAX_MENU_CHARS: usize = 800;
const MAX_LINE_CHARS: usize = 72;
const MAX_LINES: usize = 12;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardView {
    Empty,
    NoText,
    Text(String),
}

impl ClipboardView {
    pub fn from_os() -> Self {
        match arboard::Clipboard::new() {
            Ok(mut cb) => match cb.get_text() {
                Ok(text) if text.trim().is_empty() => Self::Empty,
                Ok(text) => Self::Text(text),
                Err(_) => Self::NoText,
            },
            Err(_) => Self::NoText,
        }
    }

    pub fn menu_lines(&self) -> Vec<String> {
        match self {
            Self::Empty => vec!["(clipboard is empty)".to_string()],
            Self::NoText => vec!["(clipboard has no text)".to_string()],
            Self::Text(text) => preview_lines(text),
        }
    }
}

pub fn preview_lines(text: &str) -> Vec<String> {
    let clipped: String = text.chars().take(MAX_MENU_CHARS).collect();
    let truncated_total = text.chars().count() > MAX_MENU_CHARS;

    let mut lines = Vec::new();
    for raw in clipped.lines() {
        if lines.len() >= MAX_LINES {
            break;
        }
        let line = raw.replace('\t', "    ");
        if line.chars().count() <= MAX_LINE_CHARS {
            lines.push(if line.is_empty() {
                " ".to_string()
            } else {
                line
            });
            continue;
        }
        let mut rest: String = line;
        while !rest.is_empty() && lines.len() < MAX_LINES {
            let take: String = rest.chars().take(MAX_LINE_CHARS).collect();
            rest = rest.chars().skip(MAX_LINE_CHARS).collect();
            lines.push(take);
        }
    }

    if lines.is_empty() {
        lines.push("(clipboard is empty)".to_string());
    }
    if truncated_total || text.lines().count() > MAX_LINES {
        lines.truncate(MAX_LINES.saturating_sub(1));
        lines.push(…".to_string());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::preview_lines;

    #[test]
    fn empty_text_becomes_placeholder() {
        assert_eq!(preview_lines(""), vec!["(clipboard is empty)"]);
    }

    #[test]
    fn wraps_long_line() {
        let long = "a".repeat(80);
        let lines = preview_lines(&long);
        assert!(lines[0].len() <= 72);
        assert!(lines.len() >= 2);
    }

    #[test]
    fn caps_line_count() {
        let text = (0..40).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
        let lines = preview_lines(&text);
        assert!(lines.len() <= 12);
        assert_eq!(lines.last().unwrap(), "…");
    }
}
