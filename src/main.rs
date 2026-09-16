mod clipboard;
mod dataframe;
mod format;
mod highlight;
mod icon;
mod menubar;
mod preview;
mod redact;
mod transform;

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
