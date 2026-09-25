use crate::appearance::Theme;
use crate::compress;
use crate::decode;
use crate::format;
use crate::toolbar_visibility;

pub const MAX_VISIBLE: usize = 8;
pub const CHIP_PITCH: f64 = 34.0;
pub const CHIP_PILL_H: f64 = 28.0;

const CHIP_GAP: f64 = 6.0;
/// History arrow buttons. Same height as a chip, wide enough for `<` and `>`.
pub const NAV_BUTTON: f64 = 32.0;
pub const NAV_GAP: f64 = CHIP_GAP;
pub const NAV_SPAN: f64 = NAV_BUTTON + NAV_GAP + NAV_BUTTON;
/// Empty space kept on the right of the first chip row so the arrows fit.
pub const NAV_RESERVE: f64 = CHIP_GAP + NAV_SPAN;
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
    /// `<` older and `>` newer, when an earlier copy exists. Absent hides both.
    pub history_nav: Option<HistoryNav>,
    /// Current link and the copies beside it, fetched ahead of the next arrow.
    pub warm_links: Vec<String>,
    pub theme: Theme,
}

/// Which way history can move. Index 0 is the newest copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoryNav {
    pub can_older: bool,
    pub can_newer: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandId {
    Preview,
    Visit,
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
    /// Step to the previous copy. The history order stays put.
    HistoryOlder,
    /// Step back toward the newest copy.
    HistoryNewer,
    ClearClipboard,
    ClearHistory,
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
    /// YouTube page, when the card should load a video thumbnail.
    pub link_page: Option<String>,
    pub link_thumb: Option<String>,
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
            link_page: None,
            link_thumb: None,
        },
        SubjectKind::Text => {
            let text = data.subject_text.as_deref().unwrap_or("");
            if let Some(card) = youtube_card(text) {
                return card;
            }
            if let Some(card) = page_card(text) {
                return card;
            }
            WorkCard {
                title: format::detect(text).source_heading().to_string(),
                meta: text_meta(text),
                excerpt: payload_excerpt(text),
                placeholder: String::new(),
                shows_image: false,
                link_page: None,
                link_thumb: None,
            }
        }
        SubjectKind::Empty => WorkCard {
            title: "Clipboard".to_string(),
            meta: String::new(),
            excerpt: String::new(),
            placeholder: "Nothing copied".to_string(),
            shows_image: false,
            link_page: None,
            link_thumb: None,
        },
        SubjectKind::NoText => WorkCard {
            title: "Clipboard".to_string(),
            meta: String::new(),
            excerpt: String::new(),
            placeholder: "No text on the clipboard".to_string(),
            shows_image: false,
            link_page: None,
            link_thumb: None,
        },
    }
}

fn page_card(text: &str) -> Option<WorkCard> {
    let page = crate::page_preview::page_url(text)?;
    let host = crate::page_preview::host(page).unwrap_or("Page");
    Some(WorkCard {
        title: host.to_string(),
        meta: text_meta(text),
        excerpt: payload_excerpt(text),
        placeholder: String::new(),
        shows_image: false,
        link_page: Some(page.to_string()),
        link_thumb: None,
    })
}

fn youtube_card(text: &str) -> Option<WorkCard> {
    let id = crate::youtube::video_id(text)?;
    Some(WorkCard {
        title: "YouTube".to_string(),
        meta: text_meta(text),
        excerpt: payload_excerpt(text),
        placeholder: String::new(),
        shows_image: false,
        link_page: Some(text.trim().to_string()),
        link_thumb: Some(crate::youtube::thumbnail_url(id)),
    })
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
        SubjectKind::Text => {
            let text = data.subject_text.as_deref().unwrap_or("");
            if crate::youtube::video_id(text).is_some()
                || crate::page_preview::page_url(text).is_some()
            {
                link_chips(text)
            } else {
                text_chips(text)
            }
        }
        SubjectKind::Empty | SubjectKind::NoText => Vec::new(),
    }
}

/// Chips, earlier copies, and appearance. Quit stays out.
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

