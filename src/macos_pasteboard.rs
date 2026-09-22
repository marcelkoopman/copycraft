#![cfg(target_os = "macos")]

use std::path::PathBuf;
use std::sync::Mutex;

use base64::Engine;
use objc2::rc::autoreleasepool;
use objc2_app_kit::{
    NSPasteboard, NSPasteboardTypeFileURL, NSPasteboardTypePNG, NSPasteboardTypeString,
    NSPasteboardTypeTIFF,
};
use objc2_foundation::{NSArray, NSData, NSString, NSURL};

use crate::clipboard::{ClipboardImage, ClipboardView};
use crate::commands::ImageFacts;
use crate::image_ops;

const IMAGE_EXTS: &[&str] = &[
    "png", "jpg", "jpeg", "tif", "tiff", "gif", "bmp", "webp", "heic",
];

struct CachedView {
    change_count: isize,
    view: ClipboardView,
    image_path: Option<PathBuf>,
}

static CACHE: Mutex<Option<CachedView>> = Mutex::new(None);

#[derive(Clone)]
struct CardSnap {
    change_count: isize,
    facts: ImageFacts,
    thumbnail: ClipboardImage,
}

static CARD: Mutex<Option<CardSnap>> = Mutex::new(None);

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
    let (view, image_path) = read_view(&pasteboard);
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some(CachedView {
            change_count,
            view: view.clone(),
            image_path,
        });
    }
    view
}

fn cached_image_path(change_count: isize) -> Option<PathBuf> {
    let guard = CACHE.lock().ok()?;
    let cached = guard.as_ref()?;
    if cached.change_count == change_count {
        cached.image_path.clone()
    } else {
        None
    }
}

pub(crate) fn change_count() -> isize {
    NSPasteboard::generalPasteboard().changeCount()
}

pub(crate) fn current_image_bytes() -> Option<Vec<u8>> {
    let pasteboard = NSPasteboard::generalPasteboard();
    let change_count = pasteboard.changeCount();
    let cached_path = cached_image_path(change_count);
    autoreleasepool(
        |_| match image_source(&pasteboard, cached_path.as_deref())? {
            ImageSource::Bytes { bytes, .. } => Some(bytes),
            ImageSource::File(path) => std::fs::read(path).ok(),
        },
    )
}

pub(crate) fn decode_preview() -> Option<DecodedPreview> {
    let pasteboard = NSPasteboard::generalPasteboard();
    let change_count = pasteboard.changeCount();
    // Copy the encoded bytes out of the pasteboard before decoding so a large
    // image is not decoded while AppKit is still holding the original buffer.
    let cached_path = cached_image_path(change_count);
    let source = autoreleasepool(|_| image_source(&pasteboard, cached_path.as_deref()))?;
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
    let is_png = path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("png"));
    if is_png {
        let bytes = std::fs::read(path).ok()?;
        return decode_bytes(bytes, true);
    }
    if let Some(image) = crate::macos_image_io::preview_from_path(path) {
        return Some(DecodedPreview {
            image,
            source_png: None,
            change_count: 0,
        });
    }
    let bytes = std::fs::read(path).ok()?;
    decode_bytes(bytes, false)
}

pub(crate) fn write_png(bytes: &[u8]) -> Result<(), String> {
    let pasteboard = NSPasteboard::generalPasteboard();
    let data = NSData::with_bytes(bytes);
    let ok = unsafe {
        pasteboard.clearContents();
        pasteboard.setData_forType(Some(&data), NSPasteboardTypePNG)
    };
    invalidate_caches();
    if ok {
        Ok(())
    } else {
        Err("could not write image".into())
    }
}

pub(crate) fn image_facts() -> Option<ImageFacts> {
    card_snap().map(|snap| snap.facts)
}

pub(crate) fn image_thumbnail() -> Option<ClipboardImage> {
    card_snap().map(|snap| snap.thumbnail)
}

pub(crate) fn image_as_text(data_url: bool) -> Option<String> {
    let exported = export_image()?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&exported.bytes);
    if data_url {
        Some(format!("data:{};base64,{encoded}", exported.mime))
    } else {
        Some(encoded)
    }
}

pub(crate) fn copy_image_file() -> Result<(), String> {
    let exported = export_image().ok_or_else(|| "no image".to_string())?;
    let path = if let Some(path) = exported.path {
        path
    } else {
        let path = std::env::temp_dir().join(format!("copycraft.{}", exported.extension));
        std::fs::write(&path, &exported.bytes).map_err(|err| err.to_string())?;
        path
    };
    set_file_url(&path)
}

fn card_snap() -> Option<CardSnap> {
    let change_count = change_count();
    if let Some(cached) = cached_card(change_count) {
        return Some(cached);
    }
    let built = build_card(change_count)?;
    if let Ok(mut guard) = CARD.lock() {
        *guard = Some(built.clone());
    }
    Some(built)
}

fn cached_card(change_count: isize) -> Option<CardSnap> {
    let guard = CARD.lock().ok()?;
    let cached = guard.as_ref()?;
    (cached.change_count == change_count).then(|| cached.clone())
}

