use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};

use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use tray_icon::{
    MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{
        IconMenuItem, Menu, MenuEvent, MenuItem, NativeIcon, PredefinedMenuItem, Submenu, TextStyle,
    },
};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
};

use crate::appearance::{self, Theme};
use crate::clipboard::{self, ClipboardHistory, ClipboardView};
use crate::commands::{self, CommandId, Hist, LaunchData, SubjectKind};
use crate::format;
use crate::hotkey;
use crate::icon;
use crate::launcher::{self, UserEvent};
use crate::preview::{self, PreviewAction};

const REFRESH: Duration = Duration::from_millis(400);

struct App {
    tray: TrayIcon,
    shown_accent: Option<[u8; 4]>,
    history: ClipboardHistory,
    /// Index into history while the arrows are browsing. 0 is the newest.
    history_cursor: usize,
    current_image: Option<Arc<[u8]>>,
    recorded_image_change: Option<isize>,
    skip_image_change: Option<isize>,
    skip_record: Option<String>,
    signature: ClipSig,
    _hotkeys: GlobalHotKeyManager,
    format_hotkey_id: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct ClipSig {
    text_hash: u64,
    image: bool,
    image_change: isize,
    history_len: usize,
}

impl Default for ClipSig {
    fn default() -> Self {
        Self {
            text_hash: 0,
            image: false,
            image_change: -1,
            history_len: usize::MAX,
        }
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, _: &ActiveEventLoop) {}

    fn window_event(&mut self, _: &ActiveEventLoop, _: winit::window::WindowId, _: WindowEvent) {}

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Run(id) => self.run_command(event_loop, id),
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            match event.id.0.as_str() {
                "quit" => {
                    event_loop.exit();
                    return;
                }
                id => {
                    if let Some(index) = id
                        .strip_prefix("hist-")
                        .and_then(|value| value.parse::<usize>().ok())
                    {
                        self.restore_history(index);
                    }
                }
            }
        }

        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.id == self.format_hotkey_id && event.state == HotKeyState::Pressed {
                self.summon_popup();
            }
        }

        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                self.reveal_popup();
            }
        }

        let changed = self.note_clipboard();
        if changed && launcher::is_open() {
            let view = ClipboardView::from_os();
            launcher::sync(self.launch_data(&view));
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + REFRESH));
    }
}

impl App {
    fn run_command(&mut self, event_loop: &ActiveEventLoop, id: CommandId) {
        match id {
            CommandId::Preview => self.show_current(PreviewAction::Original),
            CommandId::Visit => self.visit_current(),
            CommandId::Format => {
                if !self.format_link_in_place() {
                    self.show_current(PreviewAction::Format);
                }
            }
            CommandId::Convert => self.show_current(PreviewAction::Convert),
            CommandId::Decode => self.show_current(PreviewAction::Decode),
            CommandId::Compress => self.show_current(PreviewAction::Compress),
            CommandId::Redact => self.show_current(PreviewAction::Redact),
            CommandId::Dataframe => self.show_current(PreviewAction::Dataframe),
            CommandId::History(index) => self.restore_history(index),
            CommandId::HistoryOlder => self.step_history(true),
            CommandId::HistoryNewer => self.step_history(false),
            CommandId::ImageBase64 => self.copy_image_text(false),
            CommandId::ImageDataUrl => self.copy_image_text(true),
            CommandId::ImageFile => self.copy_image_file(),
            CommandId::ClearClipboard => self.clear_clipboard(),
            CommandId::ClearHistory => self.clear_history(),
            CommandId::Appearance(theme) => {
                theme.save();
                self.refresh_popup();
            }
            CommandId::Quit => event_loop.exit(),
        }
    }

    fn summon_popup(&mut self) {
        let view = ClipboardView::from_os();
        self.record_current(&view);
        launcher::summon(self.launch_data(&view));
    }

    fn reveal_popup(&mut self) {
        let view = ClipboardView::from_os();
        self.record_current(&view);
        launcher::reveal(self.launch_data(&view));
    }

