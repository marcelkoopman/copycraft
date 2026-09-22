use crate::appearance::Theme;
use crate::compress;
use crate::decode;
use crate::format;
use crate::toolbar_visibility;

pub const MAX_VISIBLE: usize = 8;
pub const CHIP_PITCH: f64 = 34.0;
pub const CHIP_PILL_H: f64 = 28.0;

const CHIP_GAP: f64 = 6.0;
const EXCERPT_LINES: usize = 6;
const EXCERPT_LINE_CHARS: usize = 48;

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
pub struct ImageFacts {
    pub format: String,
    pub width: usize,
    pub height: usize,
    pub byte_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchData {
    pub subject_kind: SubjectKind,
    pub subject_text: Option<String>,
    pub image: Option<ImageFacts>,
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
    ImageBase64,
    ImageDataUrl,
    ImageFile,
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
}

impl Command {
    fn matches(&self, needle: &str) -> bool {
        self.title.to_lowercase().contains(needle)
            || self.detail.to_lowercase().contains(needle)
            || self.keywords.to_lowercase().contains(needle)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkCard {
    pub title: String,
    pub meta: String,
    pub excerpt: String,
    pub placeholder: String,
    pub shows_image: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChipFrame {
    pub x: f64,
    pub row: usize,
    pub width: f64,
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

/// The start of the clipboard text, kept on its own lines.
pub fn payload_excerpt(text: &str) -> String {
    let mut out = Vec::new();
    let mut extra = false;
    for (index, line) in text.lines().enumerate() {
        if index >= EXCERPT_LINES {
            extra = true;
            break;
        }
        out.push(clip_chars(line, EXCERPT_LINE_CHARS));
    }
    if extra
        && let Some(last) = out.last_mut()
        && !last.ends_with('…')
    {
        last.push('…');
    }
    out.join("\n")
}

pub fn work_card(data: &LaunchData) -> WorkCard {
    match data.subject_kind {
        SubjectKind::Image => WorkCard {
            title: "Image".to_string(),
            meta: data.image.as_ref().map(image_meta).unwrap_or_default(),
            excerpt: String::new(),
            placeholder: String::new(),
            shows_image: true,
        },
        SubjectKind::Text => {
            let text = data.subject_text.as_deref().unwrap_or("");
            WorkCard {
                title: format::detect(text).source_heading().to_string(),
                meta: text_meta(text),
                excerpt: payload_excerpt(text),
                placeholder: String::new(),
                shows_image: false,
            }
        }
        SubjectKind::Empty => WorkCard {
            title: "Clipboard".to_string(),
            meta: String::new(),
            excerpt: String::new(),
            placeholder: "Nothing copied".to_string(),
            shows_image: false,
        },
        SubjectKind::NoText => WorkCard {
            title: "Clipboard".to_string(),
            meta: String::new(),
            excerpt: String::new(),
            placeholder: "No text on the clipboard".to_string(),
            shows_image: false,
        },
    }
}

/// Actions for the thing on the clipboard. Housekeeping stays in [`overflow`].
pub fn chips(data: &LaunchData) -> Vec<Command> {
    match data.subject_kind {
        SubjectKind::Image => vec![
            command(CommandId::Preview, "Preview", "Open", "preview image open"),
            command(
                CommandId::ImageBase64,
                "Base64",
                "Encoded image",
                "base64 encode text",
            ),
            command(
                CommandId::ImageDataUrl,
                "Data URL",
                "data:image",
                "data url uri embed",
            ),
            command(
                CommandId::ImageFile,
                "File",
                "Paste as file",
                "file save path",
            ),
        ],
        SubjectKind::Text => text_chips(data.subject_text.as_deref().unwrap_or("")),
        SubjectKind::Empty | SubjectKind::NoText => Vec::new(),
    }
}

/// Chips, earlier copies, and appearance. Quit and the menu bar stay out.
pub fn search_pool(data: &LaunchData) -> Vec<Command> {
    let mut commands = chips(data);
    for item in &data.history {
        commands.push(command(
            CommandId::History(item.index),
            &item.title,
            &item.mark,
            "history",
        ));
    }
    for theme in [Theme::System, Theme::Light, Theme::Dark] {
        let name = theme_name(theme);
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
        ));
    }
    commands
}

pub fn matching(commands: &[Command], query: &str) -> Vec<Command> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }
    commands
        .iter()
        .filter(|cmd| cmd.matches(&needle))
        .take(MAX_VISIBLE)
        .cloned()
        .collect()
}

/// Quit, history, and the menu bar icon. Never mixed into [`chips`].
pub fn overflow(data: &LaunchData) -> Vec<Command> {
    let mut commands = vec![command(
        CommandId::ClearClipboard,
        "Clear clipboard",
        "Empty the pasteboard",
        "clear clipboard empty",
    )];
    if data.can_clear_history {
        commands.push(command(
            CommandId::ClearHistory,
            "Clear history",
            "Forget copies",
            "clear history forget",
        ));
    }
    if data.menu_bar_shown {
        commands.push(command(
            CommandId::ToggleMenuBar,
            "Hide menu bar icon",
            "Free the menu bar",
            "menu bar badge icon hide",
        ));
    } else {
        commands.push(command(
            CommandId::ToggleMenuBar,
            "Show menu bar icon",
            "Optional badge",
            "menu bar badge icon show",
        ));
    }
    for theme in [Theme::System, Theme::Light, Theme::Dark] {
        commands.push(command(
            CommandId::Appearance(theme),
            theme_name(theme),
            "Appearance",
            "appearance theme",
        ));
    }
    commands.push(command(
        CommandId::Quit,
        "Quit",
        "Quit Copycraft",
        "quit exit",
    ));
    commands
}

pub fn chip_width(title: &str) -> f64 {
    let chars = title.chars().count() as f64;
    (22.0 + chars * 7.4).clamp(52.0, 196.0)
}

pub fn layout_chips(titles: &[&str], width: f64) -> Vec<ChipFrame> {
    let mut frames = Vec::with_capacity(titles.len());
    let mut x = 0.0;
    let mut row = 0usize;
    for title in titles {
        let chip = chip_width(title);
        if x > 0.0 && x + chip > width {
            row += 1;
            x = 0.0;
        }
        frames.push(ChipFrame {
            x,
            row,
            width: chip,
        });
        x += chip + CHIP_GAP;
    }
    frames
}

pub fn chips_height(frames: &[ChipFrame]) -> f64 {
    if frames.is_empty() {
        0.0
    } else {
        (frames.iter().map(|frame| frame.row).max().unwrap_or(0) + 1) as f64 * CHIP_PITCH
    }
}

pub fn step_chip(frames: &[ChipFrame], index: usize, dx: isize, dy: isize) -> usize {
    if frames.is_empty() {
        return 0;
    }
    let index = index.min(frames.len() - 1);
    if dy == 0 {
        return (index as isize + dx).clamp(0, frames.len() as isize - 1) as usize;
    }
    let current = frames[index];
    let target = current.row as isize + dy;
    if target < 0 {
        return index;
    }
    let center = current.x + current.width / 2.0;
    frames
        .iter()
        .enumerate()
        .filter(|(_, frame)| frame.row == target as usize)
        .min_by(|(_, a), (_, b)| {
            let da = (a.x + a.width / 2.0 - center).abs();
            let db = (b.x + b.width / 2.0 - center).abs();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(next, _)| next)
        .unwrap_or(index)
}

pub fn keeps_card_open(id: &CommandId) -> bool {
    matches!(
        id,
        CommandId::ToggleMenuBar
            | CommandId::Appearance(_)
            | CommandId::ClearClipboard
            | CommandId::ClearHistory
            | CommandId::ImageBase64
            | CommandId::ImageDataUrl
            | CommandId::ImageFile
    )
}

fn text_chips(text: &str) -> Vec<Command> {
    let kind = format::detect(text);
    let mut commands = Vec::new();
    if toolbar_visibility::shows_format(text) {
        commands.push(command(
            CommandId::Format,
            "Format",
            kind.source_heading(),
            "format pretty print",
        ));
    }
    if toolbar_visibility::shows_convert(text) {
        commands.push(command(
            CommandId::Convert,
            "Convert",
            "Another format",
            "convert yaml json csv",
        ));
    }
    if toolbar_visibility::shows_decode(kind) && decode::try_decode(text).is_some() {
        commands.push(command(
            CommandId::Decode,
            "Decode",
            "Encoded text",
            "decode jwt base64 percent",
        ));
    }
    if toolbar_visibility::shows_redact(kind, text) {
        commands.push(command(
            CommandId::Redact,
            "Redact",
            "Hide sensitive values",
            "redact pii email phone",
        ));
    }
    if toolbar_visibility::shows_dataframe_button(kind, text) {
        commands.push(command(
            CommandId::Dataframe,
            "Dataframe",
            "Table",
            "dataframe table csv tsv",
        ));
    }
    if toolbar_visibility::shows_compress(kind) && compress::is_large_enough(text) {
        commands.push(command(
            CommandId::Compress,
            "Compress",
            "Shorter prompt",
            "compress prompt shorten",
        ));
    }
    commands.push(command(
        CommandId::Preview,
        "Preview",
        kind.source_heading(),
        "preview open original",
    ));
    commands
}

fn image_meta(facts: &ImageFacts) -> String {
    let mut parts = Vec::new();
    if !facts.format.is_empty() {
        parts.push(facts.format.clone());
    }
    if facts.width > 0 && facts.height > 0 {
        parts.push(format!("{}×{}", facts.width, facts.height));
    }
    if facts.byte_len > 0 {
        parts.push(format_bytes(facts.byte_len));
    }
    parts.join("  ")
}

fn text_meta(text: &str) -> String {
    let size = format_bytes(text.len());
    let lines = text.lines().count();
    if lines > 1 {
        format!("{lines} lines  {size}")
    } else {
        size
    }
}

fn format_bytes(n: usize) -> String {
    if n >= 1_000_000 {
        format_unit(n as f64 / 1_000_000.0, "MB")
    } else if n >= 1_000 {
        format_unit(n as f64 / 1_000.0, "KB")
    } else {
        format!("{n} B")
    }
}

fn format_unit(value: f64, unit: &str) -> String {
    if value >= 100.0 || (value - value.round()).abs() < 0.05 {
        format!("{:.0} {unit}", value.round())
    } else {
        format!("{value:.1} {unit}")
    }
}

fn clip_chars(text: &str, max: usize) -> String {
    let mut chars = text.chars();
    let body: String = chars.by_ref().take(max).collect();
    if chars.next().is_some() {
        format!("{body}…")
    } else {
        body
    }
}

fn theme_name(theme: Theme) -> &'static str {
    match theme {
        Theme::System => "System",
        Theme::Light => "Light",
        Theme::Dark => "Dark",
    }
}

fn command(id: CommandId, title: &str, detail: &str, keywords: &str) -> Command {
    Command {
        id,
        title: title.to_string(),
        detail: detail.to_string(),
        keywords: keywords.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CommandId, Hist, ImageFacts, LaunchData, SubjectKind, chips, layout_chips, matching,
        overflow, payload_excerpt, search_pool, snippet, step_chip, work_card,
    };
    use crate::appearance::Theme;

    fn data(kind: SubjectKind, text: Option<&str>) -> LaunchData {
        LaunchData {
            subject_kind: kind,
            subject_text: text.map(str::to_string),
            image: None,
            history: Vec::new(),
            can_clear_history: false,
            menu_bar_shown: false,
            theme: Theme::System,
        }
    }

    fn titles(commands: &[super::Command]) -> Vec<String> {
        commands.iter().map(|cmd| cmd.title.clone()).collect()
    }

    fn ids(commands: &[super::Command]) -> Vec<CommandId> {
        commands.iter().map(|cmd| cmd.id.clone()).collect()
    }

    fn housekeeping(id: &CommandId) -> bool {
        matches!(
            id,
            CommandId::Quit | CommandId::ToggleMenuBar | CommandId::ClearHistory
        )
    }

    #[test]
    fn messy_json_leads_with_format_and_hides_convert() {
        let input = data(SubjectKind::Text, Some(r#"{"name":"copycraft"}"#));
        let shown = chips(&input);
        assert_eq!(shown[0].id, CommandId::Format);
        assert_eq!(shown.last().unwrap().id, CommandId::Preview);
        assert!(!shown.iter().any(|cmd| cmd.id == CommandId::Convert));
        assert!(!shown.iter().any(|cmd| cmd.id == CommandId::Dataframe));
        assert!(!shown.iter().any(|cmd| cmd.id == CommandId::Decode));
        assert!(shown.iter().all(|cmd| !housekeeping(&cmd.id)));
    }

    #[test]
    fn pretty_json_skips_format() {
        let pretty = "{\n  \"name\": \"copycraft\"\n}";
        let shown = chips(&data(SubjectKind::Text, Some(pretty)));
        assert!(!shown.iter().any(|cmd| cmd.id == CommandId::Format));
        assert_eq!(shown[0].id, CommandId::Preview);
        assert!(!shown.iter().any(|cmd| cmd.id == CommandId::Convert));
    }

    #[test]
    fn yaml_offers_convert() {
        let shown = chips(&data(
            SubjectKind::Text,
            Some("name: copycraft\ncount: 2\n"),
        ));
        assert!(shown.iter().any(|cmd| cmd.id == CommandId::Convert));
    }

    #[test]
    fn encoded_text_offers_decode() {
        let shown = chips(&data(
            SubjectKind::Text,
            Some("eyJuYW1lIjoiY29weWNyYWZ0In0="),
        ));
        assert!(shown.iter().any(|cmd| cmd.id == CommandId::Decode));
    }

    #[test]
    fn prose_with_email_offers_redact() {
        let shown = chips(&data(
            SubjectKind::Text,
            Some("mail me at jan.devries@email.nl please"),
        ));
        assert!(shown.iter().any(|cmd| cmd.id == CommandId::Redact));
    }

    #[test]
    fn long_prose_offers_compress() {
        let text = "please summarize these notes for the team. ".repeat(40);
        let shown = chips(&data(SubjectKind::Text, Some(&text)));
        assert!(shown.iter().any(|cmd| cmd.id == CommandId::Compress));
    }

    #[test]
    fn image_card_is_a_picture_with_encoding_chips() {
        let mut input = data(SubjectKind::Image, None);
        input.image = Some(ImageFacts {
            format: "PNG".into(),
            width: 1280,
            height: 720,
            byte_len: 184_000,
        });
        let card = work_card(&input);
        assert_eq!(card.title, "Image");
        assert_eq!(card.meta, "PNG  1280×720  184 KB");
        assert!(card.shows_image);
        assert!(card.excerpt.is_empty());
        assert_eq!(
            titles(&chips(&input)),
            vec!["Preview", "Base64", "Data URL", "File"]
        );
    }

    #[test]
    fn text_card_shows_the_payload_not_a_label() {
        let card = work_card(&data(SubjectKind::Text, Some("{\n  \"a\": 1\n}")));
        assert_eq!(card.title, "JSON");
        assert_eq!(card.excerpt, "{\n  \"a\": 1\n}");
        assert!(card.meta.contains("lines"));
        assert!(!card.excerpt.contains("Preview"));
        assert!(!card.shows_image);
    }

    #[test]
    fn empty_clipboard_has_no_chips_and_keeps_quit_in_the_menu() {
        let input = data(SubjectKind::Empty, None);
        assert!(chips(&input).is_empty());
        let card = work_card(&input);
        assert_eq!(card.placeholder, "Nothing copied");
        assert!(card.excerpt.is_empty());
        let menu = overflow(&input);
        assert_eq!(menu.last().unwrap().id, CommandId::Quit);
        assert!(menu.iter().any(|cmd| cmd.id == CommandId::ToggleMenuBar));
        assert!(menu.iter().all(|cmd| cmd.id != CommandId::Preview));
        assert!(matching(&search_pool(&input), "quit").is_empty());
    }

    #[test]
    fn hidden_badge_is_the_menu_copy() {
        let mut input = data(SubjectKind::Empty, None);
        assert_eq!(
            overflow(&input)
                .iter()
                .find(|cmd| cmd.id == CommandId::ToggleMenuBar)
                .unwrap()
                .title,
            "Show menu bar icon"
        );
        input.menu_bar_shown = true;
        assert_eq!(
            overflow(&input)
                .iter()
                .find(|cmd| cmd.id == CommandId::ToggleMenuBar)
                .unwrap()
                .title,
            "Hide menu bar icon"
        );
    }

    #[test]
    fn query_finds_appearance_and_history_but_not_quit() {
        let mut input = data(SubjectKind::Text, Some("hello world"));
        input.history.push(Hist {
            index: 3,
            title: "notes from yesterday".into(),
            mark: "¶".into(),
        });
        let pool = search_pool(&input);
        assert!(matching(&pool, "quit").is_empty());
        assert_eq!(
            ids(&matching(&pool, "dark")),
            vec![CommandId::Appearance(Theme::Dark)]
        );
        assert!(
            chips(&input)
                .iter()
                .all(|cmd| !matches!(cmd.id, CommandId::Appearance(_) | CommandId::History(_)))
        );
        assert_eq!(
            ids(&matching(&pool, "yesterday")),
            vec![CommandId::History(3)]
        );
    }

    #[test]
    fn default_chips_leave_history_for_search() {
        let mut input = data(SubjectKind::Text, Some("hello world"));
        for index in 0..10 {
            input.history.push(Hist {
                index,
                title: format!("older {index}"),
                mark: "¶".into(),
            });
        }
        input.can_clear_history = true;
        let shown = chips(&input);
        assert!(shown.iter().all(|cmd| !housekeeping(&cmd.id)));
        assert!(
            shown
                .iter()
                .all(|cmd| !matches!(cmd.id, CommandId::History(_)))
        );
        assert_eq!(
            ids(&matching(&search_pool(&input), "older 9")),
            vec![CommandId::History(9)]
        );
        assert!(
            overflow(&input)
                .iter()
                .any(|cmd| cmd.id == CommandId::ClearHistory)
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
    fn excerpt_keeps_lines_and_cuts_a_long_one() {
        assert_eq!(payload_excerpt("alpha\nbeta"), "alpha\nbeta");
        let long = "x".repeat(80);
        let cut = payload_excerpt(&long);
        assert!(cut.ends_with('…'));
        let many = (0..12)
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let clipped = payload_excerpt(&many);
        assert_eq!(clipped.lines().count(), 6);
        assert!(clipped.ends_with('…'));
        assert!(cut.chars().count() <= 49);
    }

    #[test]
    fn chips_wrap_and_arrows_move_between_pills() {
        let titles = ["Preview", "Base64", "Data URL", "File", "Format", "Convert"];
        let frames = layout_chips(&titles, 220.0);
        assert!(frames.len() == titles.len());
        assert_eq!(frames[0].row, 0);
        assert!(frames.last().unwrap().row > 0);
        assert_eq!(step_chip(&frames, 0, 1, 0), 1);
        assert_eq!(step_chip(&frames, 0, -1, 0), 0);
        let down = step_chip(&frames, 0, 0, 1);
        assert_ne!(frames[down].row, frames[0].row);
        assert_eq!(step_chip(&frames, down, 0, -1), 0);
        assert_eq!(step_chip(&[], 0, 1, 0), 0);
    }
}
