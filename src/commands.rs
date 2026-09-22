use crate::appearance::Theme;
use crate::compress;
use crate::decode;
use crate::format;
use crate::toolbar_visibility;

pub const MAX_VISIBLE: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubjectKind {
    Empty,
    NoText,
    Image,
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hist {
    pub index: usize,
    pub title: String,
    pub mark: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchData {
    pub subject_kind: SubjectKind,
    pub subject_text: Option<String>,
    pub history: Vec<Hist>,
    pub can_clear_history: bool,
    pub menu_bar_shown: bool,
    pub theme: Theme,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandId {
    Preview,
    Format,
    Convert,
    Decode,
    Compress,
    Redact,
    Dataframe,
    History(usize),
    ClearClipboard,
    ClearHistory,
    ToggleMenuBar,
    Appearance(Theme),
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub id: CommandId,
    pub title: String,
    pub detail: String,
    keywords: String,
    in_default: bool,
    pinned: bool,
}

impl Command {
    fn matches(&self, needle: &str) -> bool {
        self.title.to_lowercase().contains(needle)
            || self.detail.to_lowercase().contains(needle)
            || self.keywords.to_lowercase().contains(needle)
    }
}

pub fn snippet(text: &str) -> String {
    const MAX: usize = 72;
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = flat.chars();
    let body: String = chars.by_ref().take(MAX).collect();
    if chars.next().is_some() {
        format!("{body}…")
    } else {
        body
    }
}

pub fn context_line(data: &LaunchData) -> String {
    match data.subject_kind {
        SubjectKind::Empty => "Clipboard is empty".to_string(),
        SubjectKind::NoText => "Clipboard has no text".to_string(),
        SubjectKind::Image => "Image".to_string(),
        SubjectKind::Text => {
            let text = data.subject_text.as_deref().unwrap_or("");
            let heading = format::detect(text).source_heading();
            let body = snippet(text);
            if body.is_empty() {
                heading.to_string()
            } else {
                format!("{heading}   {body}")
            }
        }
    }
}

pub fn list(data: &LaunchData) -> Vec<Command> {
    let mut commands = Vec::new();
    push_actions(&mut commands, data);
    push_history(&mut commands, data);
    push_settings(&mut commands, data);
    commands
}

pub fn visible(commands: &[Command], query: &str) -> Vec<Command> {
    let q = query.trim();
    let matched: Vec<Command> = if q.is_empty() {
        commands
            .iter()
            .filter(|cmd| cmd.in_default)
            .cloned()
            .collect()
    } else {
        let needle = q.to_lowercase();
        commands
            .iter()
            .filter(|cmd| cmd.matches(&needle))
            .cloned()
            .collect()
    };
    if !q.is_empty() {
        return matched.into_iter().take(MAX_VISIBLE).collect();
    }
    let (mut rest, mut pinned): (Vec<Command>, Vec<Command>) =
        matched.into_iter().partition(|cmd| !cmd.pinned);
    let room = MAX_VISIBLE.saturating_sub(pinned.len());
    rest.truncate(room);
    rest.append(&mut pinned);
    rest
}

fn push_actions(commands: &mut Vec<Command>, data: &LaunchData) {
    match data.subject_kind {
        SubjectKind::Image => {
            commands.push(command(
                CommandId::Preview,
                "Preview",
                "Image",
                "preview image open",
                true,
                false,
            ));
        }
        SubjectKind::Text => {
            let text = data.subject_text.as_deref().unwrap_or("");
            let kind = format::detect(text);
            if toolbar_visibility::shows_format(text) {
                commands.push(command(
                    CommandId::Format,
                    "Format",
                    kind.source_heading(),
                    "format pretty print",
                    true,
                    false,
                ));
            }
            if toolbar_visibility::shows_convert(text) {
                commands.push(command(
                    CommandId::Convert,
                    "Convert",
                    "Another format",
                    "convert yaml json csv",
                    true,
                    false,
                ));
            }
            if toolbar_visibility::shows_decode(kind) && decode::try_decode(text).is_some() {
                commands.push(command(
                    CommandId::Decode,
                    "Decode",
                    "Encoded text",
                    "decode jwt base64 percent",
                    true,
                    false,
                ));
            }
            if toolbar_visibility::shows_redact(kind, text) {
                commands.push(command(
                    CommandId::Redact,
                    "Redact",
                    "Hide sensitive values",
                    "redact pii email phone",
                    true,
                    false,
                ));
            }
            if toolbar_visibility::shows_dataframe_button(kind, text) {
                commands.push(command(
                    CommandId::Dataframe,
                    "Dataframe",
                    "Table",
                    "dataframe table csv tsv",
                    true,
                    false,
                ));
            }
            if toolbar_visibility::shows_compress(kind) && compress::is_large_enough(text) {
                commands.push(command(
                    CommandId::Compress,
                    "Compress",
                    "Shorter prompt",
                    "compress prompt shorten",
                    true,
                    false,
                ));
            }
            commands.push(command(
                CommandId::Preview,
                "Preview",
                kind.source_heading(),
                "preview open original",
                true,
                false,
            ));
            commands.push(command(
                CommandId::ClearClipboard,
                "Clear clipboard",
                "Empty the pasteboard",
                "clear clipboard empty",
                true,
                false,
            ));
        }
        SubjectKind::Empty | SubjectKind::NoText => {}
    }
}

fn push_history(commands: &mut Vec<Command>, data: &LaunchData) {
    for item in &data.history {
        commands.push(command(
            CommandId::History(item.index),
            &item.title,
            &item.mark,
            "history",
            true,
            false,
        ));
    }
    if data.can_clear_history {
        commands.push(command(
            CommandId::ClearHistory,
            "Clear history",
            "Forget copies",
            "clear history forget",
            true,
            false,
        ));
    }
}

fn push_settings(commands: &mut Vec<Command>, data: &LaunchData) {
    for theme in [Theme::System, Theme::Light, Theme::Dark] {
        let name = match theme {
            Theme::System => "System",
            Theme::Light => "Light",
            Theme::Dark => "Dark",
        };
        let detail = if data.theme == theme {
            "Appearance · current"
        } else {
            "Appearance"
        };
        let keywords = match theme {
            Theme::System => "appearance theme system",
            Theme::Light => "appearance theme light",
            Theme::Dark => "appearance theme dark",
        };
        commands.push(command(
            CommandId::Appearance(theme),
            name,
            detail,
            keywords,
            false,
            false,
        ));
    }
    if data.menu_bar_shown {
        commands.push(command(
            CommandId::ToggleMenuBar,
            "Hide menu bar icon",
            "Free the menu bar",
            "menu bar badge icon hide",
            true,
            true,
        ));
    } else {
        commands.push(command(
            CommandId::ToggleMenuBar,
            "Show menu bar icon",
            "Optional badge",
            "menu bar badge icon show",
            true,
            true,
        ));
    }
    commands.push(command(
        CommandId::Quit,
        "Quit",
        "Quit Copycraft",
        "quit exit",
        true,
        true,
    ));
}

fn command(
    id: CommandId,
    title: &str,
    detail: &str,
    keywords: &str,
    in_default: bool,
    pinned: bool,
) -> Command {
    Command {
        id,
        title: title.to_string(),
        detail: detail.to_string(),
        keywords: keywords.to_string(),
        in_default,
        pinned,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CommandId, Hist, LaunchData, MAX_VISIBLE, SubjectKind, context_line, list, snippet, visible,
    };
    use crate::appearance::Theme;

    fn data(kind: SubjectKind, text: Option<&str>) -> LaunchData {
        LaunchData {
            subject_kind: kind,
            subject_text: text.map(str::to_string),
            history: Vec::new(),
            can_clear_history: false,
            menu_bar_shown: false,
            theme: Theme::System,
        }
    }

    fn ids(commands: &[super::Command]) -> Vec<CommandId> {
        commands.iter().map(|cmd| cmd.id.clone()).collect()
    }

    #[test]
    fn messy_json_leads_with_format_and_hides_convert() {
        let shown = visible(
            &list(&data(SubjectKind::Text, Some(r#"{"name":"copycraft"}"#))),
            "",
        );
        assert_eq!(shown[0].id, CommandId::Format);
        assert!(shown.iter().any(|cmd| cmd.id == CommandId::Preview));
        assert!(!shown.iter().any(|cmd| cmd.id == CommandId::Convert));
        assert!(!shown.iter().any(|cmd| cmd.id == CommandId::Dataframe));
        assert!(!shown.iter().any(|cmd| cmd.id == CommandId::Decode));
        assert_eq!(shown[shown.len() - 2].id, CommandId::ToggleMenuBar);
        assert_eq!(shown.last().unwrap().id, CommandId::Quit);
        assert_eq!(shown[shown.len() - 2].title, "Show menu bar icon");
    }

    #[test]
    fn pretty_json_skips_format() {
        let pretty = "{\n  \"name\": \"copycraft\"\n}";
        let shown = visible(&list(&data(SubjectKind::Text, Some(pretty))), "");
        assert!(!shown.iter().any(|cmd| cmd.id == CommandId::Format));
        assert_eq!(shown[0].id, CommandId::Preview);
        assert!(!shown.iter().any(|cmd| cmd.id == CommandId::Convert));
    }

    #[test]
    fn yaml_offers_convert() {
        let shown = visible(
            &list(&data(
                SubjectKind::Text,
                Some("name: copycraft\ncount: 2\n"),
            )),
            "",
        );
        assert!(shown.iter().any(|cmd| cmd.id == CommandId::Convert));
    }

    #[test]
    fn encoded_text_offers_decode() {
        let shown = visible(
            &list(&data(
                SubjectKind::Text,
                Some("eyJuYW1lIjoiY29weWNyYWZ0In0="),
            )),
            "",
        );
        assert!(shown.iter().any(|cmd| cmd.id == CommandId::Decode));
    }

    #[test]
    fn prose_with_email_offers_redact() {
        let shown = visible(
            &list(&data(
                SubjectKind::Text,
                Some("mail me at jan.devries@email.nl please"),
            )),
            "",
        );
        assert!(shown.iter().any(|cmd| cmd.id == CommandId::Redact));
    }

    #[test]
    fn long_prose_offers_compress() {
        let text = "please summarize these notes for the team. ".repeat(40);
        let shown = visible(&list(&data(SubjectKind::Text, Some(&text))), "");
        assert!(shown.iter().any(|cmd| cmd.id == CommandId::Compress));
    }

    #[test]
    fn image_is_preview_only() {
        let shown = visible(&list(&data(SubjectKind::Image, None)), "");
        assert_eq!(shown[0].id, CommandId::Preview);
        assert!(!shown.iter().any(|cmd| cmd.id == CommandId::Format));
        assert!(!shown.iter().any(|cmd| cmd.id == CommandId::Convert));
        assert!(!shown.iter().any(|cmd| cmd.id == CommandId::ClearClipboard));
    }

    #[test]
    fn empty_clipboard_keeps_quit_and_the_badge_toggle() {
        let shown = visible(&list(&data(SubjectKind::Empty, None)), "");
        assert_eq!(ids(&shown), vec![CommandId::ToggleMenuBar, CommandId::Quit]);
        let line = context_line(&data(SubjectKind::Empty, None));
        assert_eq!(line, "Clipboard is empty");
    }

    #[test]
    fn hidden_badge_is_the_default_command_copy() {
        let mut input = data(SubjectKind::Empty, None);
        assert_eq!(
            list(&input)
                .iter()
                .find(|cmd| cmd.id == CommandId::ToggleMenuBar)
                .unwrap()
                .title,
            "Show menu bar icon"
        );
        input.menu_bar_shown = true;
        assert_eq!(
            list(&input)
                .iter()
                .find(|cmd| cmd.id == CommandId::ToggleMenuBar)
                .unwrap()
                .title,
            "Hide menu bar icon"
        );
    }

    #[test]
    fn query_finds_quit_appearance_and_history() {
        let mut input = data(SubjectKind::Text, Some("hello world"));
        input.history.push(Hist {
            index: 3,
            title: "notes from yesterday".into(),
            mark: "¶".into(),
        });
        let commands = list(&input);
        assert_eq!(ids(&visible(&commands, "quit")), vec![CommandId::Quit]);
        assert_eq!(
            ids(&visible(&commands, "dark")),
            vec![CommandId::Appearance(Theme::Dark)]
        );
        assert!(
            visible(&commands, "")
                .iter()
                .all(|cmd| !matches!(cmd.id, CommandId::Appearance(_)))
        );
        assert_eq!(
            ids(&visible(&commands, "yesterday")),
            vec![CommandId::History(3)]
        );
    }

    #[test]
    fn default_list_reserves_the_last_rows_for_the_badge_and_quit() {
        let mut input = data(SubjectKind::Text, Some("hello world"));
        for index in 0..10 {
            input.history.push(Hist {
                index,
                title: format!("older {index}"),
                mark: "¶".into(),
            });
        }
        let shown = visible(&list(&input), "");
        assert_eq!(shown.len(), MAX_VISIBLE);
        assert_eq!(shown[shown.len() - 1].id, CommandId::Quit);
        assert_eq!(shown[shown.len() - 2].id, CommandId::ToggleMenuBar);
        assert!(shown.iter().any(|cmd| cmd.id == CommandId::History(0)));
        assert!(!shown.iter().any(|cmd| cmd.id == CommandId::History(9)));
        assert!(
            visible(&list(&input), "older 9")
                .iter()
                .any(|cmd| cmd.id == CommandId::History(9))
        );
    }

    #[test]
    fn snippet_flattens_and_truncates() {
        assert_eq!(snippet("  hello\nworld  "), "hello world");
        let long = "word ".repeat(40);
        let cut = snippet(&long);
        assert!(cut.ends_with('…'));
        assert!(cut.chars().count() <= 73);
    }

    #[test]
    fn context_line_names_the_kind_and_a_snippet() {
        let line = context_line(&data(SubjectKind::Text, Some("{\"a\": 1}")));
        assert!(line.starts_with("JSON"));
        assert!(line.contains("{\"a\": 1}"));
    }
}
