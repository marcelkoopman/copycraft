use std::sync::Mutex;

use winit::event_loop::EventLoopProxy;

use crate::commands::{CommandId, LaunchData};

#[derive(Debug, Clone)]
pub enum UserEvent {
    Run(CommandId),
}

static PROXY: Mutex<Option<EventLoopProxy<UserEvent>>> = Mutex::new(None);

pub fn install_proxy(proxy: EventLoopProxy<UserEvent>) {
    *PROXY.lock().expect("launcher proxy") = Some(proxy);
}

pub fn emit(event: UserEvent) {
    let guard = PROXY.lock().expect("launcher proxy");
    if let Some(proxy) = guard.as_ref() {
        let _ = proxy.send_event(event);
    }
}

pub fn summon(data: LaunchData) {
    #[cfg(target_os = "macos")]
    crate::macos_launcher::summon(data);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = data;
    }
}

pub fn reveal(data: LaunchData) {
    #[cfg(target_os = "macos")]
    crate::macos_launcher::reveal(data);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = data;
    }
}

pub fn sync(data: LaunchData) {
    #[cfg(target_os = "macos")]
    crate::macos_launcher::sync(data);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = data;
    }
}

pub fn is_open() -> bool {
    #[cfg(target_os = "macos")]
    {
        crate::macos_launcher::is_open()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}