    fn refresh_popup(&mut self) {
        if !launcher::is_open() {
            return;
        }
        let view = ClipboardView::from_os();
        self.record_current(&view);
        launcher::sync(self.launch_data(&view));
    }

    fn launch_data(&self, view: &ClipboardView) -> LaunchData {
        let (subject_kind, subject_text) = match view {
            ClipboardView::Empty => (SubjectKind::Empty, None),
            ClipboardView::NoText => (SubjectKind::NoText, None),
            ClipboardView::Image => (SubjectKind::Image, None),
            ClipboardView::Text(text) => (SubjectKind::Text, Some(text.clone())),
        };
        let history = self
            .history
            .labels()
            .into_iter()
            .map(|(index, _)| Hist {
                index,
                title: history_title(&self.history, index),
                mark: self.history.mark(index).unwrap_or("").to_string(),
            })
            .collect();
        LaunchData {
            subject_kind,
            subject_text,
            image: image_facts(view),
            history,
            can_clear_history: !self.history.is_empty(),
            history_nav: commands::history_nav(self.history.len(), self.history_cursor),
            warm_links: warm_history_links(&self.history, self.history_cursor),
            theme: Theme::load(),
        }
    }

    fn visit_current(&self) {
        let Some(text) = ClipboardView::from_os().text().map(str::to_string) else {
            return;
        };
        let raw = crate::page_preview::page_url(&text)
            .map(str::to_string)
            .or_else(|| crate::youtube::video_id(&text).map(|_| text.trim().to_string()));
        let Some(raw) = raw else {
            return;
        };
        let url = crate::format::format_text(&raw);
        #[cfg(target_os = "macos")]
        std::thread::spawn(move || {
            if let Err(e) = std::process::Command::new("open").arg(url).status() {
                eprintln!("visit failed: {e}");
            }
        });
        #[cfg(not(target_os = "macos"))]
        let _ = url;
    }

    fn format_link_in_place(&mut self) -> bool {
        let Some(text) = ClipboardView::from_os().text().map(str::to_string) else {
            return false;
        };
        let link = crate::page_preview::page_url(&text).is_some()
            || crate::youtube::video_id(&text).is_some();
        if !link {
            return false;
        }
        let formatted = crate::format::format_text(text.trim());
        if formatted != text.trim() {
            if let Err(e) = clipboard::write_clipboard(&formatted) {
                eprintln!("format link failed: {e}");
            } else {
                self.refresh_popup();
            }
        }
        true
    }