fn build_card(change_count: isize) -> Option<CardSnap> {
    let pasteboard = NSPasteboard::generalPasteboard();
    let cached_path = cached_image_path(change_count);
    let source = autoreleasepool(|_| image_source(&pasteboard, cached_path.as_deref()))?;
    let (byte_len, label_hint, thumbnail) = match source {
        ImageSource::Bytes { bytes, is_png } => {
            let len = bytes.len();
            let label = sniff_image(&bytes, is_png).0;
            let thumbnail = crate::macos_image_io::preview_from_bytes(&bytes)
                .or_else(|| image_ops::preview_from_encoded(&bytes))?;
            (len, label, thumbnail)
        }
        ImageSource::File(path) => {
            let len = std::fs::metadata(&path)
                .ok()
                .and_then(|meta| usize::try_from(meta.len()).ok())
                .unwrap_or(0);
            let label = extension_label(&path);
            let thumbnail = crate::macos_image_io::preview_from_path(&path).or_else(|| {
                let bytes = std::fs::read(&path).ok()?;
                image_ops::preview_from_encoded(&bytes)
            })?;
            (len, label, thumbnail)
        }
    };
    Some(CardSnap {
        change_count,
        facts: ImageFacts {
            format: label_hint.to_string(),
            width: thumbnail.full_width,
            height: thumbnail.full_height,
            byte_len,
        },
        thumbnail,
    })
}

struct ImageExport {
    bytes: Vec<u8>,
    mime: &'static str,
    extension: &'static str,
    path: Option<PathBuf>,
}

fn export_image() -> Option<ImageExport> {
    let pasteboard = NSPasteboard::generalPasteboard();
    let change_count = pasteboard.changeCount();
    let cached_path = cached_image_path(change_count);
    let source = autoreleasepool(|_| image_source(&pasteboard, cached_path.as_deref()))?;
    match source {
        ImageSource::Bytes { bytes, is_png } => {
            let (label, mime) = sniff_image(&bytes, is_png);
            Some(ImageExport {
                bytes,
                mime,
                extension: extension_for(label),
                path: None,
            })
        }
        ImageSource::File(path) => {
            let bytes = std::fs::read(&path).ok()?;
            let from_ext = extension_label(&path);
            let (sniffed, _) = sniff_image(&bytes, from_ext == "PNG");
            let label = if sniffed != "TIFF" {
                sniffed
            } else if from_ext != "Image" {
                from_ext
            } else {
                sniffed
            };
            Some(ImageExport {
                bytes,
                mime: mime_for(label),
                extension: extension_for(label),
                path: Some(path),
            })
        }
    }
}

fn set_file_url(path: &std::path::Path) -> Result<(), String> {
    let text = path.to_str().ok_or_else(|| "path".to_string())?;
    let url = NSURL::fileURLWithPath(&NSString::from_str(text));
    let absolute = url
        .absoluteString()
        .ok_or_else(|| "url".to_string())?
        .to_string();
    let pasteboard = NSPasteboard::generalPasteboard();
    pasteboard.clearContents();
    let ok = pasteboard.setString_forType(&NSString::from_str(&absolute), unsafe {
        NSPasteboardTypeFileURL
    });
    invalidate_caches();
    if ok {
        Ok(())
    } else {
        Err("could not copy file".into())
    }
}

fn invalidate_caches() {
    if let Ok(mut guard) = CACHE.lock() {
        *guard = None;
    }
    if let Ok(mut guard) = CARD.lock() {
        *guard = None;
    }
}

fn sniff_image(bytes: &[u8], hint_png: bool) -> (&'static str, &'static str) {
    if hint_png || bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return ("PNG", "image/png");
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return ("JPEG", "image/jpeg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return ("GIF", "image/gif");
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return ("WEBP", "image/webp");
    }
    ("TIFF", "image/tiff")
}

fn extension_label(path: &std::path::Path) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "PNG",
        "jpg" | "jpeg" => "JPEG",
        "gif" => "GIF",
        "webp" => "WEBP",
        "heic" => "HEIC",
        "bmp" => "BMP",
        "tif" | "tiff" => "TIFF",
        _ => "Image",
    }
}

fn extension_for(label: &str) -> &'static str {
    match label {
        "JPEG" => "jpg",
        "GIF" => "gif",
        "WEBP" => "webp",
        "HEIC" => "heic",
        "BMP" => "bmp",
        "TIFF" => "tiff",
        _ => "png",
    }
}

fn mime_for(label: &str) -> &'static str {
    match label {
        "JPEG" => "image/jpeg",
        "GIF" => "image/gif",
        "WEBP" => "image/webp",
        "HEIC" => "image/heic",
        "BMP" => "image/bmp",
        "TIFF" => "image/tiff",
        _ => "image/png",
    }
}

