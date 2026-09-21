#![cfg(target_os = "macos")]

use std::path::PathBuf;
use std::sync::Mutex;

use objc2::rc::autoreleasepool;
use objc2_app_kit::{
    NSPasteboard, NSPasteboardTypeFileURL, NSPasteboardTypePNG, NSPasteboardTypeString,
    NSPasteboardTypeTIFF,
};
use objc2_foundation::{NSArray, NSData, NSString, NSURL};

use crate::clipboard::{ClipboardImage, ClipboardView};
use crate::image_ops;

const IMAGE_EXTS: &[&str] = &[
    "png", "jpg", "jpeg", "tif", "tiff", "gif", "bmp", "webp", "heic",
];

struct CachedView {
    change_count: isize,
    view: ClipboardView,
}

static CACHE: Mutex<Option<CachedView>> = Mutex::new(None);

pub(crate) struct DecodedPreview {
    pub image: ClipboardImage,
    pub source_png: Option<Vec<u8>>,
    pub change_count: isize,
}

pub(crate) fn current_view() -> ClipboardView {
    let pasteboard = NSPasteboard::generalPasteboard();
    let change_count = pasteboard.changeCount();
    if let Ok(guard) = CACHE.lock()
        && let Some(cached) = guard.as_ref()
        && cached.change_count == change_count
    {
        return cached.view.clone();
    }
    let view = read_view(&pasteboard);
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some(CachedView {
            change_count,
            view: view.clone(),
        });
    }
    view
}

pub(crate) fn change_count() -> isize {
    NSPasteboard::generalPasteboard().changeCount()
}

pub(crate) fn decode_preview() -> Option<DecodedPreview> {
    let pasteboard = NSPasteboard::generalPasteboard();
    let change_count = pasteboard.changeCount();
    // Copy the encoded bytes out of the pasteboard before decoding so a large
    // image is not decoded while AppKit is still holding the original buffer.
    let source = autoreleasepool(|_| image_source(&pasteboard))?;
    let decoded = match source {
        ImageSource::Bytes { bytes, is_png } => decode_bytes(bytes, is_png)?,
        ImageSource::File(path) => match read_image_file(&path) {
            Some(decoded) => decoded,
            None => {
                let bytes = autoreleasepool(|_| {
                    pasteboard_data(&pasteboard, unsafe { NSPasteboardTypeTIFF })
                })?;
                decode_bytes(bytes, false)?
            }
        },
    };
    Some(DecodedPreview {
        change_count,
        ..decoded
    })
}

fn read_image_file(path: &std::path::Path) -> Option<DecodedPreview> {
    let bytes = std::fs::read(path).ok()?;
    let is_png = path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("png"));
    decode_bytes(bytes, is_png)
}

pub(crate) fn write_png(bytes: &[u8]) -> Result<(), String> {
    let pasteboard = NSPasteboard::generalPasteboard();
    let data = NSData::with_bytes(bytes);
    let ok = unsafe {
        pasteboard.clearContents();
        pasteboard.setData_forType(Some(&data), NSPasteboardTypePNG)
    };
    if let Ok(mut guard) = CACHE.lock() {
        *guard = None;
    }
    if ok {
        Ok(())
    } else {
        Err("could not write image".into())
    }
}

fn read_view(pasteboard: &NSPasteboard) -> ClipboardView {
    autoreleasepool(|_| {
        let text = pasteboard_string(pasteboard, unsafe { NSPasteboardTypeString });
        match text {
            Some(text) if !text.trim().is_empty() => ClipboardView::Text(text),
            other => {
                if has_image(pasteboard) {
                    ClipboardView::Image
                } else if other.is_some() {
                    ClipboardView::Empty
                } else {
                    ClipboardView::NoText
                }
            }
        }
    })
}

fn has_image(pasteboard: &NSPasteboard) -> bool {
    let jpeg = NSString::from_str("public.jpeg");
    let heic = NSString::from_str("public.heic");
    let types = unsafe {
        NSArray::from_slice(&[NSPasteboardTypePNG, NSPasteboardTypeTIFF, &*jpeg, &*heic])
    };
    pasteboard.availableTypeFromArray(&types).is_some() || image_file(pasteboard).is_some()
}

enum ImageSource {
    Bytes { bytes: Vec<u8>, is_png: bool },
    File(PathBuf),
}

fn image_source(pasteboard: &NSPasteboard) -> Option<ImageSource> {
    if has_declared(pasteboard, unsafe { NSPasteboardTypePNG })
        && let Some(bytes) = pasteboard_data(pasteboard, unsafe { NSPasteboardTypePNG })
    {
        return Some(ImageSource::Bytes {
            bytes,
            is_png: true,
        });
    }
    let jpeg = NSString::from_str("public.jpeg");
    if has_declared(pasteboard, &jpeg)
        && let Some(bytes) = pasteboard_data(pasteboard, &jpeg)
    {
        return Some(ImageSource::Bytes {
            bytes,
            is_png: false,
        });
    }
    if let Some(path) = image_file(pasteboard) {
        return Some(ImageSource::File(path));
    }
    if let Some(bytes) = pasteboard_data(pasteboard, unsafe { NSPasteboardTypeTIFF }) {
        return Some(ImageSource::Bytes {
            bytes,
            is_png: false,
        });
    }
    let bytes = pasteboard_data(pasteboard, unsafe { NSPasteboardTypePNG })?;
    Some(ImageSource::Bytes {
        bytes,
        is_png: true,
    })
}

fn decode_bytes(bytes: Vec<u8>, is_png: bool) -> Option<DecodedPreview> {
    let image = image_ops::preview_from_encoded(&bytes)?;
    let source_png = is_png.then_some(bytes);
    Some(DecodedPreview {
        image,
        source_png,
        change_count: 0,
    })
}

fn has_declared(pasteboard: &NSPasteboard, kind: &NSString) -> bool {
    pasteboard
        .types()
        .is_some_and(|types| types.iter().any(|ty| ty.isEqualToString(kind)))
}

fn pasteboard_string(pasteboard: &NSPasteboard, kind: &NSString) -> Option<String> {
    let text = pasteboard.stringForType(kind)?;
    Some(text.to_string())
}

fn pasteboard_data(pasteboard: &NSPasteboard, kind: &NSString) -> Option<Vec<u8>> {
    let data = pasteboard.dataForType(kind)?;
    let bytes = data.to_vec();
    (!bytes.is_empty()).then_some(bytes)
}

fn image_file(pasteboard: &NSPasteboard) -> Option<PathBuf> {
    let url = pasteboard_string(pasteboard, unsafe { NSPasteboardTypeFileURL })?;
    let url = NSString::from_str(&url);
    let path = NSURL::URLWithString(&url)?.path()?.to_string();
    let path = PathBuf::from(path);
    let ext = path.extension()?.to_str()?;
    IMAGE_EXTS
        .iter()
        .any(|candidate| ext.eq_ignore_ascii_case(candidate))
        .then_some(path)
}
