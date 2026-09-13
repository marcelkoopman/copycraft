#![cfg(target_os = "macos")]

use std::cell::RefCell;

use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol};
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSAutoresizingMaskOptions, NSBackingStoreType, NSButton, NSColor, NSControl,
    NSFont, NSFontWeightRegular, NSForegroundColorAttributeName, NSScrollView, NSTextView,
    NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{
    NSMutableAttributedString, NSPoint, NSRange, NSRect, NSSize, NSString,
};

use crate::clipboard;
use crate::format::FormatKind;
use crate::highlight::{self, TokenKind};

thread_local! {
    static WINDOW: RefCell<Option<Retained<NSWindow>>> = const { RefCell::new(None) };
    static TARGET: RefCell<Option<Retained<CopyTarget>>> = const { RefCell::new(None) };
    static PREVIEW_TEXT: RefCell<String> = const { RefCell::new(String::new()) };
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "CopycraftCopyTarget"]
    struct CopyTarget;

    impl CopyTarget {
        #[unsafe(method(copyClicked:))]
        fn copy_clicked(&self, _sender: Option<&AnyObject>) {
            PREVIEW_TEXT.with(|text| {
                let _ = clipboard::write_clipboard(&text.borrow());
            });
        }
    }
);

impl CopyTarget {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this: Allocated<Self> = Self::alloc(mtm);
        unsafe { msg_send![this, init] }
    }
}

pub fn show(formatted: &str, kind: FormatKind) -> Result<(), String> {
    let mtm = MainThreadMarker::new().ok_or("preview must run on the main thread")?;
    let title = kind.preview_heading();
    let body = formatted.to_string();
    PREVIEW_TEXT.with(|slot| slot.replace(body.clone()));

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
            NSBackingStoreType::Buffered,
            false,
        )
    };
    unsafe { window.setReleasedWhenClosed(false) };
    window.setTitle(&NSString::from_str(&format!("{title} — Copycraft")));
    window.setBackgroundColor(Some(&NSColor::blackColor()));

    let content = window
        .contentView()
        .ok_or("window has no content view")?;

    let button = NSButton::initWithFrame(
        NSButton::alloc(mtm),
        NSRect::new(NSPoint::new(16.0, 504.0), NSSize::new(180.0, 28.0)),
    );
    button.setTitle(&NSString::from_str("Copy to clipboard"));
    let target = CopyTarget::new(mtm);
    button.setTarget(Some(&target));
    unsafe { button.setAction(Some(sel!(copyClicked:))) };

    let scroll = NSScrollView::initWithFrame(
        NSScrollView::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(760.0, 496.0)),
    );
    scroll.setHasVerticalScroller(true);
    scroll.setHasHorizontalScroller(true);
    scroll.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );

    let text = NSTextView::initWithFrame(
        NSTextView::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(760.0, 496.0)),
    );
    text.setEditable(false);
    text.setSelectable(true);
    text.setDrawsBackground(true);
    text.setBackgroundColor(&NSColor::colorWithCalibratedRed_green_blue_alpha(
        0.12, 0.12, 0.13, 1.0,
    ));
    let weight = unsafe { NSFontWeightRegular };
    text.setFont(Some(&NSFont::monospacedSystemFontOfSize_weight(13.0, weight)));
    if let Some(storage) = text.textStorage() {
        storage.setAttributedString(&colored_text(&body, kind));
    } else {
        text.setString(&NSString::from_str(&body));
    }
    scroll.setDocumentView(Some(&text));

    content.addSubview(&scroll);
    content.addSubview(&button);

    window.center();
    window.makeKeyAndOrderFront(None);
    window.orderFrontRegardless();

    TARGET.with(|slot| slot.replace(Some(target)));
    WINDOW.with(|slot| slot.replace(Some(window)));
    Ok(())
}

fn colored_text(source: &str, kind: FormatKind) -> Retained<NSMutableAttributedString> {
    let ns = NSString::from_str(source);
    let attr = NSMutableAttributedString::initWithString(NSMutableAttributedString::alloc(), &ns);
    let mut offset = 0usize;
    for (token_kind, token) in highlight::tokens(source, kind) {
        let len = token.encode_utf16().count();
        let range = NSRange {
            location: offset,
            length: len,
        };
        let color = color_for(token_kind);
        unsafe {
            attr.addAttribute_value_range(NSForegroundColorAttributeName, &color, range);
        }
        offset += len;
    }
    attr
}

fn color_for(kind: TokenKind) -> Retained<NSColor> {
    match kind {
        TokenKind::Key => NSColor::colorWithCalibratedRed_green_blue_alpha(0.49, 0.83, 0.99, 1.0),
        TokenKind::String => NSColor::colorWithCalibratedRed_green_blue_alpha(0.53, 0.94, 0.67, 1.0),
        TokenKind::Number => NSColor::colorWithCalibratedRed_green_blue_alpha(0.98, 0.75, 0.14, 1.0),
        TokenKind::Keyword => NSColor::colorWithCalibratedRed_green_blue_alpha(0.77, 0.71, 0.99, 1.0),
        TokenKind::Punct => NSColor::colorWithCalibratedRed_green_blue_alpha(0.80, 0.84, 0.88, 1.0),
        TokenKind::Text => NSColor::colorWithCalibratedRed_green_blue_alpha(0.91, 0.91, 0.93, 1.0),
    }
}
