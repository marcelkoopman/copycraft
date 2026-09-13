#![cfg(target_os = "macos")]

use std::cell::RefCell;

use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyObject, NSObject};
use objc2::{AnyThread, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSAutoresizingMaskOptions, NSBackingStoreType, NSButton, NSColor, NSFont,
    NSFontAttributeName, NSForegroundColorAttributeName, NSScrollView, NSTextView,
    NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView,
    NSWindow, NSWindowStyleMask, NSWindowTitleVisibility,
};
use objc2_foundation::{NSMutableAttributedString, NSPoint, NSRange, NSRect, NSSize, NSString};

use crate::clipboard;
use crate::format::FormatKind;
use crate::highlight::{self, TokenKind};

thread_local! {
    static WINDOW: RefCell<Option<Retained<NSWindow>>> = const { RefCell::new(None) };
    static TARGET: RefCell<Option<Retained<CopyTarget>>> = const { RefCell::new(None) };
    static TEXT: RefCell<Option<Retained<NSTextView>>> = const { RefCell::new(None) };
    static BUTTON: RefCell<Option<Retained<NSButton>>> = const { RefCell::new(None) };
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
            style_copy_button(true);
            unsafe {
                let _: () = msg_send![
                    self,
                    performSelector: sel!(resetCopyLabel:),
                    withObject: None::<&AnyObject>,
                    afterDelay: 1.6
                ];
            }
        }

        #[unsafe(method(resetCopyLabel:))]
        fn reset_copy_label(&self, _sender: Option<&AnyObject>) {
            style_copy_button(false);
        }
    }
);

impl CopyTarget {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this: Allocated<Self> = Self::alloc(mtm);
        unsafe { msg_send![this, init] }
    }
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
        if let Some(font) = NSFont::fontWithName_size(&NSString::from_str(name), 14.0) {
            return font;
        }
    }
    NSFont::monospacedSystemFontOfSize_weight(14.0, 0.0)
}

fn style_copy_button(copied: bool) {
    BUTTON.with(|slot| {
        let borrowed = slot.borrow();
        let Some(button) = borrowed.as_ref() else {
            return;
        };
        let label = if copied { "Copied  \u{2713}" } else { "Copy" };
        let ns = NSString::from_str(label);
        let attr =
            NSMutableAttributedString::initWithString(NSMutableAttributedString::alloc(), &ns);
        let all = NSRange {
            location: 0,
            length: ns.length(),
        };
        let color = if copied {
            NSColor::colorWithCalibratedRed_green_blue_alpha(0.32, 0.84, 0.54, 1.0)
        } else {
            NSColor::colorWithCalibratedRed_green_blue_alpha(0.86, 0.89, 0.93, 1.0)
        };
        unsafe {
            attr.addAttribute_value_range(NSForegroundColorAttributeName, &color, all);
            attr.addAttribute_value_range(
                NSFontAttributeName,
                &NSFont::systemFontOfSize(13.0),
                all,
            );
        }
        button.setAttributedTitle(&attr);
    });
}

