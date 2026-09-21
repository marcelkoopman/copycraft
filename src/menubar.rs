use std::time::{Duration, Instant};

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use tray_icon::{
    TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{
        CheckMenuItem, IconMenuItem, Menu, MenuEvent, MenuItem, NativeIcon, PredefinedMenuItem,
        Submenu, TextStyle,
    },
};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
};

use crate::appearance::{self, Theme};
use crate::clipboard::{self, ClipboardHistory, ClipboardView};
use crate::format;
use crate::icon;
use crate::preview;

const REFRESH: Duration = Duration::from_millis(400);

struct App {
    tray: TrayIcon,
    history: ClipboardHistory,
    last_label: String,
    history_len: usize,
    last_kind: Option<format::FormatKind>,
    skip_record: Option<String>,
    _hotkeys: GlobalHotKeyManager,
    format_hotkey_id: u32,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, _: &ActiveEventLoop) {}

    fn window_event(&mut self, _: &ActiveEventLoop, _: winit::window::WindowId, _: WindowEvent) {}

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            let id = event.id.0.as_str();
            match id {
                "quit" => {
                    event_loop.exit();
                    return;
                }
                "clear" => self.clear_history(),
                "clear_clipboard" => self.clear_clipboard(),
                "current" => self.show_current(),
                "format_preview" => self.format_and_preview(),
                id if id.starts_with("hist_") => {
                    if let Ok(index) = id.trim_start_matches("hist_").parse::<usize>() {
                        self.show_history(index);
                    }
                }
                id if Theme::from_id(id).is_some() => {
                    if let Some(theme) = Theme::from_id(id) {
                        theme.save();
                        self.rebuild_menu(true);
                    }
                }
                _ => {}
            }
        }

        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.id == self.format_hotkey_id && event.state == HotKeyState::Pressed {
                self.format_and_preview();
            }
        }

        while TrayIconEvent::receiver().try_recv().is_ok() {
            self.rebuild_menu(false);
        }

        self.rebuild_menu(false);
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + REFRESH));
    }
}

impl App {
    fn current_text(&self) -> Option<String> {
        ClipboardView::from_os().text().map(str::to_string)
    }

    fn format_and_preview(&mut self) {
        self.show_current();
    }

    fn clear_history(&mut self) {
        self.skip_record = self.current_text();
        self.history.clear();
        self.last_label.clear();
        self.history_len = 0;
        self.rebuild_menu(true);
    }

    fn clear_clipboard(&mut self) {
        if let Err(e) = clipboard::clear_clipboard() {
            eprintln!("clear clipboard failed: {e}");
            return;
        }
        self.skip_record = Some(String::new());
        self.last_label.clear();
        self.rebuild_menu(true);
    }

    fn should_record(&self, text: &str) -> bool {
        self.skip_record.as_deref() != Some(text)
    }

    fn show_current(&mut self) {
        let view = ClipboardView::from_os();
        if view.is_image() {
            self.open_image_preview();
            return;
        }
        if let Some(text) = view.text() {
            self.open_preview(text);
        }
    }

    fn show_history(&mut self, index: usize) {
        if let Some(text) = self.history.get(index).map(str::to_string) {
            self.open_preview(&text);
        }
    }

    fn open_preview(&mut self, text: &str) {
        let kind = format::detect(text);
        if let Err(e) = preview::show(text, kind) {
            eprintln!("preview failed: {e}");
        }
        self.rebuild_menu(true);
    }

    fn open_image_preview(&mut self) {
        if let Err(e) = preview::show_clipboard_image() {
            eprintln!("preview failed: {e}");
        }
        self.rebuild_menu(true);
    }

