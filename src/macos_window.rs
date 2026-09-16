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