fn read_view(pasteboard: &NSPasteboard) -> (ClipboardView, Option<PathBuf>) {
    autoreleasepool(|_| {
        let text = pasteboard_string(pasteboard, unsafe { NSPasteboardTypeString });
        // Copying a PNG file also puts its name on the pasteboard. That name is
        // the file, so it still counts as an image. Other text stays text.
        let copied_file = image_file(pasteboard);
        match text {
            Some(text)
                if !text.trim().is_empty()
                    && !names_copied_image(text.trim(), copied_file.as_deref()) =>
            {
                (ClipboardView::Text(text), None)
            }
            other => {
                if copied_file.is_some() || has_image_data(pasteboard) {
                    (ClipboardView::Image, copied_file)
                } else if other.is_some() {
                    (ClipboardView::Empty, None)
                } else {
                    (ClipboardView::NoText, None)
                }
            }
        }
    })
}

fn names_copied_image(text: &str, path: Option<&std::path::Path>) -> bool {
    let Some(path) = path else {
        return false;
    };
    let full = path.to_string_lossy();
    if text == full {
        return true;
    }
    if path.file_name().and_then(|name| name.to_str()) == Some(text) {
        return true;
    }
    text.strip_prefix("file://") == Some(full.as_ref())
}

fn has_image_data(pasteboard: &NSPasteboard) -> bool {
    let jpeg = NSString::from_str("public.jpeg");
    let heic = NSString::from_str("public.heic");
    let types = unsafe {
        NSArray::from_slice(&[NSPasteboardTypePNG, NSPasteboardTypeTIFF, &*jpeg, &*heic])
    };
    pasteboard.availableTypeFromArray(&types).is_some()
}

enum ImageSource {
    Bytes { bytes: Vec<u8>, is_png: bool },
    File(PathBuf),
}

fn image_source(
    pasteboard: &NSPasteboard,
    cached_path: Option<&std::path::Path>,
) -> Option<ImageSource> {
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
    if let Some(path) = cached_path {
        return Some(ImageSource::File(path.to_path_buf()));
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
    let image = crate::macos_image_io::preview_from_bytes(&bytes)
        .or_else(|| image_ops::preview_from_encoded(&bytes))?;
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
    if type_available(pasteboard, unsafe { NSPasteboardTypeFileURL })
        && let Some(path) = path_from_file_url(pasteboard).filter(|path| is_image_file(path))
    {
        return Some(path);
    }
    let filenames = NSString::from_str("NSFilenamesPboardType");
    if type_available(pasteboard, &filenames) {
        return filenames_image(pasteboard);
    }
    None
}

fn type_available(pasteboard: &NSPasteboard, kind: &NSString) -> bool {
    let types = NSArray::from_slice(&[kind]);
    pasteboard.availableTypeFromArray(&types).is_some()
}

fn path_from_file_url(pasteboard: &NSPasteboard) -> Option<PathBuf> {
    let url = pasteboard_string(pasteboard, unsafe { NSPasteboardTypeFileURL })?;
    let url = NSString::from_str(&url);
    let path = NSURL::URLWithString(&url)?.path()?.to_string();
    Some(PathBuf::from(path))
}

fn filenames_image(pasteboard: &NSPasteboard) -> Option<PathBuf> {
    let kind = NSString::from_str("NSFilenamesPboardType");
    let list = pasteboard.propertyListForType(&kind)?;
    let array = list.downcast::<NSArray>().ok()?;
    if array.count() != 1 {
        return None;
    }
    let path = PathBuf::from(
        array
            .objectAtIndex(0)
            .downcast::<NSString>()
            .ok()?
            .to_string(),
    );
    is_image_file(&path).then_some(path)
}

fn is_image_file(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            IMAGE_EXTS
                .iter()
                .any(|candidate| ext.eq_ignore_ascii_case(candidate))
        })
        && path.is_file()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{extension_label, names_copied_image, sniff_image};

    #[test]
    fn copied_png_filename_is_the_image() {
        let path = Path::new("/tmp/ed38402972604e86f4303a04442655a5171d7860.png");
        assert!(names_copied_image(
            "ed38402972604e86f4303a04442655a5171d7860.png",
            Some(path)
        ));
        assert!(names_copied_image(
            "/tmp/ed38402972604e86f4303a04442655a5171d7860.png",
            Some(path)
        ));
        assert!(names_copied_image(
            "file:///tmp/ed38402972604e86f4303a04442655a5171d7860.png",
            Some(path)
        ));
    }

    #[test]
    fn sniff_names_png_jpeg_and_a_heic_file() {
        assert_eq!(sniff_image(b"\x89PNG\r\n", true).0, "PNG");
        assert_eq!(sniff_image(&[0xFF, 0xD8, 0xFF, 0], false).0, "JPEG");
        assert_eq!(extension_label(Path::new("/tmp/shot.heic")), "HEIC");
    }

    #[test]
    fn document_text_stays_text_when_an_image_file_is_also_present() {
        let path = Path::new("/tmp/shot.png");
        assert!(!names_copied_image("hello", Some(path)));
        assert!(!names_copied_image("shot.png\nmore", Some(path)));
        assert!(!names_copied_image("shot.png", None));
    }
}
