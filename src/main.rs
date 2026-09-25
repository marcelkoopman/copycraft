mod appearance;
mod clipboard;
mod commands;
mod compress;
mod convert;
mod dataframe;
mod decode;
mod format;
mod highlight;
mod hotkey;
mod icon;
mod image_ops;
mod launcher;
mod menubar;
mod page_preview;
mod preview;
mod redact;
mod toolbar_visibility;
mod transform;
mod validate;
mod youtube;

#[cfg(target_os = "macos")]
mod macos_fetch;
#[cfg(target_os = "macos")]
mod macos_image_io;
#[cfg(target_os = "macos")]
mod macos_launcher;
#[cfg(target_os = "macos")]
mod macos_pasteboard;
#[cfg(target_os = "macos")]
mod macos_preview_image;
#[cfg(target_os = "macos")]
mod macos_preview_text;
#[cfg(target_os = "macos")]
mod macos_vision;
#[cfg(target_os = "macos")]
mod macos_window;

fn main() {
    if let Err(e) = menubar::run() {
        eprintln!("copycraft failed: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name() {
        assert_eq!(env!("CARGO_PKG_NAME"), "copycraft");
    }
}
