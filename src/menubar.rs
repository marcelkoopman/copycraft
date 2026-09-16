use std::time::{Duration, Instant};

use tray_icon::{
    TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{
        IconMenuItem, Menu, MenuEvent, MenuItem, NativeIcon, PredefinedMenuItem, Submenu, TextStyle,
    },
};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
};

use crate::clipboard::{self, ClipboardHistory, ClipboardView};
use crate::format;
use crate::icon;
use crate::preview;
use crate::redact;

const REFRESH: Duration = Duration::from_millis(400);

struct App {
    tray: TrayIcon,
    history: ClipboardHistory,
    last_label: String,
    history_len: usize,
    last_kind: Option<format::FormatKind>,
    skip_record: Option<String>,
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
                "current_format" => self.show_current(),
                "current_redact" => self.show_current_redacted(),
                id if id.starts_with("hist_format_") => {
                    if let Ok(index) = id.trim_start_matches("hist_format_").parse::<usize>() {
                        self.show_history(index);
                    }
                }
                id if id.starts_with("hist_redact_") => {
                    if let Ok(index) = id.trim_start_matches("hist_redact_").parse::<usize>() {
                        self.show_history_redacted(index);
                    }
                }
                _ => {}
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

    fn auto_format(&mut self) {
        let Some(text) = self.current_text() else {
            return;
        };
        let formatted = clipboard::formatted(&text);
        if formatted == text {
            return;
        }
        if let Err(e) = clipboard::write_clipboard(&formatted) {
            eprintln!("auto-format write failed: {e}");
            return;
        }
        if self.should_record(&formatted) {
            self.history.record(formatted);
        } else {
            self.skip_record = Some(formatted);
        }
    }

    fn show_current(&mut self) {
        if let Some(text) = self.current_text() {
            self.open_preview(&text);
        }
    }

    fn show_history(&mut self, index: usize) {
        if let Some(text) = self.history.get(index).map(str::to_string) {
            self.open_preview(&text);
        }
    }

    fn show_current_redacted(&mut self) {
        if let Some(text) = self.current_text() {
            self.open_redacted_preview(&text);
        }
    }

    fn show_history_redacted(&mut self, index: usize) {
        if let Some(text) = self.history.get(index).map(str::to_string) {
            self.open_redacted_preview(&text);
        }
    }

    fn open_preview(&mut self, text: &str) {
        let shown = clipboard::formatted(text);
        let kind = format::detect(&shown);
        if let Err(e) = preview::show(&shown, kind) {
            eprintln!("preview failed: {e}");
        }
        self.rebuild_menu(true);
    }

    fn open_redacted_preview(&mut self, text: &str) {
        let shown = redact::redact(text);
        let kind = format::detect(&shown);
        if let Err(e) = preview::show(&shown, kind) {
            eprintln!("preview failed: {e}");
        }
        self.rebuild_menu(true);
    }

    fn rebuild_menu(&mut self, force: bool) {
        self.auto_format();
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
        let kind = current_text.map(format::detect);
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
        let current = clipboard_entry_submenu(
            "current",
            format!("• {label}"),
            view.text().is_some(),
            view.text(),
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
                let item = clipboard_entry_submenu(
                    &format!("hist_{index}"),
                    item_label,
                    true,
                    self.history.get(index),
                    false,
                );
                let _ = menu.append(&item);
            }
        }

        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&IconMenuItem::with_id_and_native_icon(
            "clear_clipboard",
            "Clear clipboard",
            view.text().is_some(),
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
        let _ = menu.append(&IconMenuItem::with_id_and_native_icon(
            "quit",
            "Quit",
            true,
            Some(NativeIcon::StopProgress),
            None,
        ));
        self.tray.set_menu(Some(Box::new(menu)));
        let _ = self.tray.set_tooltip(Some(label.as_str()));
        self.tray.set_title(None::<&str>);
    }
}

fn clipboard_entry_submenu(
    id: &str,
    title: String,
    enabled: bool,
    text: Option<&str>,
    current: bool,
) -> Submenu {
    let submenu = Submenu::with_id(id, &title, enabled);
    style_entry_submenu(&submenu, text, current);
    let (format_id, redact_id) = if id == "current" {
        ("current_format".to_string(), "current_redact".to_string())
    } else if let Some(rest) = id.strip_prefix("hist_") {
        (format!("hist_format_{rest}"), format!("hist_redact_{rest}"))
    } else {
        (format!("{id}_format"), format!("{id}_redact"))
    };
    let _ = submenu.append(&MenuItem::with_id(format_id, "Format", enabled, None));
    let _ = submenu.append(&MenuItem::with_id(redact_id, "Redact", enabled, None));
    submenu
}

fn style_entry_submenu(item: &Submenu, text: Option<&str>, current: bool) {
    let Some(text) = text else {
        return;
    };
    let (kind, preview) = clipboard::type_and_preview(text);
    let Some(kind) = kind else {
        let plain = if current {
            format!("• {preview}")
        } else {
            preview
        };
        item.set_text(plain);
        return;
    };
    let mut parts: Vec<(String, TextStyle)> = Vec::new();
    if current {
        parts.push(("• ".into(), TextStyle::Default));
    }
    parts.push((kind.into(), TextStyle::Default));
    parts.push((" | ".into(), TextStyle::Secondary));
    parts.push((preview, TextStyle::Secondary));
    item.set_styled_text(parts);
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
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
    };
    app.rebuild_menu(true);

    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut app)?;
    Ok(())
}