fn set_body(text: &NSTextView, body: &str, kind: FormatKind) {
    if let Some(storage) = unsafe { text.textStorage() } {
        storage.setAttributedString(&colored_text(body, kind));
    } else {
        text.setString(&NSString::from_str(body));
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

    let reused = WINDOW.with(|slot| slot.borrow().is_some());
    if reused {
        WINDOW.with(|slot| {
            if let Some(window) = slot.borrow().as_ref() {
                window.setTitle(&NSString::from_str(title));
                window.setAlphaValue(0.86);
                window.makeKeyAndOrderFront(None);
                window.orderFrontRegardless();
            }
        });
        TEXT.with(|slot| {
            if let Some(text) = slot.borrow().as_ref() {
                set_body(text, &body, kind);
            }
        });
        style_copy_button(false);
        return Ok(());
    }

    let width = 720.0;
    let height = 480.0;
    let titlebar = 28.0;
    let frame = NSRect::new(NSPoint::new(260.0, 200.0), NSSize::new(width, height));
    let style = NSWindowStyleMask::Titled
        | NSWindowStyleMask::Closable
        | NSWindowStyleMask::Resizable
        | NSWindowStyleMask::Miniaturizable
        | NSWindowStyleMask::FullSizeContentView;
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
    window.setTitle(&NSString::from_str(title));
    window.setTitlebarAppearsTransparent(true);
    window.setTitleVisibility(NSWindowTitleVisibility::Visible);
    window.setOpaque(false);
    window.setHasShadow(true);
    window.setBackgroundColor(Some(&NSColor::clearColor()));
    window.setAlphaValue(0.86);

    let bounds = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, height));
    let frosted = NSVisualEffectView::initWithFrame(NSVisualEffectView::alloc(mtm), bounds);
    frosted.setMaterial(NSVisualEffectMaterial::UnderWindowBackground);
    frosted.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
    frosted.setState(NSVisualEffectState::Active);
    frosted.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );

    let button = NSButton::initWithFrame(
        NSButton::alloc(mtm),
        NSRect::new(
            NSPoint::new(width - 132.0, height - titlebar + 3.0),
            NSSize::new(116.0, 22.0),
        ),
    );
    button.setBordered(true);
    button.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewMinXMargin | NSAutoresizingMaskOptions::ViewMinYMargin,
    );
    let target = CopyTarget::new(mtm);
    unsafe {
        button.setTarget(Some(&target));
        button.setAction(Some(sel!(copyClicked:)));
    }

    let scroll = NSScrollView::initWithFrame(
        NSScrollView::alloc(mtm),
        NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(width, height - titlebar),
        ),
    );
    scroll.setHasVerticalScroller(true);
    scroll.setHasHorizontalScroller(false);
    scroll.setDrawsBackground(false);
    scroll.setBackgroundColor(&NSColor::clearColor());
    scroll.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );

    let text = NSTextView::initWithFrame(
        NSTextView::alloc(mtm),
        NSRect::new(
            NSPoint::new(8.0, 0.0),
            NSSize::new(width - 16.0, height - titlebar),
        ),
    );
    text.setEditable(false);
    text.setSelectable(true);
    text.setDrawsBackground(false);
    text.setBackgroundColor(&NSColor::clearColor());
    text.setTextContainerInset(NSSize::new(10.0, 12.0));
    text.setFont(Some(&editor_font()));
    set_body(&text, &body, kind);
    scroll.setDocumentView(Some(&text));

    frosted.addSubview(&scroll);
    frosted.addSubview(&button);
    window.setContentView(Some(&frosted));

    window.center();
    window.makeKeyAndOrderFront(None);
    window.orderFrontRegardless();

    TARGET.with(|slot| slot.replace(Some(target)));
    TEXT.with(|slot| slot.replace(Some(text)));
    BUTTON.with(|slot| slot.replace(Some(button)));
    WINDOW.with(|slot| slot.replace(Some(window)));
    style_copy_button(false);
    Ok(())
}

fn gutter(line: usize, width: usize) -> String {
    format!("{line:>width$}  \u{2502} ")
}

fn colored_text(source: &str, kind: FormatKind) -> Retained<NSMutableAttributedString> {
    let line_count = source.lines().count().max(1);
    let width = line_count.to_string().len();
    let mut display = String::new();
    let mut spans: Vec<(TokenKind, usize)> = Vec::new();
    let mut line = 1usize;

    let start = gutter(line, width);
    display.push_str(&start);
    spans.push((TokenKind::Comment, start.encode_utf16().count()));

    for (token_kind, token) in highlight::tokens(source, kind) {
        let parts: Vec<&str> = token.split('\n').collect();
        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                display.push('\n');
                spans.push((TokenKind::Text, 1));
                line += 1;
                let g = gutter(line, width);
                display.push_str(&g);
                spans.push((TokenKind::Comment, g.encode_utf16().count()));
            }
            if !part.is_empty() {
                display.push_str(part);
                spans.push((token_kind, part.encode_utf16().count()));
            }
        }
    }

    let ns = NSString::from_str(&display);
    let attr = NSMutableAttributedString::initWithString(NSMutableAttributedString::alloc(), &ns);
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
    for (token_kind, len) in spans {
        let range = NSRange {
            location: offset,
            length: len,
        };
        unsafe {
            attr.addAttribute_value_range(
                NSForegroundColorAttributeName,
                &color_for(token_kind),
                range,
            );
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
        TokenKind::String => {
            NSColor::colorWithCalibratedRed_green_blue_alpha(0.62, 0.80, 0.42, 1.0)
        }
        TokenKind::Number => {
            NSColor::colorWithCalibratedRed_green_blue_alpha(0.86, 0.61, 0.36, 1.0)
        }
        TokenKind::Keyword => {
            NSColor::colorWithCalibratedRed_green_blue_alpha(0.78, 0.63, 0.97, 1.0)
        }
        TokenKind::Type => NSColor::colorWithCalibratedRed_green_blue_alpha(0.45, 0.80, 0.93, 1.0),
        TokenKind::Macro => NSColor::colorWithCalibratedRed_green_blue_alpha(0.48, 0.69, 0.97, 1.0),
        TokenKind::Comment => {
            NSColor::colorWithCalibratedRed_green_blue_alpha(0.45, 0.48, 0.55, 1.0)
        }
        TokenKind::Punct => NSColor::colorWithCalibratedRed_green_blue_alpha(0.62, 0.65, 0.72, 1.0),
        TokenKind::Text => NSColor::colorWithCalibratedRed_green_blue_alpha(0.78, 0.80, 0.86, 1.0),
    }
}
