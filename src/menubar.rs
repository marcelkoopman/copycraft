use std::time::{Duration, Instant};

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
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
use crate::format;
use crate::icon;
use crate::preview;
use crate::transform::{self, Transform};

const REFRESH: Duration = Duration::from_millis(400);

struct Hotkeys {
    pretty: HotKey,
    minify: HotKey,
    to_yaml: HotKey,
    to_json: HotKey,
    preview: HotKey,
    _manager: GlobalHotKeyManager,
}

impl Hotkeys {
    fn register() -> Option<Self> {
        let manager = GlobalHotKeyManager::new().ok()?;
        let mods = Modifiers::META | Modifiers::SHIFT;
        let pretty = HotKey::new(Some(mods), Code::KeyJ);
        let minify = HotKey::new(Some(mods), Code::KeyM);
        let to_yaml = HotKey::new(Some(mods), Code::KeyY);
        let to_json = HotKey::new(Some(mods), Code::KeyU);
        let preview = HotKey::new(Some(mods), Code::KeyP);
        manager.register(pretty).ok()?;
        manager.register(minify).ok()?;
        manager.register(to_yaml).ok()?;
        manager.register(to_json).ok()?;
        manager.register(preview).ok()?;
        Some(Self {
            pretty,
            minify,
            to_yaml,
            to_json,
            preview,
            _manager: manager,
        })
    }
}

struct App {
    tray: TrayIcon,
    history: ClipboardHistory,
    last_label: String,
    history_len: usize,
    hotkeys: Option<Hotkeys>,
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
                    self.rebuild_menu(true);
                }
                "current" => self.show_current(),
                id if id.starts_with("hist_") => {
                    if let Ok(index) = id.trim_start_matches("hist_").parse::<usize>() {
                        self.show_history(index);
                    }
                }
                id => {
                    if let Some(action) = Transform::from_id(id) {
                        self.apply_transform(action);
                    }
                }
            }
        }

        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.state != HotKeyState::Pressed {
                continue;
            }
            if let Some(hotkeys) = &self.hotkeys {
                if event.id == hotkeys.pretty.id() {
                    self.apply_transform(Transform::PrettyJson);
                } else if event.id == hotkeys.minify.id() {
                    self.apply_transform(Transform::MinifyJson);
                } else if event.id == hotkeys.to_yaml.id() {
                    self.apply_transform(Transform::JsonToYaml);
                } else if event.id == hotkeys.to_json.id() {
                    self.apply_transform(Transform::YamlToJson);
                } else if event.id == hotkeys.preview.id() {
                    self.show_current();
                }
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

    fn apply_transform(&mut self, action: Transform) {
        let Some(text) = self.current_text() else {
            return;
        };
        match transform::apply(action, &text) {
            Ok(next) => {
                if let Err(e) = clipboard::write_clipboard(&next) {
                    eprintln!("clipboard write failed: {e}");
                    return;
                }
                self.history.record(next);
                self.rebuild_menu(true);
            }
            Err(e) => eprintln!("transform failed: {e}"),
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
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&MenuItem::new("Transform", false, None));
        let _ = menu.append(&MenuItem::with_id(
            Transform::PrettyJson.id(),
            "Pretty JSON    ⌘⇧J",
            true,
            None,
        ));
        let _ = menu.append(&MenuItem::with_id(
            Transform::MinifyJson.id(),
            "Minify JSON    ⌘⇧M",
            true,
            None,
        ));
        let _ = menu.append(&MenuItem::with_id(
            Transform::JsonToYaml.id(),
            "JSON → YAML    ⌘⇧Y",
            true,
            None,
        ));
        let _ = menu.append(&MenuItem::with_id(
            Transform::YamlToJson.id(),
            "YAML → JSON    ⌘⇧U",
            true,
            None,
        ));
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

    let hotkeys = Hotkeys::register();
    if hotkeys.is_none() {
        eprintln!("global hotkeys unavailable (permissions or already registered)");
    }

    let mut app = App {
        tray,
        history: ClipboardHistory::default(),
        last_label: String::new(),
        history_len: 0,
        hotkeys,
    };
    app.rebuild_menu(true);

    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut app)?;
    Ok(())
}
