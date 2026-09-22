#![cfg(target_os = "macos")]

use std::cell::{Cell, RefCell};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use dispatch2::DispatchQueue;
use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyObject, NSObject};
use objc2::{AnyThread, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAnimatablePropertyContainer, NSAnimationContext, NSApplication, NSAutoresizingMaskOptions,
    NSBackingStoreType, NSButton, NSColor, NSFont, NSFontAttributeName,
    NSForegroundColorAttributeName, NSImageAlignment, NSImageScaling, NSImageView,
    NSModalResponseOK, NSSavePanel, NSScrollView, NSTextView, NSVisualEffectBlendingMode,
    NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView, NSWindow, NSWindowStyleMask,
    NSWindowTitleVisibility,
};
use objc2_foundation::{
    NSArray, NSMutableAttributedString, NSPoint, NSRange, NSRect, NSSize, NSString,
};

use crate::clipboard::{self, ClipboardImage};
use crate::compress;
use crate::convert;
use crate::dataframe;
use crate::decode;
use crate::format::{self, FormatKind};
use crate::image_ops;
use crate::macos_preview_image::{nsimage_from_bytes, nsimage_from_clipboard};
use crate::macos_preview_text::{configure_scrolling_text, editor_font, set_body};
use crate::macos_vision;
use crate::preview::PreviewAction;
use crate::redact;
use crate::toolbar_visibility;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    Original,
    Format,
    Convert,
    Decode,
    Compress,
    Redact,
    Dataframe,
    Info,
    Ocr,
    Qr,
}

include!("macos_window_state.rs");
include!("macos_window_show.rs");