    fn copy_image_text(&mut self, data_url: bool) {
        #[cfg(target_os = "macos")]
        {
            let Some(text) = crate::macos_pasteboard::image_as_text(data_url) else {
                eprintln!("image encoding failed");
                return;
            };
            self.skip_record = Some(text.clone());
            if let Err(e) = clipboard::write_clipboard(&text) {
                eprintln!("copy image text failed: {e}");
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = data_url;
        }
    }

    fn copy_image_file(&mut self) {
        #[cfg(target_os = "macos")]
        {
            if let Err(e) = crate::macos_pasteboard::copy_image_file() {
                eprintln!("copy image file failed: {e}");
                return;
            }
            self.skip_image_change = Some(crate::macos_pasteboard::change_count());
        }
    }

    fn clear_history(&mut self) {
        let view = ClipboardView::from_os();
        self.skip_record = view.text().map(str::to_string);
        if view.is_image() {
            #[cfg(target_os = "macos")]
            {
                self.skip_image_change = Some(crate::macos_pasteboard::change_count());
            }
        }
        self.current_image = None;
        self.history.clear();
        self.history_cursor = 0;
    }

    fn clear_clipboard(&mut self) {
        if let Err(e) = clipboard::clear_clipboard() {
            eprintln!("clear clipboard failed: {e}");
            return;
        }
        self.skip_record = Some(String::new());
    }

    fn should_record(&self, text: &str) -> bool {
        self.skip_record.as_deref() != Some(text)
    }

    fn show_current(&mut self, action: PreviewAction) {
        let view = ClipboardView::from_os();
        if view.is_image() {
            self.open_image_preview();
            return;
        }
        if let Some(text) = view.text() {
            self.open_preview(text, action);
        }
    }

    fn step_history(&mut self, older: bool) {
        let Some(next) = commands::step_history(self.history.len(), self.history_cursor, older)
        else {
            return;
        };
        let previous = self.history_cursor;
        self.history_cursor = next;
        if !self.present_history(next) {
            self.history_cursor = previous;
        }
    }

    // Put a history entry on the clipboard without moving it to the front.
    fn present_history(&mut self, index: usize) -> bool {
        if self.history.image(index).is_some() {
            return self.present_history_image(index);
        }
        let Some(text) = self.history.get(index).map(str::to_string) else {
            return false;
        };
        // Recording would move this entry to the front, so the other arrow
        // could no longer walk back through the list.
        self.skip_record = Some(text.clone());
        if let Err(e) = clipboard::write_clipboard(&text) {
            eprintln!("restore history failed: {e}");
            self.skip_record = None;
            return false;
        }
        if launcher::is_open() {
            self.refresh_popup();
        }
        true
    }

    fn present_history_image(&mut self, index: usize) -> bool {
        let Some(bytes) = self.history.image(index) else {
            return false;
        };
        #[cfg(target_os = "macos")]
        {
            if let Err(e) = crate::macos_pasteboard::write_history_image(&bytes) {
                eprintln!("restore image failed: {e}");
                return false;
            }
            self.skip_record = None;
            self.skip_image_change = Some(crate::macos_pasteboard::change_count());
            if launcher::is_open() {
                self.refresh_popup();
            }
            true
        }
        #[cfg(not(target_os = "macos"))]
        {
            self.open_stored_image(bytes);
            false
        }
    }

    fn restore_history(&mut self, index: usize) {
        if let Some(bytes) = self.history.image(index) {
            #[cfg(target_os = "macos")]
            if bytes.starts_with(b"\x89PNG") {
                if let Err(e) = crate::macos_pasteboard::write_png(&bytes) {
                    eprintln!("restore image failed: {e}");
                }
            } else {
                self.open_stored_image(bytes);
            }
            #[cfg(not(target_os = "macos"))]
            self.open_stored_image(bytes);
            return;
        }
        let Some(text) = self.history.get(index).map(str::to_string) else {
            return;
        };
        if let Err(e) = clipboard::write_clipboard(&text) {
            eprintln!("restore history failed: {e}");
            return;
        }
        if launcher::is_open() {
            self.refresh_popup();
        }
    }

    fn open_preview(&mut self, text: &str, action: PreviewAction) {
        let kind = format::detect(text);
        if let Err(e) = preview::show_action(text, kind, action) {
            eprintln!("preview failed: {e}");
        }
    }

    fn open_image_preview(&mut self) {
        if let Err(e) = preview::show_clipboard_image() {
            eprintln!("preview failed: {e}");
        }
    }

    fn open_stored_image(&mut self, bytes: Arc<[u8]>) {
        if let Err(e) = preview::show_stored_image(bytes.to_vec()) {
            eprintln!("preview failed: {e}");
        }
    }

    fn record_current(&mut self, view: &ClipboardView) {
        #[cfg(target_os = "macos")]
        if view.is_image() {
            let change = crate::macos_pasteboard::change_count();
            if self.skip_image_change == Some(change) || self.recorded_image_change == Some(change)
            {
                return;
            }
            self.recorded_image_change = Some(change);
            if let Some(bytes) = crate::macos_pasteboard::current_image_bytes() {
                self.current_image = self.history.record_image(bytes);
                self.history_cursor = 0;
            }
            return;
        }
        self.current_image = None;
        if let Some(text) = view.text()
            && self.should_record(text)
        {
            self.skip_record = None;
            self.history.record(text.to_string());
            self.history_cursor = 0;
        }
    }

    fn note_clipboard(&mut self) -> bool {
        let view = ClipboardView::from_os();
        self.record_current(&view);
        #[cfg(target_os = "macos")]
        let image_change = crate::macos_pasteboard::change_count();
        #[cfg(not(target_os = "macos"))]
        let image_change = 0;
        let signature = ClipSig {
            text_hash: hash_text(view.text().unwrap_or("")),
            image: view.is_image(),
            image_change,
            history_len: self.history.labels().len(),
        };
        if signature == self.signature {
            return false;
        }
        self.signature = signature;
        self.sync_icon(icon::accent_for_kind(detected_kind(&view)));
        self.sync_tooltip(&view);
        self.refresh_status_menu();
        true
    }

    fn refresh_status_menu(&self) {
        let entries: Vec<(usize, String)> = self
            .history
            .labels()
            .into_iter()
            .map(|(index, _)| (index, history_title(&self.history, index)))
            .collect();
        self.tray.set_menu(Some(Box::new(status_menu(&entries))));
    }

    fn sync_icon(&mut self, accent: Option<[u8; 4]>) {
        if accent == self.shown_accent {
            return;
        }
        self.shown_accent = accent;
        match icon::menu_icon_tinted(accent) {
            Ok(icon) => {
                if let Err(e) = self.tray.set_icon(Some(icon)) {
                    eprintln!("menu bar icon failed: {e}");
                }
            }
            Err(e) => eprintln!("menu bar icon failed: {e}"),
        }
    }

    fn sync_tooltip(&self, view: &ClipboardView) {
        if let Err(e) = self.tray.set_tooltip(Some(icon_tip(view))) {
            eprintln!("menu bar tooltip failed: {e}");
        }
    }
}

fn icon_tip(view: &ClipboardView) -> String {
    match view {
        ClipboardView::Image => "Image".to_string(),
        ClipboardView::Empty | ClipboardView::NoText => "Copycraft".to_string(),
        ClipboardView::Text(text) => {
            if crate::youtube::video_id(text).is_some() {
                "YouTube".to_string()
            } else if let Some(host) =
                crate::page_preview::page_url(text).and_then(crate::page_preview::host)
            {
                host.to_string()
            } else {
                format::detect(text).source_heading().to_string()
            }
        }
    }
}

fn detected_kind(view: &ClipboardView) -> Option<format::FormatKind> {
    match view {
        ClipboardView::Image => Some(format::FormatKind::Image),
        ClipboardView::Text(text) => Some(format::detect(text)),
        ClipboardView::Empty | ClipboardView::NoText => None,
    }
}

fn image_facts(view: &ClipboardView) -> Option<commands::ImageFacts> {
    if !view.is_image() {
        return None;
    }
    #[cfg(target_os = "macos")]
    {
        crate::macos_pasteboard::image_facts()
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

fn warm_history_links(history: &ClipboardHistory, cursor: usize) -> Vec<String> {
    let entries: Vec<Option<&str>> = (0..history.len()).map(|index| history.get(index)).collect();
    commands::pages_around(&entries, cursor)
}

fn history_title(history: &ClipboardHistory, index: usize) -> String {
    if let Some(text) = history.get(index) {
        let body = commands::snippet(text);
        if body.is_empty() {
            history.mark(index).unwrap_or("").to_string()
        } else {
            body
        }
    } else {
        "Image".to_string()
    }
}

fn hash_text(text: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

fn version_label() -> String {
    format!("Copycraft {}", env!("CARGO_PKG_VERSION"))
}

fn status_labels() -> [String; 3] {
    [
        hotkey::LABEL.to_string(),
        version_label(),
        "Quit".to_string(),
    ]
}

#[cfg(test)]
fn status_rows(entries: &[(usize, String)]) -> Vec<String> {
    let [hotkey, version, quit] = status_labels();
    let mut rows = vec![hotkey];
    if !entries.is_empty() {
        rows.push("History".to_string());
    }
    rows.push(version);
    rows.push(quit);
    rows
}

fn info_item(label: &str) -> MenuItem {
    let item = MenuItem::new(label, false, None);
    item.set_styled_text(vec![(label.to_string(), TextStyle::Secondary)]);
    item
}

fn status_menu(entries: &[(usize, String)]) -> Menu {
    let [hotkey, version, quit] = status_labels();
    let menu = Menu::new();
    let _ = menu.append(&info_item(&hotkey));
    if !entries.is_empty() {
        let history = Submenu::new("History", true);
        for (index, title) in entries {
            let _ = history.append(&MenuItem::with_id(
                format!("hist-{index}"),
                title,
                true,
                None,
            ));
        }
        let _ = menu.append(&history);
    }
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&info_item(&version));
    let _ = menu.append(&IconMenuItem::with_id_and_native_icon(
        "quit",
        &quit,
        true,
        Some(NativeIcon::StopProgress),
        None,
    ));
    menu
}

fn register_format_hotkey() -> Result<(GlobalHotKeyManager, u32), Box<dyn std::error::Error>> {
    let manager = GlobalHotKeyManager::new()?;
    let hotkey = hotkey::open();
    let id = hotkey.id();
    manager.register(hotkey)?;
    Ok((manager, id))
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    appearance::apply(Theme::load());
    let (hotkeys, format_hotkey_id) = register_format_hotkey()?;
    let icon = icon::menu_icon()?;
    let tray = TrayIconBuilder::new()
        .with_icon(icon)
        .with_icon_as_template(false)
        .with_menu(Box::new(status_menu(&[])))
        .with_menu_on_left_click(false)
        .with_tooltip("Copycraft")
        .build()?;

    let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;
    launcher::install_proxy(event_loop.create_proxy());

    let mut app = App {
        tray,
        shown_accent: None,
        history: ClipboardHistory::default(),
        history_cursor: 0,
        current_image: None,
        recorded_image_change: None,
        skip_image_change: None,
        skip_record: None,
        signature: ClipSig::default(),
        _hotkeys: hotkeys,
        format_hotkey_id,
    };
    event_loop.run_app(&mut app)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{detected_kind, status_labels, status_rows, version_label};
    use crate::clipboard::ClipboardView;
    use crate::format::FormatKind;
    use crate::hotkey;
    use crate::icon;

    #[test]
    fn version_label_includes_package_version() {
        assert_eq!(
            version_label(),
            format!("Copycraft {}", env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn icon_accent_follows_detected_content() {
        assert_eq!(
            icon::accent_for_kind(detected_kind(&ClipboardView::Empty)),
            None
        );
        assert_eq!(
            icon::accent_for_kind(detected_kind(&ClipboardView::Text("hello".into()))),
            None
        );
        assert_eq!(
            icon::accent_for_kind(detected_kind(&ClipboardView::Text(r#"{"a":1}"#.into()))),
            FormatKind::Json.accent_rgba()
        );
        assert_eq!(
            icon::accent_for_kind(detected_kind(&ClipboardView::Text("fn main() {}".into()))),
            FormatKind::Rust.accent_rgba()
        );
        assert_eq!(
            icon::accent_for_kind(detected_kind(&ClipboardView::Image)),
            FormatKind::Image.accent_rgba()
        );
    }

    #[test]
    fn status_menu_lists_hotkey_then_version_then_quit() {
        assert_eq!(
            status_labels(),
            [
                hotkey::LABEL.to_string(),
                version_label(),
                "Quit".to_string(),
            ]
        );
    }

    #[test]
    fn status_menu_lists_history_between_the_hotkey_and_version() {
        let entries = vec![
            (1, "older note".to_string()),
            (0, "just copied".to_string()),
        ];
        assert_eq!(
            status_rows(&entries),
            vec![
                hotkey::LABEL.to_string(),
                "History".to_string(),
                version_label(),
                "Quit".to_string(),
            ]
        );
    }
}
