use std::time::{Duration, Instant};

use tray_icon::{
    TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
};

use crate::clipboard::{self, ClipboardHistory, ClipboardView};
use crate::icon;

const REFRESH: Duration = Duration::from_millis(400);

struct App {
    tray: TrayIcon,
    history: ClipboardHistory,
    last_label: String,
    history_len: usize,
    preview: Option<String>,
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
                "clear" => {
                    self.history.clear();
                    self.preview = None;
                    self.rebuild_menu(true);
                }
                "current" => self.show_current(),
                id if id.starts_with("hist_") => {
                    if let Ok(index) = id.trim_start_matches("hist_").parse::<usize>() {
                        self.show_history(index);
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
    fn show_current(&mut self) {
        if let Some(text) = ClipboardView::from_os().text().map(str::to_string) {
            self.open_formatted(&text);
        }
    }

    fn show_history(&mut self, index: usize) {
        if let Some(text) = self.history.get(index).map(str::to_string) {
            self.open_formatted(&text);
        }
    }

    fn open_formatted(&mut self, text: &str) {
        let shown = clipboard::formatted(text);
        let _ = clipboard::write_clipboard(&shown);
        self.history.record(shown.clone());
        self.preview = Some(shown);
        self.rebuild_menu(true);
    }

    fn rebuild_menu(&mut self, force: bool) {
        let view = ClipboardView::from_os();
        if let Some(text) = view.text() {
            self.history.record(text.to_string());
        }

        let label = view.label();
        let history_len = self.history.labels().len();
        if !force && label == self.last_label && history_len == self.history_len {
            return;
        }
        self.last_label = label.clone();
        self.history_len = history_len;

        let menu = Menu::new();
        let _ = menu.append(&MenuItem::new("Current", false, None));
        let _ = menu.append(&MenuItem::with_id(
            "current",
            format!("• {label}"),
            true,
            None,
        ));

        if let Some(preview) = &self.preview {
            let _ = menu.append(&PredefinedMenuItem::separator());
            let _ = menu.append(&MenuItem::new(clipboard::preview_heading(preview), false, None));
            for line in clipboard::preview_lines(preview) {
                let _ = menu.append(&MenuItem::new(line, false, None));
            }
        }

        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&MenuItem::new("History", false, None));

        if self.history.is_empty() {
            let _ = menu.append(&MenuItem::new("(no history yet)", false, None));
        } else {
            for (index, item_label) in self.history.labels() {
                let id = format!("hist_{index}");
                let _ = menu.append(&MenuItem::with_id(id, item_label, true, None));
            }
        }

        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&MenuItem::with_id(
            "clear",
            "Clear history",
            !self.history.is_empty(),
            None,
        ));
        let _ = menu.append(&MenuItem::with_id("quit", "Quit", true, None));
        self.tray.set_menu(Some(Box::new(menu)));
        let _ = self.tray.set_tooltip(Some(label.as_str()));
    }
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let icon = icon::menu_icon()?;
    let menu = Menu::new();
    let _ = menu.append(&MenuItem::new("Clipboard", false, None));
    let _ = menu.append(&MenuItem::with_id("quit", "Quit", true, None));

    let tray = TrayIconBuilder::new()
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .with_tooltip("Copycraft")
        .build()?;

    let mut app = App {
        tray,
        history: ClipboardHistory::default(),
        last_label: String::new(),
        history_len: 0,
        preview: None,
    };
    app.rebuild_menu(true);

    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut app)?;
    Ok(())
}
