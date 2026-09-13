use crate::format::FormatKind;

#[cfg(target_os = "macos")]
mod macos_window;

pub fn show(formatted: &str, kind: FormatKind) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        macos_window::show(formatted, kind)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (formatted, kind);
        Err("native preview is macOS-only".into())
    }
}

#[cfg(test)]
mod tests {
    use super::show;
    use crate::format::FormatKind;

    #[test]
    fn show_is_defined() {
        let _ = show("{\"a\":1}", FormatKind::Json);
    }
}
