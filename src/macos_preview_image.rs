#![cfg(target_os = "macos")]

use objc2::AnyThread;
use objc2::rc::Retained;
use objc2_app_kit::{NSBitmapFormat, NSBitmapImageRep, NSCalibratedRGBColorSpace, NSImage};
use objc2_foundation::{NSData, NSSize};

use crate::clipboard::ClipboardImage;

pub(crate) fn nsimage_from_bytes(bytes: &[u8]) -> Option<Retained<NSImage>> {
    let data = NSData::with_bytes(bytes);
    NSImage::initWithData(NSImage::alloc(), &data)
}

pub(crate) fn nsimage_from_clipboard(image: &ClipboardImage) -> Option<Retained<NSImage>> {
    nsimage_from_rgba(image).or_else(|| nsimage_from_bytes(&image.png_bytes().ok()?))
}

fn nsimage_from_rgba(image: &ClipboardImage) -> Option<Retained<NSImage>> {
    let width = image.width as isize;
    let height = image.height as isize;
    let row = width.checked_mul(4)?;
    let rep = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bitmapFormat_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            width,
            height,
            8,
            4,
            true,
            false,
            NSCalibratedRGBColorSpace,
            NSBitmapFormat::AlphaNonpremultiplied,
            row,
            32,
        )
    }?;
    unsafe {
        let dest = rep.bitmapData();
        if dest.is_null() {
            return None;
        }
        dest.copy_from(image.rgba.as_ptr(), image.rgba.len());
    }
    let nsimage = NSImage::initWithSize(
        NSImage::alloc(),
        NSSize::new(image.width as f64, image.height as f64),
    );
    nsimage.addRepresentation(&rep);
    Some(nsimage)
}
