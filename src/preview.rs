use std::io::Write;
use std::process::{Command, Stdio};

use crate::format::FormatKind;

pub fn show(formatted: &str, kind: FormatKind) -> Result<(), String> {
    if !cfg!(target_os = "macos") {
        return Err("native preview is macOS-only".into());
    }
    let script = jxa_script(formatted, kind);
    let mut child = Command::new("osascript")
        .args(["-l", "JavaScript"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    child
        .stdin
        .as_mut()
        .ok_or_else(|| "osascript stdin".to_string())?
        .write_all(script.as_bytes())
        .map_err(|e| e.to_string())?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

pub fn jxa_script(source: &str, kind: FormatKind) -> String {
    let title = js_string(kind.preview_heading());
    let raw = js_string(source);
    let html = js_string(&format!(
        "<pre style='font:13px/1.5 Menlo,monospace;color:#e8e8ed;background:#1e1e22'>{}</pre>",
        highlight(source, kind)
    ));
    let mut script = String::new();
    script.push_str("ObjC.import('Cocoa');\n");
    script.push_str(&format!("const raw = {raw};\n"));
    script.push_str(&format!("const html = {html};\n"));
    script.push_str(&format!("const title = {title};\n"));
    script.push_str(
        r#"
const win = $.NSWindow.alloc.initWithContentRectStyleMaskBackingDefer(
  $.NSMakeRect(200, 160, 780, 560),
  $.NSWindowStyleMaskTitled | $.NSWindowStyleMaskClosable | $.NSWindowStyleMaskResizable | $.NSWindowStyleMaskMiniaturizable,
  $.NSBackingStoreBuffered,
  false
);
win.title = title;
win.backgroundColor = $.NSColor.colorWithCalibratedRedGreenBlueAlpha(0.12, 0.12, 0.13, 1);

const button = $.NSButton.alloc.initWithFrame($.NSMakeRect(16, 516, 168, 28));
button.title = 'Copy to clipboard';
button.bezelStyle = $.NSBezelStyleRounded;
button.setCOSJSTargetFunction(function (_sender) {
  const pb = $.NSPasteboard.generalPasteboard;
  pb.clearContents();
  pb.setStringForType(raw, $.NSPasteboardTypeString);
});

const scroll = $.NSScrollView.alloc.initWithFrame($.NSMakeRect(0, 0, 780, 508));
scroll.hasVerticalScroller = true;
scroll.hasHorizontalScroller = true;
scroll.autoresizingMask = $.NSViewWidthSizable | $.NSViewHeightSizable;
const text = $.NSTextView.alloc.initWithFrame($.NSMakeRect(0, 0, 760, 508));
text.editable = false;
text.drawsBackground = true;
text.backgroundColor = $.NSColor.colorWithCalibratedRedGreenBlueAlpha(0.12, 0.12, 0.13, 1);
const data = $.NSString.alloc.initWithUTF8String(html).dataUsingEncoding($.NSUTF8StringEncoding);
const attr = $.NSAttributedString.alloc.initWithHTMLDocumentAttributes(data, null);
text.textStorage.setAttributedString(attr);
scroll.documentView = text;

win.contentView.addSubview(scroll);
win.contentView.addSubview(button);
win.makeKeyAndOrderFront(null);
$.NSApplication.sharedApplication.activateIgnoringOtherApps(true);
while (win.isVisible) {
  $.NSRunLoop.currentRunLoop.runUntilDate($.NSDate.dateWithTimeIntervalSinceNow(0.15));
}
"#,
    );
    script
}

fn highlight(source: &str, kind: FormatKind) -> String {
    match kind {
        FormatKind::Json => highlight_json(source),
        FormatKind::Rust | FormatKind::Java => highlight_code(source, kind),
        _ => escape_html(source),
    }
}

fn highlight_json(source: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = source.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '"' {
            let (token, next) = take_string(&chars, i);
            let after = skip_ws(&chars, next);
            let hex = if after < chars.len() && chars[after] == ':' {
                "#7dd3fc"
            } else {
                "#86efac"
            };
            out.push_str(&color(hex, &token));
            i = next;
            continue;
        }
        if ch.is_ascii_digit()
            || (ch == '-' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit())
        {
            let (token, next) = take_while(&chars, i, |c| {
                c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E')
            });
            out.push_str(&color("#fbbf24", &token));
            i = next;
            continue;
        }
        if starts_with(&chars, i, "true")
            || starts_with(&chars, i, "false")
            || starts_with(&chars, i, "null")
        {
            let word = if starts_with(&chars, i, "true") {
                "true"
            } else if starts_with(&chars, i, "false") {
                "false"
            } else {
                "null"
            };
            out.push_str(&color("#c4b5fd", word));
            i += word.chars().count();
            continue;
        }
        if matches!(ch, '{' | '}' | '[' | ']' | ':' | ',') {
            out.push_str(&color("#cbd5e1", &ch.to_string()));
            i += 1;
            continue;
        }
        out.push_str(&escape_html(&ch.to_string()));
        i += 1;
    }
    out
}

fn highlight_code(source: &str, kind: FormatKind) -> String {
    let keywords: &[&str] = match kind {
        FormatKind::Rust => &[
            "fn", "let", "mut", "pub", "impl", "struct", "enum", "match", "if", "else", "use",
            "mod", "return", "async", "await", "self", "Self", "crate", "const", "static",
        ],
        _ => &[
            "public", "private", "protected", "class", "static", "void", "int", "long",
            "boolean", "return", "if", "else", "new", "package", "import", "final", "this",
        ],
    };
    let mut out = String::new();
    let chars: Vec<char> = source.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '"' {
            let (token, next) = take_string(&chars, i);
            out.push_str(&color("#86efac", &token));
            i = next;
            continue;
        }
        if ch.is_ascii_alphabetic() || ch == '_' {
            let (token, next) = take_while(&chars, i, |c| c.is_ascii_alphanumeric() || c == '_');
            if keywords.contains(&token.as_str()) {
                out.push_str(&color("#c4b5fd", &token));
            } else {
                out.push_str(&escape_html(&token));
            }
            i = next;
            continue;
        }
        if ch.is_ascii_digit() {
            let (token, next) = take_while(&chars, i, |c| c.is_ascii_digit() || c == '.');
            out.push_str(&color("#fbbf24", &token));
            i = next;
            continue;
        }
        out.push_str(&escape_html(&ch.to_string()));
        i += 1;
    }
    out
}

fn take_string(chars: &[char], start: usize) -> (String, usize) {
    let mut i = start + 1;
    let mut token = String::from("\"");
    while i < chars.len() {
        let ch = chars[i];
        token.push(ch);
        if ch == '\\' && i + 1 < chars.len() {
            token.push(chars[i + 1]);
            i += 2;
            continue;
        }
        if ch == '"' {
            return (token, i + 1);
        }
        i += 1;
    }
    (token, i)
}

fn take_while(chars: &[char], start: usize, pred: impl Fn(char) -> bool) -> (String, usize) {
    let mut i = start;
    let mut token = String::new();
    while i < chars.len() && pred(chars[i]) {
        token.push(chars[i]);
        i += 1;
    }
    (token, i)
}

fn skip_ws(chars: &[char], mut i: usize) -> usize {
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    i
}

fn starts_with(chars: &[char], i: usize, word: &str) -> bool {
    let w: Vec<char> = word.chars().collect();
    if i + w.len() > chars.len() {
        return false;
    }
    chars[i..i + w.len()] == w[..]
        && (i + w.len() == chars.len() || !chars[i + w.len()].is_ascii_alphanumeric())
}

fn color(hex: &str, text: &str) -> String {
    format!("<span style='color:{hex}'>{}</span>", escape_html(text))
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&")
        .replace('<', "<")
        .replace('>', ">")
}

fn js_string(text: &str) -> String {
    format!(
        "\"{}\"",
        text.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
    )
}

#[cfg(test)]
mod tests {
    use super::{escape_html, highlight_json, jxa_script};
    use crate::format::FormatKind;

    #[test]
    fn escapes_html() {
        assert_eq!(escape_html("<tag>"), "<tag>");
    }

    #[test]
    fn colors_json_keys() {
        let html = highlight_json("{\"name\":1}");
        assert!(html.contains("#7dd3fc"));
        assert!(html.contains("#fbbf24"));
    }

    #[test]
    fn script_has_native_copy_button() {
        let script = jxa_script("{\"a\":1}", FormatKind::Json);
        assert!(script.contains("Copy to clipboard"));
        assert!(script.contains("NSWindow"));
        assert!(!script.contains("navigator.clipboard"));
    }
}
