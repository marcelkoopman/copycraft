const MAX_LABEL_CHARS: usize = 48;
const MAX_HISTORY: usize = 20;
const MAX_PREVIEW_LINES: usize = 24;
const MAX_PREVIEW_LINE_CHARS: usize = 72;

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

    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            Self::Empty | Self::NoText => None,
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::Empty => "(clipboard is empty)".to_string(),
            Self::NoText => "(clipboard has no text)".to_string(),
            Self::Text(text) => one_line(text),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct ClipboardHistory {
    entries: Vec<String>,
}

impl ClipboardHistory {
    pub fn record(&mut self, text: String) {
        if text.trim().is_empty() {
            return;
        }
        self.entries.retain(|existing| existing != &text);
        self.entries.insert(0, text);
        self.entries.truncate(MAX_HISTORY);
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(String::as_str)
    }

    pub fn labels(&self) -> Vec<(usize, String)> {
        self.entries
            .iter()
            .enumerate()
            .map(|(i, text)| (i, one_line(text)))
            .collect()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

pub fn write_clipboard(text: &str) -> Result<(), String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    cb.set_text(text.to_string()).map_err(|e| e.to_string())
}

pub fn try_format_json(text: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(text.trim()).ok()?;
    if !value.is_object() && !value.is_array() {
        return None;
    }
    serde_json::to_string_pretty(&value).ok()
}

pub fn formatted(text: &str) -> String {
    try_format_json(text).unwrap_or_else(|| text.to_string())
}

pub fn preview_lines(text: &str) -> Vec<String> {
    let body = formatted(text);
    let mut lines = Vec::new();
    for raw in body.lines() {
        if lines.len() >= MAX_PREVIEW_LINES {
            break;
        }
        let line = if raw.is_empty() { " " } else { raw };
        if line.chars().count() <= MAX_PREVIEW_LINE_CHARS {
            lines.push(line.to_string());
        } else {
            let take: String = line.chars().take(MAX_PREVIEW_LINE_CHARS).collect();
            lines.push(take);
        }
    }
    if body.lines().count() > MAX_PREVIEW_LINES {
        lines.truncate(MAX_PREVIEW_LINES.saturating_sub(1));
        lines.push("...".to_string());
    }
    if lines.is_empty() {
        lines.push("(empty)".to_string());
    }
    lines
}

pub fn one_line(text: &str) -> String {
    let first = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("(empty)");
    let collapsed: String = first.split_whitespace().collect::<Vec<_>>().join(" ");
    let kind = classify(text);
    let raw = if kind.is_empty() {
        collapsed
    } else {
        format!("{kind} {collapsed}")
    };
    truncate_label(&raw)
}

fn classify(text: &str) -> &'static str {
    if try_format_json(text).is_some() {
        "json"
    } else if text.trim_start().starts_with("http://") || text.trim_start().starts_with("https://")
    {
        "url"
    } else if text.trim_start().starts_with('<') {
        "xml"
    } else if text.contains('\n') {
        "text"
    } else {
        ""
    }
}

fn truncate_label(label: &str) -> String {
    if label.chars().count() <= MAX_LABEL_CHARS {
        return label.to_string();
    }
    let mut out: String = label.chars().take(MAX_LABEL_CHARS.saturating_sub(3)).collect();
    out.push_str("...");
    out
}

#[cfg(test)]
mod tests {
    use super::{ClipboardHistory, one_line, preview_lines, try_format_json};

    #[test]
    fn one_line_uses_first_nonempty_line() {
        assert_eq!(one_line("\n  Hello world  \nmore"), "Hello world");
    }

    #[test]
    fn one_line_marks_json() {
        let label = one_line("{\"name\":\"copycraft\"}");
        assert!(label.starts_with("json "));
    }

    #[test]
    fn formats_json_when_valid() {
        let pretty = try_format_json("{\"name\":\"copycraft\"}").expect("json");
        assert!(pretty.contains('\n'));
        assert!(pretty.contains("copycraft"));
    }

    #[test]
    fn ignores_non_json() {
        assert_eq!(try_format_json("hello"), None);
        assert_eq!(try_format_json("123"), None);
    }

    #[test]
    fn preview_shows_pretty_json_lines() {
        let lines = preview_lines("{\"a\":1}");
        assert!(lines.iter().any(|l| l.contains('{')));
        assert!(lines.len() >= 2);
    }

    #[test]
    fn one_line_truncates() {
        let label = one_line(&"a".repeat(80));
        assert!(label.chars().count() <= 48);
        assert!(label.ends_with("..."));
    }

    #[test]
    fn history_dedupes_and_moves_to_front() {
        let mut history = ClipboardHistory::default();
        history.record("one".into());
        history.record("two".into());
        history.record("one".into());
        assert_eq!(history.get(0), Some("one"));
        assert_eq!(history.get(1), Some("two"));
        assert_eq!(history.labels().len(), 2);
    }
}