    fn rebuild_menu(&mut self, force: bool) {
        let view = ClipboardView::from_os();
        if let Some(text) = view.text()
            && self.should_record(text)
        {
            self.skip_record = None;
            self.history.record(text.to_string());
        }

        let label = view.label();
        let current_text = view.text();
        let history_len = self
            .history
            .labels()
            .into_iter()
            .filter(|(index, _)| self.history.get(*index) != current_text)
            .count();
        let kind = match &view {
            ClipboardView::Text(text) => Some(format::detect(text)),
            ClipboardView::Image => Some(format::FormatKind::Image),
            ClipboardView::Empty | ClipboardView::NoText => None,
        };
        if !force
            && label == self.last_label
            && history_len == self.history_len
            && kind == self.last_kind
        {
            return;
        }
        self.last_label = label.clone();
        self.history_len = history_len;
        if force || kind != self.last_kind {
            self.last_kind = kind;
            let accent = icon::accent_for_kind(kind);
            match icon::menu_icon_tinted(accent) {
                Ok(tray_icon) => {
                    if let Err(e) = self.tray.set_icon_with_as_template(Some(tray_icon), false) {
                        eprintln!("set tray icon failed: {e}");
                    }
                }
                Err(e) => eprintln!("build tray icon failed: {e}"),
            }
        }

        let history_rows: Vec<(usize, String)> = self
            .history
            .labels()
            .into_iter()
            .filter(|(index, _)| self.history.get(*index) != current_text)
            .collect();

        let menu = Menu::new();
        let _ = menu.append(&MenuItem::new("Current", false, None));
        let current = clipboard_entry_item(
            "current",
            format!("• {label}"),
            view.is_previewable(),
            view.type_mark(),
            true,
        );
        let _ = menu.append(&current);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let history_title = if history_rows.is_empty() {
            "History".to_string()
        } else {
            format!("History ({})", history_rows.len())
        };
        let _ = menu.append(&MenuItem::new(history_title, false, None));

        if history_rows.is_empty() {
            let _ = menu.append(&MenuItem::new("(no history yet)", false, None));
        } else {
            for (index, item_label) in history_rows {
                let item = clipboard_entry_item(
                    &format!("hist_{index}"),
                    item_label,
                    true,
                    self.history.get(index).map(clipboard::menu_mark),
                    false,
                );
                let _ = menu.append(&item);
            }
        }

        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&MenuItem::with_id(
            "format_preview",
            "Format & preview    ⌃⌥⌘F",
            true,
            None,
        ));
        let _ = menu.append(&appearance_menu());
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&IconMenuItem::with_id_and_native_icon(
            "clear_clipboard",
            "Clear clipboard",
            view.is_previewable(),
            Some(NativeIcon::TrashEmpty),
            None,
        ));
        let _ = menu.append(&IconMenuItem::with_id_and_native_icon(
            "clear",
            "Clear history",
            !self.history.is_empty(),
            Some(NativeIcon::TrashFull),
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
        self.tray.set_menu(Some(Box::new(menu)));
        let _ = self.tray.set_tooltip(Some("Copycraft"));
        self.tray.set_title(None::<&str>);
    }
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

fn appearance_menu() -> Submenu {
    let current = Theme::load();
    let menu = Submenu::new("Appearance", true);
    let _ = menu.append(&CheckMenuItem::with_id(
        Theme::System.as_id(),
        "System",
        true,
        current == Theme::System,
        None,
    ));
    let _ = menu.append(&CheckMenuItem::with_id(
        Theme::Light.as_id(),
        "Light",
        true,
        current == Theme::Light,
        None,
    ));
    let _ = menu.append(&CheckMenuItem::with_id(
        Theme::Dark.as_id(),
        "Dark",
        true,
        current == Theme::Dark,
        None,
    ));
    menu
}

fn clipboard_entry_item(
    id: &str,
    title: String,
    enabled: bool,
    mark: Option<&str>,
    current: bool,
) -> MenuItem {
    let item = MenuItem::with_id(id, &title, enabled, None);
    style_entry_item(&item, mark, current);
    item
}

fn style_entry_item(item: &MenuItem, mark: Option<&str>, current: bool) {
    let Some(mark) = mark else {
        return;
    };
    let mark = mark.to_string();
    if current {
        item.set_styled_text(vec![
            ("• ".to_string(), TextStyle::Default),
            (mark, TextStyle::Secondary),
        ]);
    } else {
        item.set_styled_text(vec![(mark, TextStyle::Secondary)]);
    }
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
    let menu = Menu::new();
    let _ = menu.append(&MenuItem::new("Clipboard", false, None));
    let _ = menu.append(&MenuItem::with_id("quit", "Quit", true, None));

    let tray = TrayIconBuilder::new()
        .with_icon(icon)
        .with_icon_as_template(false)
        .with_menu(Box::new(menu))
        .with_tooltip("Copycraft")
        .build()?;

    let mut app = App {
        tray,
        history: ClipboardHistory::default(),
        last_label: String::new(),
        history_len: 0,
        last_kind: None,
        skip_record: None,
        _hotkeys: hotkeys,
        format_hotkey_id,
    };
    app.rebuild_menu(true);

    let event_loop = EventLoop::new()?;
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
