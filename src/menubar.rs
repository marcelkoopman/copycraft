use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use tray_icon::{
    MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{IconMenuItem, Menu, MenuEvent, MenuItem, NativeIcon, PredefinedMenuItem, TextStyle},
};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
};

use crate::appearance::{self, Theme};
use crate::badge;
use crate::clipboard::{self, ClipboardHistory, ClipboardView};
use crate::commands::{self, CommandId, Hist, LaunchData, SubjectKind};
use crate::format;
use crate::icon;
use crate::launcher::{self, UserEvent};
use crate::preview::{self, PreviewAction};

const REFRESH: Duration = Duration::from_millis(400);

struct App {
    tray: TrayIcon,
    history: ClipboardHistory,
    current_image: Option<Arc<[u8]>>,
    recorded_image_change: Option<isize>,
    skip_image_change: Option<isize>,
    skip_record: Option<String>,
    signature: ClipSig,
    welcomed: bool,
    _hotkeys: GlobalHotKeyManager,
    format_hotkey_id: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct ClipSig {
    text_hash: u64,
    image: bool,
    image_change: isize,
    history_len: usize,
    badge: bool,
}

impl Default for ClipSig {
    fn default() -> Self {
        Self {
            text_hash: 0,
            image: false,
            image_change: -1,
            history_len: usize::MAX,
            badge: !badge::DEFAULT_SHOWN,
        }
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, _: &ActiveEventLoop) {
        if self.welcomed {
            return;
        }
        self.welcomed = true;
        self.reveal_popup();
    }

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
                "open" => self.reveal_popup(),
                "hide_badge" => self.set_badge(false),
                _ => {}
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
            CommandId::Format => self.show_current(PreviewAction::Format),
            CommandId::Convert => self.show_current(PreviewAction::Convert),
            CommandId::Decode => self.show_current(PreviewAction::Decode),
            CommandId::Compress => self.show_current(PreviewAction::Compress),
            CommandId::Redact => self.show_current(PreviewAction::Redact),
            CommandId::Dataframe => self.show_current(PreviewAction::Dataframe),
            CommandId::History(index) => self.show_history(index),
            CommandId::ImageBase64 => self.copy_image_text(false),
            CommandId::ImageDataUrl => self.copy_image_text(true),
            CommandId::ImageFile => self.copy_image_file(),
            CommandId::ClearClipboard => self.clear_clipboard(),
            CommandId::ClearHistory => self.clear_history(),
            CommandId::ToggleMenuBar => self.set_badge(!badge::is_shown()),
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

    fn set_badge(&mut self, shown: bool) {
        badge::set_shown(shown);
        if let Err(e) = self.tray.set_visible(shown) {
            eprintln!("menu bar badge failed: {e}");
        }
        self.refresh_popup();
    }

    fn launch_data(&self, view: &ClipboardView) -> LaunchData {
        let (subject_kind, subject_text) = match view {
            ClipboardView::Empty => (SubjectKind::Empty, None),
            ClipboardView::NoText => (SubjectKind::NoText, None),
            ClipboardView::Image => (SubjectKind::Image, None),
            ClipboardView::Text(text) => (SubjectKind::Text, Some(text.clone())),
        };
        let current_text = view.text();
        let history = self
            .history
            .labels()
            .into_iter()
            .filter(|(index, _)| {
                self.history
                    .shows_in_history(*index, current_text, self.current_image.as_ref())
            })
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
            menu_bar_shown: badge::is_shown(),
            theme: Theme::load(),
        }
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

    fn show_history(&mut self, index: usize) {
        if let Some(bytes) = self.history.image(index) {
            self.open_stored_image(bytes);
            return;
        }
        if let Some(text) = self.history.get(index).map(str::to_string) {
            self.open_preview(&text, PreviewAction::Original);
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
            }
            return;
        }
        self.current_image = None;
        if let Some(text) = view.text()
            && self.should_record(text)
        {
            self.skip_record = None;
            self.history.record(text.to_string());
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
            badge: badge::is_shown(),
        };
        if signature == self.signature {
            return false;
        }
        self.signature = signature;
        true
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

fn version_item() -> MenuItem {
    let label = version_label();
    let item = MenuItem::new(&label, false, None);
    item.set_styled_text(vec![(label, TextStyle::Secondary)]);
    item
}

fn badge_menu() -> Menu {
    let menu = Menu::new();
    let _ = menu.append(&MenuItem::with_id("open", "Open    ⌃⌥⌘F", true, None));
    let _ = menu.append(&MenuItem::with_id(
        "hide_badge",
        "Hide menu bar icon",
        true,
        None,
    ));
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&version_item());
    let _ = menu.append(&IconMenuItem::with_id_and_native_icon(
        "quit",
        "Quit",
        true,
        Some(NativeIcon::StopProgress),
        None,
    ));
    menu
}

fn register_format_hotkey() -> Result<(GlobalHotKeyManager, u32), Box<dyn std::error::Error>> {
    let manager = GlobalHotKeyManager::new()?;
    let hotkey = HotKey::new(
        Some(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SUPER),
        Code::KeyF,
    );
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
        .with_menu(Box::new(badge_menu()))
        .with_menu_on_left_click(false)
        .with_tooltip("Copycraft")
        .build()?;
    if !badge::is_shown() {
        tray.set_visible(false)?;
    }

    let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;
    launcher::install_proxy(event_loop.create_proxy());

    let mut app = App {
        tray,
        history: ClipboardHistory::default(),
        current_image: None,
        recorded_image_change: None,
        skip_image_change: None,
        skip_record: None,
        signature: ClipSig::default(),
        welcomed: false,
        _hotkeys: hotkeys,
        format_hotkey_id,
    };
    event_loop.run_app(&mut app)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::version_label;

    #[test]
    fn version_label_includes_package_version() {
        assert_eq!(
            version_label(),
            format!("Copycraft {}", env!("CARGO_PKG_VERSION"))
        );
    }
}
