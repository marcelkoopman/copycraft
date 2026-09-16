#![cfg(target_os = "macos")]

use objc2::rc::Retained;
use objc2_app_kit::{
    NSColor, NSFont, NSFontAttributeName, NSForegroundColorAttributeName, NSTextView,
};
use objc2_foundation::{NSMutableAttributedString, NSRange, NSSize, NSString};

use crate::format::FormatKind;
use crate::highlight::{self, TokenKind};

pub(crate) fn editor_font() -> Retained<NSFont> {
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

pub(crate) fn configure_scrolling_text(text: &NSTextView) {
    const LARGE: f64 = 10_000_000.0;
    text.setHorizontallyResizable(true);
    text.setVerticallyResizable(true);
    text.setMaxSize(NSSize::new(LARGE, LARGE));
    if let Some(container) = unsafe { text.textContainer() } {
        container.setWidthTracksTextView(false);
        container.setHeightTracksTextView(false);
        container.setContainerSize(NSSize::new(LARGE, LARGE));
    }
}

pub(crate) fn set_body(text: &NSTextView, body: &str, kind: FormatKind) {
    if let Some(storage) = unsafe { text.textStorage() } {
        storage.setAttributedString(&colored_text(body, kind));
    } else {
        text.setString(&NSString::from_str(body));
    }
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