/// Quit, earlier copies, and housekeeping. Copies stay out of [`chips`].
pub fn overflow(data: &LaunchData) -> Vec<Command> {
    let mut commands = vec![command(
        CommandId::ClearClipboard,
        "Clear clipboard",
        "Empty the pasteboard",
        "clear clipboard empty",
    )];
    for item in &data.history {
        commands.push(command(
            CommandId::History(item.index),
            &item.title,
            &item.mark,
            "history",
        ));
    }
    if data.can_clear_history {
        commands.push(command(
            CommandId::ClearHistory,
            "Clear history",
            "Forget copies",
            "clear history forget",
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

/// Width of a chip. The label is inset 8pt on each side, and the text field
/// adds its own padding around the 13pt system font. A field that is even
/// slightly short replaces the tail with an ellipsis, so "Copy" draws as "Co…".
pub fn chip_width(title: &str) -> f64 {
    let chars = title.chars().count() as f64;
    (32.0 + chars * 8.0).clamp(64.0, 220.0)
}

/// `trailing` is kept clear on the right of the first row only.
pub fn layout_chips(titles: &[&str], width: f64, trailing: f64) -> Vec<ChipFrame> {
    let mut frames = Vec::with_capacity(titles.len());
    let mut x = 0.0;
    let mut row = 0usize;
    for title in titles {
        let chip = chip_width(title);
        let limit = if row == 0 {
            (width - trailing).max(0.0)
        } else {
            width
        };
        if x > 0.0 && x + chip > limit {
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

/// Arrows when there is an earlier copy. `cursor` 0 is the newest entry.
pub fn history_nav(len: usize, cursor: usize) -> Option<HistoryNav> {
    if len < 2 {
        return None;
    }
    let cursor = cursor.min(len - 1);
    Some(HistoryNav {
        can_older: cursor + 1 < len,
        can_newer: cursor > 0,
    })
}

pub fn step_history(len: usize, cursor: usize, older: bool) -> Option<usize> {
    let nav = history_nav(len, cursor)?;
    let cursor = cursor.min(len - 1);
    if older && nav.can_older {
        Some(cursor + 1)
    } else if !older && nav.can_newer {
        Some(cursor - 1)
    } else {
        None
    }
}

/// Link pages for `cursor` and the entries beside it. Current first, then older, then newer.
pub fn pages_around(entries: &[Option<&str>], cursor: usize) -> Vec<String> {
    if entries.is_empty() {
        return Vec::new();
    }
    let cursor = cursor.min(entries.len() - 1);
    let mut pages = Vec::new();
    let mut push = |index: usize| {
        let Some(text) = entries.get(index).copied().flatten() else {
            return;
        };
        let Some(page) = link_page_key(text) else {
            return;
        };
        if !pages.iter().any(|existing| existing == &page) {
            pages.push(page);
        }
    };
    push(cursor);
    if let Some(older) = cursor.checked_add(1) {
        push(older);
    }
    if let Some(newer) = cursor.checked_sub(1) {
        push(newer);
    }
    pages
}

fn link_page_key(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if crate::youtube::video_id(text).is_some() || crate::page_preview::page_url(text).is_some() {
        Some(text.to_string())
    } else {
        None
    }
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
        CommandId::History(_)
            | CommandId::HistoryOlder
            | CommandId::HistoryNewer
            | CommandId::Appearance(_)
            | CommandId::ClearClipboard
            | CommandId::ClearHistory
            | CommandId::ImageBase64
            | CommandId::ImageDataUrl
            | CommandId::ImageFile
    )
}

fn link_chips(text: &str) -> Vec<Command> {
    let mut commands = vec![command(
        CommandId::Visit,
        "Visit",
        "Open in browser",
        "visit open browser",
    )];
    if toolbar_visibility::shows_format(text) {
        commands.push(command(
            CommandId::Format,
            "Format",
            "URI",
            "format url uri",
        ));
    }
    commands
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

/// Pasted size. Megabytes from 0.01 MB up; kilobytes below that.
fn format_bytes(n: usize) -> String {
    if n < 10_000 {
        format_kilobytes(n)
    } else {
        format_megabytes(n)
    }
}

fn format_megabytes(n: usize) -> String {
    let mb = n as f64 / 1_000_000.0;
    let places = if mb >= 100.0 {
        0
    } else if mb >= 10.0 {
        1
    } else {
        2
    };
    format!("{mb:.places$} MB")
}

fn format_kilobytes(n: usize) -> String {
    if n == 0 {
        return "0 KB".to_string();
    }
    let kb = n as f64 / 1_000.0;
    if kb >= 1.0 {
        return format!("{kb:.1} KB");
    }
    let mut body = format!("{kb:.3}");
    while body.contains('.') && body.ends_with('0') {
        body.pop();
    }
    format!("{body} KB")
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
        CommandId, Hist, ImageFacts, LaunchData, NAV_RESERVE, NAV_SPAN, SubjectKind, chip_width,
        chips, history_nav, layout_chips, matching, overflow, pages_around, payload_excerpt,
        search_pool, snippet, step_chip, step_history, work_card,
    };
    use crate::appearance::Theme;

    fn data(kind: SubjectKind, text: Option<&str>) -> LaunchData {
        LaunchData {
            subject_kind: kind,
            subject_text: text.map(str::to_string),
            image: None,
            history: Vec::new(),
            can_clear_history: false,
            history_nav: None,
            warm_links: Vec::new(),
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
        matches!(id, CommandId::Quit | CommandId::ClearHistory)
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
        assert_eq!(card.meta, "PNG  1280×720  0.18 MB");
        assert!(card.shows_image);
        assert!(card.excerpt.is_empty());
        assert_eq!(
            titles(&chips(&input)),
            vec!["Preview", "Base64", "Data URL", "File"]
        );
    }

    #[test]
    fn youtube_url_previews_as_a_thumbnail() {
        let url = "https://www.youtube.com/watch?v=bEN9Dyg48b0";
        let card = work_card(&data(SubjectKind::Text, Some(url)));
        assert_eq!(card.title, "YouTube");
        assert_eq!(card.link_page.as_deref(), Some(url));
        assert_eq!(
            card.link_thumb.as_deref(),
            Some("https://i.ytimg.com/vi/bEN9Dyg48b0/hqdefault.jpg")
        );
        assert_eq!(card.excerpt, url);
        assert!(!card.shows_image);
        let input = data(SubjectKind::Text, Some(url));
        assert_eq!(titles(&chips(&input)), vec!["Visit"]);
    }

    #[test]
    fn html_url_previews_like_a_page() {
        let url = "https://www.example.com/news/story";
        let input = data(SubjectKind::Text, Some(url));
        let card = work_card(&input);
        assert_eq!(card.title, "example.com");
        assert_eq!(card.link_page.as_deref(), Some(url));
        assert!(card.link_thumb.is_none());
        assert_eq!(card.excerpt, url);
        assert_eq!(titles(&chips(&input)), vec!["Visit"]);
    }

    #[test]
    fn bare_host_previews_as_a_page() {
        let input = data(SubjectKind::Text, Some("grok.com"));
        let card = work_card(&input);
        assert_eq!(card.title, "grok.com");
        assert_eq!(card.link_page.as_deref(), Some("grok.com"));
        assert_eq!(titles(&chips(&input)), vec!["Visit", "Format"]);
    }

    #[test]
    fn page_card_adds_format_when_the_url_needs_it() {
        let messy = "https://example.com/search?q=a/b";
        assert_eq!(
            titles(&chips(&data(SubjectKind::Text, Some(messy)))),
            vec!["Visit", "Format"]
        );
    }

    #[test]
    fn file_urls_stay_text() {
        let card = work_card(&data(
            SubjectKind::Text,
            Some("https://example.com/report.pdf"),
        ));
        assert_eq!(card.title, "URL");
        assert!(card.link_page.is_none());
        assert_eq!(
            titles(&chips(&data(
                SubjectKind::Text,
                Some("https://example.com/report.pdf")
            ))),
            vec!["Preview"]
        );
    }

    #[test]
    fn pasted_size_uses_kilobytes_when_small() {
        assert_eq!(super::format_bytes(0), "0 KB");
        assert_eq!(super::format_bytes(12), "0.012 KB");
        assert_eq!(super::format_bytes(1_500), "1.5 KB");
        assert_eq!(super::format_bytes(9_000), "9.0 KB");
        assert_eq!(super::format_bytes(184_000), "0.18 MB");
        assert_eq!(super::format_bytes(2_000_000), "2.00 MB");
        assert_eq!(super::format_bytes(15_500_000), "15.5 MB");
        assert_eq!(super::format_bytes(250_000_000), "250 MB");
        let card = work_card(&data(SubjectKind::Text, Some("hello")));
        assert_eq!(card.meta, "0.005 KB");
    }

    #[test]
    fn text_card_shows_the_payload_not_a_label() {
        let card = work_card(&data(SubjectKind::Text, Some("{\n  \"a\": 1\n}")));
        assert_eq!(card.title, "JSON");
        assert_eq!(card.excerpt, "{\n  \"a\": 1\n}");
        assert!(card.meta.contains("lines"));
        assert!(card.meta.contains("KB"));
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
        assert!(menu.iter().all(|cmd| cmd.id != CommandId::Preview));
        assert!(matching(&search_pool(&input), "quit").is_empty());
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
        assert!(
            overflow(&input)
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
    fn chip_width_leaves_room_for_the_label() {
        assert!(chip_width("Copy") - 16.0 >= 44.0);
        assert!(chip_width("Visit") - 16.0 >= 44.0);
        assert!(chip_width("Base64") - 16.0 >= 58.0);
        assert!(chip_width("Preview") - 16.0 >= 60.0);
    }

    #[test]
    fn chips_wrap_and_arrows_move_between_pills() {
        let titles = ["Preview", "Base64", "Data URL", "File", "Format", "Convert"];
        let frames = layout_chips(&titles, 220.0, 0.0);
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

    #[test]
    fn history_nav_hides_until_an_earlier_copy_exists() {
        assert!(history_nav(0, 0).is_none());
        assert!(history_nav(1, 0).is_none());
        let newest = history_nav(3, 0).unwrap();
        assert!(newest.can_older);
        assert!(!newest.can_newer);
        let middle = history_nav(3, 1).unwrap();
        assert!(middle.can_older && middle.can_newer);
        let oldest = history_nav(3, 2).unwrap();
        assert!(!oldest.can_older);
        assert!(oldest.can_newer);
        assert_eq!(step_history(3, 0, true), Some(1));
        assert_eq!(step_history(3, 0, false), None);
        assert_eq!(step_history(3, 2, true), None);
        assert_eq!(step_history(3, 2, false), Some(1));
        assert_eq!(step_history(1, 0, true), None);
    }

    #[test]
    fn history_navigation_warms_the_page_and_its_neighbors() {
        let current = "https://youtu.be/abcdefghijk";
        let older = "https://www.youtube.com/watch?v=bcdefghijkl";
        let newer = "https://example.com/news";
        let entries = [Some(current), Some(older), Some("plain notes"), Some(newer)];
        assert_eq!(
            pages_around(&entries, 0),
            vec![current.to_string(), older.to_string()]
        );
        assert_eq!(
            pages_around(&entries, 1),
            vec![older.to_string(), current.to_string()]
        );
        assert_eq!(
            pages_around(&entries, 2),
            vec![newer.to_string(), older.to_string()]
        );
        assert!(pages_around(&[], 0).is_empty());
    }

    #[test]
    fn history_arrows_keep_the_first_row_clear() {
        let titles = ["Visit", "Format", "Preview", "Base64", "Data URL", "File"];
        let width = 412.0;
        let frames = layout_chips(&titles, width, NAV_RESERVE);
        let nav_left = width - NAV_SPAN;
        assert!(frames.iter().any(|frame| frame.row > 0));
        for frame in frames.iter().filter(|frame| frame.row == 0) {
            assert!(frame.x + frame.width <= nav_left - super::CHIP_GAP + 0.01);
        }
    }
}
