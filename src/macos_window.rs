#![cfg(target_os = "macos")]

use std::sync::Mutex;

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSApplication, NSBackingStoreType, NSColor, NSFont, NSScrollView, NSTextView, NSView,
    NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use crate::clipboard;
use crate::format::FormatKind;

static WINDOW: Mutex<Option<Retained<NSWindow>>> = Mutex::new(None);

pub fn show(formatted: &str, kind: FormatKind) -> Result<(), String> {
    let mtm = MainThreadMarker::new().ok_or("preview must run on the main thread")?;
    let title = kind.preview_heading();
    let body = formatted.to_string();

    let app = NSApplication::sharedApplication(mtm);
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);

    let frame = NSRect::new(NSPoint::new(240.0, 180.0), NSSize::new(760.0, 540.0));
    let style = NSWindowStyleMask::Titled
        | NSWindowStyleMask::Closable
        | NSWindowStyleMask::Resizable
        | NSWindowStyleMask::Miniaturizable;
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            frame,
            style,
            NSBackingStoreType::NSBackingStoreBuffered,
            false,
        )
    };
    unsafe { window.setReleasedWhenClosed(false) };
    window.setTitle(&NSString::from_str(&format!("{title} — Copycraft")));
    window.setBackgroundColor(Some(&NSColor::blackColor()));

    let content = window
        .contentView()
        .ok_or("window has no content view")?;
    let bounds = content.bounds();
    let scroll = unsafe { NSScrollView::initWithFrame(NSScrollView::alloc(mtm), bounds) };
    scroll.setHasVerticalScroller(true);
    scroll.setHasHorizontalScroller(true);
    scroll.setAutoresizingMask(
        objc2_app_kit::NSAutoresizingMaskOptions::ViewWidthSizable
            | objc2_app_kit::NSAutoresizingMaskOptions::ViewHeightSizable,
    );

    let text = unsafe { NSTextView::initWithFrame(NSTextView::alloc(mtm), bounds) };
    text.setEditable(false);
    text.setSelectable(true);
    text.setDrawsBackground(true);
    text.setBackgroundColor(&NSColor::colorWithCalibratedRed_green_blue_alpha(
        0.12, 0.12, 0.13, 1.0,
    ));
    text.setTextColor(Some(&NSColor::colorWithCalibratedRed_green_blue_alpha(
        0.91, 0.91, 0.93, 1.0,
    )));
    text.setFont(Some(&NSFont::monospacedSystemFontOfSize_weight(
        13.0,
        objc2_app_kit::NSFontWeightRegular,
    )));
    text.setString(&NSString::from_str(&format!(
        "Select all and press Cmd+C to copy.\n\n{body}"
    )));
    scroll.setDocumentView(Some(&text));
    content.addSubview(&scroll);

    window.center();
    window.makeKeyAndOrderFront(None);
    window.orderFrontRegardless();

    let _ = clipboard::write_clipboard(&body);
    *WINDOW.lock().map_err(|e| e.to_string())? = Some(window);
    Ok(())
}
