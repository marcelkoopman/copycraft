use crate::clipboard::ClipboardImage;
use crate::format::FormatKind;

pub fn show(formatted: &str, kind: FormatKind) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        crate::macos_window::show(formatted, kind)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (formatted, kind);
        Err("native preview is macOS-only".into())
    }
}

pub fn show_clipboard_image() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        crate::macos_window::show_clipboard_image()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("native preview is macOS-only".into())
    }
}

pub fn show_stored_image(bytes: Vec<u8>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        crate::macos_window::show_stored_image(bytes)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = bytes;
        Err("native preview is macOS-only".into())
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn show_image(image: &ClipboardImage) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        crate::macos_window::show_image(image)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = image;
        Err("native preview is macOS-only".into())
    }
}

#[cfg(test)]
mod tests {
    use super::{show, show_clipboard_image, show_image, show_stored_image};
    use crate::clipboard::ClipboardImage;
    use crate::format::FormatKind;

    #[test]
    fn show_is_defined() {
        let _ = show("{\"a\":1}", FormatKind::Json);
    }

    #[test]
    fn show_image_is_defined() {
        let image = ClipboardImage::new(1, 1, vec![0, 0, 0, 255]).expect("rgba");
        let _ = show_image(&image);
        let _ = show_clipboard_image();
        let _ = show_stored_image(vec![137, 80, 78, 71, 13, 10, 26, 10]);
    }
}
