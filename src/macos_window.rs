#![cfg(target_os = "macos")]

use std::cell::RefCell;

use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyObject, NSObject};
use objc2::{AnyThread, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSAutoresizingMaskOptions, NSBackingStoreType, NSButton, NSColor, NSFont,
    NSFontAttributeName, NSForegroundColorAttributeName, NSModalResponseOK, NSSavePanel,
    NSScrollView, NSTextView, NSVisualEffectBlendingMode, NSVisualEffectMaterial,
    NSVisualEffectState, NSVisualEffectView, NSWindow, NSWindowStyleMask, NSWindowTitleVisibility,
};
use objc2_foundation::{
    NSArray, NSMutableAttributedString, NSPoint, NSRange, NSRect, NSSize, NSString,
};

use crate::clipboard;
use crate::dataframe;
use crate::format::{self, FormatKind};
use crate::highlight::{self, TokenKind};
use crate::redact;

thread_local! {
    static WINDOW: RefCell<Option<Retained<NSWindow>>> = const { RefCell::new(None) };
    static TARGET: RefCell<Option<Retained<PreviewTarget>>> = const { RefCell::new(None) };
    static TEXT: RefCell<Option<Retained<NSTextView>>> = const { RefCell::new(None) };
    static COPY_BUTTON: RefCell<Option<Retained<NSButton>>> = const { RefCell::new(None) };
    static ORIGINAL_BUTTON: RefCell<Option<Retained<NSButton>>> = const { RefCell::new(None) };
    static FORMAT_BUTTON: RefCell<Option<Retained<NSButton>>> = const { RefCell::new(None) };
    static REDACT_BUTTON: RefCell<Option<Retained<NSButton>>> = const { RefCell::new(None) };
    static DATAFRAME_BUTTON: RefCell<Option<Retained<NSButton>>> = const { RefCell::new(None) };
    static SAVE_BUTTON: RefCell<Option<Retained<NSButton>>> = const { RefCell::new(None) };
    static SOURCE_TEXT: RefCell<String> = const { RefCell::new(String::new()) };
    static PREVIEW_TEXT: RefCell<String> = const { RefCell::new(String::new()) };
    static PREVIEW_KIND: RefCell<FormatKind> = const { RefCell::new(FormatKind::Plain) };
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "CopycraftPreviewTarget"]
    struct PreviewTarget;

    impl PreviewTarget {
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

        #[unsafe(method(originalClicked:))]
        fn original_clicked(&self, _sender: Option<&AnyObject>) {
            let body = SOURCE_TEXT.with(|src| src.borrow().clone());
            apply_preview(&body);
            style_original_button(true);
            unsafe {
                let _: () = msg_send![
                    self,
                    performSelector: sel!(resetOriginalLabel:),
                    withObject: None::<&AnyObject>,
                    afterDelay: 1.6
                ];
            }
        }

        #[unsafe(method(resetOriginalLabel:))]
        fn reset_original_label(&self, _sender: Option<&AnyObject>) {
            style_original_button(false);
        }

        #[unsafe(method(formatClicked:))]
        fn format_clicked(&self, _sender: Option<&AnyObject>) {
            let body = SOURCE_TEXT.with(|src| clipboard::formatted(&src.borrow()));
            apply_preview(&body);
            style_format_button(true);
            unsafe {
                let _: () = msg_send![
                    self,
                    performSelector: sel!(resetFormatLabel:),
                    withObject: None::<&AnyObject>,
                    afterDelay: 1.6
                ];
            }
        }

        #[unsafe(method(resetFormatLabel:))]
        fn reset_format_label(&self, _sender: Option<&AnyObject>) {
            style_format_button(false);
        }

        #[unsafe(method(redactClicked:))]
        fn redact_clicked(&self, _sender: Option<&AnyObject>) {
            let body = SOURCE_TEXT.with(|src| redact::redact(&src.borrow()));
            apply_preview(&body);
            style_redact_button(true);
            unsafe {
                let _: () = msg_send![
                    self,
                    performSelector: sel!(resetRedactLabel:),
                    withObject: None::<&AnyObject>,
                    afterDelay: 1.6
                ];
            }
        }

        #[unsafe(method(resetRedactLabel:))]
        fn reset_redact_label(&self, _sender: Option<&AnyObject>) {
            style_redact_button(false);
        }

        #[unsafe(method(dataframeClicked:))]
        fn dataframe_clicked(&self, _sender: Option<&AnyObject>) {
            let body = SOURCE_TEXT.with(|src| dataframe::try_format(&src.borrow()));
            let Some(body) = body else {
                return;
            };
            apply_preview_with_kind(&body, FormatKind::Dataframe);
            style_dataframe_button(true);
            unsafe {
                let _: () = msg_send![
                    self,
                    performSelector: sel!(resetDataframeLabel:),
                    withObject: None::<&AnyObject>,
                    afterDelay: 1.6
                ];
            }
        }

        #[unsafe(method(resetDataframeLabel:))]
        fn reset_dataframe_label(&self, _sender: Option<&AnyObject>) {
            style_dataframe_button(false);
        }

        #[unsafe(method(saveClicked:))]
        fn save_clicked(&self, _sender: Option<&AnyObject>) {
            let saved = save_preview_to_file();
            style_save_button(saved);
            if saved {
                unsafe {
                    let _: () = msg_send![
                        self,
                        performSelector: sel!(resetSaveLabel:),
                        withObject: None::<&AnyObject>,
                        afterDelay: 1.6
                    ];
                }
            }
        }

        #[unsafe(method(resetSaveLabel:))]
        fn reset_save_label(&self, _sender: Option<&AnyObject>) {
            style_save_button(false);
        }
    }
);

impl PreviewTarget {
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

fn style_title_button(button: &NSButton, label: &str, color: &NSColor) {
    let ns = NSString::from_str(label);
    let attr = NSMutableAttributedString::initWithString(NSMutableAttributedString::alloc(), &ns);
    let all = NSRange {
        location: 0,
        length: ns.length(),
    };
    unsafe {
        attr.addAttribute_value_range(NSForegroundColorAttributeName, color, all);
        attr.addAttribute_value_range(NSFontAttributeName, &NSFont::systemFontOfSize(13.0), all);
    }
    button.setAttributedTitle(&attr);
}

fn idle_button_color() -> Retained<NSColor> {
    NSColor::colorWithCalibratedRed_green_blue_alpha(0.86, 0.89, 0.93, 1.0)
}

fn copy_flash_color() -> Retained<NSColor> {
    NSColor::colorWithCalibratedRed_green_blue_alpha(0.32, 0.84, 0.54, 1.0)
}

fn format_flash_color() -> Retained<NSColor> {
    NSColor::colorWithCalibratedRed_green_blue_alpha(0.96, 0.77, 0.26, 1.0)
}

fn original_flash_color() -> Retained<NSColor> {
    NSColor::colorWithCalibratedRed_green_blue_alpha(0.35, 0.78, 0.98, 1.0)
}

fn redact_flash_color() -> Retained<NSColor> {
    NSColor::colorWithCalibratedRed_green_blue_alpha(0.80, 0.48, 0.94, 1.0)
}

fn dataframe_flash_color() -> Retained<NSColor> {
    NSColor::colorWithCalibratedRed_green_blue_alpha(0.39, 0.82, 1.0, 1.0)
}

fn save_flash_color() -> Retained<NSColor> {
    NSColor::colorWithCalibratedRed_green_blue_alpha(1.0, 0.68, 0.36, 1.0)
}
