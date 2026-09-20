#![cfg(target_os = "macos")]

use objc2::AnyThread;
use objc2::rc::Retained;
use objc2_app_kit::NSImage;
use objc2_foundation::NSData;

use crate::clipboard::ClipboardImage;

pub(crate) fn nsimage_from_clipboard(image: &ClipboardImage) -> Option<Retained<NSImage>> {
    let png = image.png_bytes().ok()?;
    let data = NSData::with_bytes(&png);
    NSImage::initWithData(NSImage::alloc(), &data)
}
