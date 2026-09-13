#![cfg(target_os = "macos")]

use std::cell::RefCell;

use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyObject, NSObject};
use objc2::{define_class, msg_send, sel, AnyThread, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSAutoresizingMaskOptions, NSBackingStoreType, NSButton, NSColor, NSControl,
    NSFont, NSFontAttributeName, NSForegroundColorAttributeName, NSScrollView, NSTextView,
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

fn editor_bg() -> Retained<NSColor> {
    NSColor::colorWithCalibratedRed_green_blue_alpha(0.118, 0.122, 0.149, 1.0)
}

fn editor_font() -> Retained<NSFont> {
    const NAMES: [&str; 5] = [
        "Zed Mono",
        "Zed Mono Regular",
        "JetBrains Mono",
        "SF Mono",
        "Menlo",
    ];
    for name in NAMES {
        if let Some(font) = NSFont::fontWithName_size(&NSString::from_str(name), 15.0) {
            return font;
        }
    }
    NSFont::monospacedSystemFontOfSize_weight(15.0, 0.0)
}

pub fn show(formatted: &str, kind: FormatKind) -> Result<(), String> {
    let mtm = MainThreadMarker::new().ok_or("preview must run on the main thread")?;
    let title = kind.preview_heading();
    let body = formatted.to_string();
    PREVIEW_TEXT.with(|slot| slot.replace(body.clone()));

    let app = NSApplication::sharedApplication(mtm);
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);

    let frame = NSRect::new(NSPoint::new(240.0, 180.0), NSSize::new(820.0, 560.0));
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
    window.setBackgroundColor(Some(&editor_bg()));

    let content = window
        .contentView()
        .ok_or("window has no content view")?;

    let button = NSButton::initWithFrame(
        NSButton::alloc(mtm),
        NSRect::new(NSPoint::new(16.0, 520.0), NSSize::new(180.0, 28.0)),
    );
    button.setTitle(&NSString::from_str("Copy to clipboard"));
    let target = CopyTarget::new(mtm);
    unsafe {
        button.setTarget(Some(&target));
        button.setAction(Some(sel!(copyClicked:)));
    }

    let scroll = NSScrollView::initWithFrame(
        NSScrollView::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(820.0, 512.0)),
    );
    scroll.setHasVerticalScroller(true);
    scroll.setHasHorizontalScroller(true);
    scroll.setDrawsBackground(true);
    scroll.setBackgroundColor(&editor_bg());
    scroll.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );

    let text = NSTextView::initWithFrame(
        NSTextView::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(820.0, 512.0)),
    );
    text.setEditable(false);
    text.setSelectable(true);
    text.setDrawsBackground(true);
    text.setBackgroundColor(&editor_bg());
    text.setFont(Some(&editor_font()));
    if let Some(storage) = unsafe { text.textStorage() } {
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
    let attr =
        NSMutableAttributedString::initWithString(NSMutableAttributedString::alloc(), &ns);
    let all = NSRange {
        location: 0,
        length: ns.length(),
    };
    unsafe {
        attr.addAttribute_value_range(NSFontAttributeName, &editor_font(), all);
        attr.addAttribute_value_range(
            NSForegroundColorAttributeName,
            &color_for(TokenKind::Text),
            all,
        );
    }
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
        TokenKind::Key | TokenKind::Function => {
            NSColor::colorWithCalibratedRed_green_blue_alpha(0.48, 0.69, 0.97, 1.0)
        }
        TokenKind::String => NSColor::colorWithCalibratedRed_green_blue_alpha(0.62, 0.80, 0.42, 1.0),
        TokenKind::Number => NSColor::colorWithCalibratedRed_green_blue_alpha(0.86, 0.61, 0.36, 1.0),
        TokenKind::Keyword => NSColor::colorWithCalibratedRed_green_blue_alpha(0.78, 0.63, 0.97, 1.0),
        TokenKind::Type => NSColor::colorWithCalibratedRed_green_blue_alpha(0.45, 0.80, 0.93, 1.0),
        TokenKind::Macro => NSColor::colorWithCalibratedRed_green_blue_alpha(0.48, 0.69, 0.97, 1.0),
        TokenKind::Comment => NSColor::colorWithCalibratedRed_green_blue_alpha(0.45, 0.48, 0.55, 1.0),
        TokenKind::Punct => NSColor::colorWithCalibratedRed_green_blue_alpha(0.62, 0.65, 0.72, 1.0),
        TokenKind::Text => NSColor::colorWithCalibratedRed_green_blue_alpha(0.78, 0.80, 0.86, 1.0),
    }
}
