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

use crate::clipboard::ClipboardView;
use crate::icon;

const REFRESH: Duration = Duration::from_millis(400);

struct App {
    tray: TrayIcon,
    last_preview: Vec<String>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, _: &ActiveEventLoop) {}

    fn window_event(&mut self, _: &ActiveEventLoop, _: winit::window::WindowId, _: WindowEvent) {}

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            match event.id.0.as_str() {
                "quit" => {
                    event_loop.exit();
                    return;
                }
                "refresh" => self.refresh_menu(),
                _ => {}
            }
        }

        while let Ok(_) = TrayIconEvent::receiver().try_recv() {
            self.refresh_menu();
        }

        self.refresh_menu();
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + REFRESH));
    }
}

impl App {
    fn refresh_menu(&mut self) {
        let view = ClipboardView::from_os();
        let lines = view.menu_lines();
        if lines == self.last_preview {
            return;
        }
        self.last_preview = lines.clone();

        let menu = Menu::new();
        let _ = menu.append(&MenuItem::new("Clipboard", false, None));
        let _ = menu.append(&PredefinedMenuItem::separator());
        for line in &lines {
            let _ = menu.append(&MenuItem::new(line, false, None));
        }
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&MenuItem::with_id("refresh", "Refresh", true, None));
        let _ = menu.append(&MenuItem::with_id("quit", "Quit", true, None));
        self.tray.set_menu(Some(Box::new(menu)));
        self.tray.set_tooltip(Some("Copycraft — clipboard"));
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
        .with_tooltip("Copycraft — clipboard")
        .build()?;

    let mut app = App {
        tray,
        last_preview: Vec::new(),
    };
    app.refresh_menu();

    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut app)?;
    Ok(())
}
