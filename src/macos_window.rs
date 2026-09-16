#![cfg(target_os = "macos")]

use std::cell::RefCell;

use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyObject, NSObject};
use objc2::{AnyThread, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAnimatablePropertyContainer, NSAnimationContext, NSApplication, NSAutoresizingMaskOptions,
    NSBackingStoreType, NSButton, NSColor, NSFont, NSFontAttributeName,
    NSForegroundColorAttributeName, NSModalResponseOK, NSSavePanel, NSScrollView, NSTextView,
    NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView,
    NSWindow, NSWindowStyleMask, NSWindowTitleVisibility,
};
use objc2_foundation::{
    NSArray, NSMutableAttributedString, NSPoint, NSRange, NSRect, NSSize, NSString,
};

use crate::clipboard;
use crate::dataframe;
use crate::format::{self, FormatKind};
use crate::macos_preview_text::{configure_scrolling_text, editor_font, set_body};
use crate::redact;
use crate::toolbar_visibility;

include!("macos_window_state.rs");
include!("macos_window_show.rs");
