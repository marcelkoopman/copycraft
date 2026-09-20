#![cfg(target_os = "macos")]

use objc2::AnyThread;
use objc2::rc::Retained;
use objc2_app_kit::NSImage;
use objc2_foundation::NSData;

use crate::clipboard::ClipboardImage;

pub(crate) fn nsimage_from_bytes(bytes: &[u8]) -> Option<Retained<NSImage>> {
    let data = NSData::with_bytes(bytes);
    NSImage::initWithData(NSImage::alloc(), &data)
}

pub(crate) fn nsimage_from_clipboard(image: &ClipboardImage) -> Option<Retained<NSImage>> {
    nsimage_from_bytes(&image.png_bytes().ok()?)
}
