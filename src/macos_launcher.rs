#![cfg(target_os = "macos")]

use std::cell::{Cell, RefCell};

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, Sel};
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSBackingStoreType, NSBox, NSBoxType, NSButton, NSColor, NSControl, NSControlStateValueOff,
    NSControlStateValueOn, NSEvent, NSEventModifierFlags, NSFloatingWindowLevel, NSFocusRingType,
    NSFont, NSImage, NSImageAlignment, NSImageScaling, NSImageView, NSLineBreakMode, NSMenu,
    NSMenuItem, NSScreen, NSTextAlignment, NSTextField, NSTextView, NSTitlePosition, NSView,
    NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView,
    NSWindow, NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_foundation::{NSNotification, NSPoint, NSRect, NSSize, NSString};

use crate::appearance::Theme;
use crate::commands::{self, ChipFrame, Command, CommandId, LaunchData};
use crate::launcher::{self, UserEvent};
use crate::macos_preview_image::{nsimage_from_bytes, nsimage_from_clipboard};

const WIDTH: f64 = 440.0;
const PAD: f64 = 14.0;
const HEADER_H: f64 = 32.0;
const PREVIEW_H: f64 = 132.0;
const META_H: f64 = 22.0;
const SEARCH_H: f64 = 36.0;
const GAP: f64 = 8.0;
const HOTKEY: &str = crate::hotkey::LABEL;

thread_local! {
    static OPEN: Cell<bool> = const { Cell::new(false) };
    static SUPPRESS_RESIGN: Cell<bool> = const { Cell::new(false) };
    static SEARCHING: Cell<bool> = const { Cell::new(false) };
    static SELECTION: Cell<usize> = const { Cell::new(0) };
    static THEME: Cell<Theme> = const { Cell::new(Theme::System) };
    static SHOWS_IMAGE: Cell<bool> = const { Cell::new(false) };
    static THUMB_TOKEN: Cell<isize> = const { Cell::new(-1) };
    static ACTIONS: RefCell<Vec<Command>> = const { RefCell::new(Vec::new()) };
    static POOL: RefCell<Vec<Command>> = const { RefCell::new(Vec::new()) };
    static OVERFLOW: RefCell<Vec<Command>> = const { RefCell::new(Vec::new()) };
    static SHOWN: RefCell<Vec<Command>> = const { RefCell::new(Vec::new()) };
    static FRAMES: RefCell<Vec<ChipFrame>> = const { RefCell::new(Vec::new()) };
    static CARD_TITLE: RefCell<String> = const { RefCell::new(String::new()) };
    static CARD_META: RefCell<String> = const { RefCell::new(String::new()) };
    static CARD_EXCERPT: RefCell<String> = const { RefCell::new(String::new()) };
    static CARD_PLACEHOLDER: RefCell<String> = const { RefCell::new(String::new()) };
    static LINK_PAGE: RefCell<Option<String>> = const { RefCell::new(None) };
    static LINK_THUMB: RefCell<Option<String>> = const { RefCell::new(None) };
    static LINK_IMAGE: RefCell<Option<(String, Retained<NSImage>)>> = const { RefCell::new(None) };
    static LINK_CAPTION: RefCell<Option<(String, String)>> = const { RefCell::new(None) };
    static LINK_IN_FLIGHT: RefCell<Option<String>> = const { RefCell::new(None) };
    static LINK_FAILED: RefCell<Option<String>> = const { RefCell::new(None) };
    static LINK_GEN: Cell<u64> = const { Cell::new(0) };
    static WINDOW: RefCell<Option<Retained<LauncherWindow>>> = const { RefCell::new(None) };
    static FIELD: RefCell<Option<Retained<NSTextField>>> = const { RefCell::new(None) };
    static HEADER: RefCell<Option<Retained<NSTextField>>> = const { RefCell::new(None) };
    static META: RefCell<Option<Retained<NSTextField>>> = const { RefCell::new(None) };
    static WELL: RefCell<Option<Retained<NSBox>>> = const { RefCell::new(None) };
    static PREVIEW_TEXT: RefCell<Option<Retained<NSTextView>>> = const { RefCell::new(None) };
    static PREVIEW_IMAGE: RefCell<Option<Retained<NSImageView>>> = const { RefCell::new(None) };
    static PILLS: RefCell<Option<Retained<NSView>>> = const { RefCell::new(None) };
    static MORE: RefCell<Option<Retained<NSButton>>> = const { RefCell::new(None) };
    static CLOSE: RefCell<Option<Retained<NSButton>>> = const { RefCell::new(None) };
    static DELEGATE: RefCell<Option<Retained<LauncherDelegate>>> = const { RefCell::new(None) };
}

define_class!(
    #[unsafe(super(NSWindow))]
    #[thread_kind = MainThreadOnly]
    #[name = "CopycraftLauncherWindow"]
    struct LauncherWindow;

    impl LauncherWindow {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key_window(&self) -> bool {
            true
        }

        #[unsafe(method(canBecomeMainWindow))]
        fn can_become_main_window(&self) -> bool {
            true
        }

        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            on_key(event);
        }
    }
);

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "CopycraftLauncherDelegate"]
    struct LauncherDelegate;

    impl LauncherDelegate {
        #[unsafe(method(windowDidResignKey:))]
        fn window_did_resign_key(&self, _note: &NSNotification) {
            if SUPPRESS_RESIGN.with(Cell::get) {
                return;
            }
            hide();
        }

        #[unsafe(method(controlTextDidChange:))]
        fn control_text_did_change(&self, _note: &NSNotification) {
            let query = current_query();
            if query == "/" {
                set_query("");
                SEARCHING.set(true);
            } else if !query.is_empty() {
                SEARCHING.set(true);
            }
            SELECTION.set(0);
            layout(false);
        }

        #[unsafe(method(control:textView:doCommandBySelector:))]
        fn do_command(
            &self,
            _control: &NSControl,
            _text_view: &NSTextView,
            command: Sel,
        ) -> bool {
            if command == sel!(moveDown:) {
                nudge(0, 1);
                true
            } else if command == sel!(moveUp:) {
                nudge(0, -1);
                true
            } else if command == sel!(moveLeft:) {
                nudge(-1, 0);
                true
            } else if command == sel!(moveRight:) {
                nudge(1, 0);
                true
            } else if command == sel!(insertNewline:) {
                activate_selected();
                true
            } else if command == sel!(cancelOperation:)
                || (command == sel!(deleteBackward:) && current_query().is_empty())
            {
                close_search();
                true
            } else {
                false
            }
        }

        #[unsafe(method(chipClicked:))]
        fn chip_clicked(&self, sender: Option<&NSButton>) {
            let Some(button) = sender else {
                return;
            };
            let index = button.tag();
            if index < 0 {
                return;
            }
            SELECTION.set(index as usize);
            activate_selected();
        }

        #[unsafe(method(moreClicked:))]
        fn more_clicked(&self, _sender: Option<&NSButton>) {
            pop_overflow();
        }

        #[unsafe(method(closeClicked:))]
        fn close_clicked(&self, _sender: Option<&NSButton>) {
            hide();
        }

        #[unsafe(method(overflowClicked:))]
        fn overflow_clicked(&self, sender: Option<&NSMenuItem>) {
            let Some(item) = sender else {
                return;
            };
            let index = item.tag();
            if index < 0 {
                return;
            }
            activate_overflow(index as usize);
        }
    }
);

impl LauncherDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm);
        unsafe { msg_send![this, init] }
    }
}

pub fn is_open() -> bool {
    OPEN.with(Cell::get)
}

pub fn summon(data: LaunchData) {
    if is_open() {
        hide();
        return;
    }
    reveal(data);
}

pub fn reveal(data: LaunchData) {
    let fresh = !is_open();
    present(data, fresh);
}

pub fn sync(data: LaunchData) {
    if !is_open() {
        return;
    }
    store(data);
    layout(false);
}

fn present(data: LaunchData, fresh: bool) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    activate_app(mtm);
    ensure_window(mtm);
    if fresh {
        set_query("");
        SEARCHING.set(false);
        SELECTION.set(0);
    }
    store(data);
    OPEN.set(true);
    layout(fresh);
    WINDOW.with(|slot| {
        if let Some(window) = slot.borrow().as_ref() {
            window.makeKeyAndOrderFront(None);
            window.orderFrontRegardless();
        }
    });
    focus_card();
}

fn store(data: LaunchData) {
    let card = commands::work_card(&data);
    CARD_TITLE.with(|slot| slot.replace(card.title));
    CARD_META.with(|slot| slot.replace(card.meta));
    CARD_EXCERPT.with(|slot| slot.replace(card.excerpt));
    CARD_PLACEHOLDER.with(|slot| slot.replace(card.placeholder));
    LINK_PAGE.with(|slot| slot.replace(card.link_page));
    LINK_THUMB.with(|slot| slot.replace(card.link_thumb));
    SHOWS_IMAGE.set(card.shows_image);
    THEME.set(data.theme);
    ACTIONS.with(|slot| slot.replace(commands::chips(&data)));
    POOL.with(|slot| slot.replace(commands::search_pool(&data)));
    OVERFLOW.with(|slot| slot.replace(commands::overflow(&data)));
}

fn hide() {
    if !is_open() && !window_is_visible() {
        return;
    }
    SUPPRESS_RESIGN.with(|flag| flag.set(true));
    WINDOW.with(|slot| {
        if let Some(window) = slot.borrow().as_ref() {
            window.orderOut(None);
        }
    });
    SUPPRESS_RESIGN.with(|flag| flag.set(false));
    OPEN.set(false);
}

fn window_is_visible() -> bool {
    WINDOW.with(|slot| {
        slot.borrow()
            .as_ref()
            .is_some_and(|window| window.isVisible())
    })
}

fn activate_app(mtm: MainThreadMarker) {
    use objc2_app_kit::NSApplication;
    let app = NSApplication::sharedApplication(mtm);
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);
}

fn ensure_window(mtm: MainThreadMarker) {
    if WINDOW.with(|slot| slot.borrow().is_some()) {
        return;
    }
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WIDTH, 280.0));
    let style = NSWindowStyleMask::Borderless;
    let allocated = LauncherWindow::alloc(mtm);
    let window: Retained<LauncherWindow> = unsafe {
        msg_send![
            allocated,
            initWithContentRect: frame,
            styleMask: style,
            backing: NSBackingStoreType::Buffered,
            defer: false,
        ]
    };
    unsafe { window.setReleasedWhenClosed(false) };
    window.setOpaque(false);
    window.setHasShadow(true);
    window.setBackgroundColor(Some(&NSColor::clearColor()));
    window.setLevel(NSFloatingWindowLevel);
    window.setMovableByWindowBackground(true);
    window.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::Transient
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::IgnoresCycle,
    );

    let delegate = LauncherDelegate::new(mtm);
    unsafe {
        let _: () = msg_send![&*window, setDelegate: &*delegate];
    }
    DELEGATE.with(|slot| slot.replace(Some(delegate)));

    let content = window.contentView().expect("content view");
    content.setWantsLayer(true);
    round_view(&content, 16.0);

    let frosted =
        NSVisualEffectView::initWithFrame(NSVisualEffectView::alloc(mtm), content.bounds());
    frosted.setMaterial(NSVisualEffectMaterial::Popover);
    frosted.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
    frosted.setEmphasized(true);
    frosted.setState(NSVisualEffectState::Active);
    frosted.setAutoresizingMask(
        objc2_app_kit::NSAutoresizingMaskOptions::ViewWidthSizable
            | objc2_app_kit::NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    round_view(&frosted, 16.0);
    content.addSubview(&frosted);

    let header = static_label(mtm, 13.0, &NSColor::labelColor());
    let meta = static_label(mtm, 12.0, &NSColor::secondaryLabelColor());
    let field = search_field(mtm);
    let well = filled_box(mtm, 10.0, &NSColor::controlBackgroundColor());
    let preview_text = payload_view(mtm);
    let preview_image = image_view(mtm);
    let pills = NSView::initWithFrame(NSView::alloc(mtm), NSRect::ZERO);
    let more = icon_button(mtm, "⋯", sel!(moreClicked:));
    let close = icon_button(mtm, "✕", sel!(closeClicked:));

    content.addSubview(&header);
    content.addSubview(&more);
    content.addSubview(&close);
    content.addSubview(&well);
    content.addSubview(&preview_text);
    content.addSubview(&preview_image);
    content.addSubview(&meta);
    content.addSubview(&field);
    content.addSubview(&pills);

    HEADER.with(|slot| slot.replace(Some(header)));
    META.with(|slot| slot.replace(Some(meta)));
    FIELD.with(|slot| slot.replace(Some(field)));
    WELL.with(|slot| slot.replace(Some(well)));
    PREVIEW_TEXT.with(|slot| slot.replace(Some(preview_text)));
    PREVIEW_IMAGE.with(|slot| slot.replace(Some(preview_image)));
    PILLS.with(|slot| slot.replace(Some(pills)));
    MORE.with(|slot| slot.replace(Some(more)));
    CLOSE.with(|slot| slot.replace(Some(close)));
    WINDOW.with(|slot| slot.replace(Some(window)));
}

fn layout(fresh_place: bool) {
    let searching = SEARCHING.with(Cell::get);
    let query = if searching {
        current_query()
    } else {
        String::new()
    };
    let shown = if searching && !query.trim().is_empty() {
        POOL.with(|slot| commands::matching(&slot.borrow(), &query))
    } else {
        ACTIONS.with(|slot| slot.borrow().clone())
    };
    let meta = resolved_meta();
    let labels: Vec<String> = shown.iter().map(|cmd| cmd.title.clone()).collect();
    let titles: Vec<&str> = labels.iter().map(String::as_str).collect();
    let inner = WIDTH - PAD * 2.0;
    let frames = if shown.is_empty() {
        Vec::new()
    } else {
        commands::layout_chips(&titles, inner)
    };
    let show_empty = shown.is_empty() && searching && !query.trim().is_empty();
    let placed = place_sections(&meta, searching, &frames, show_empty);
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    place_window(mtm, placed.height, fresh_place);
    let title = CARD_TITLE.with(|slot| format!("{} · {HOTKEY}", slot.borrow()));
    HEADER.with(|slot| {
        set_label(slot, PAD, placed.header_y, inner - 64.0, HEADER_H, &title);
    });
    MORE.with(|slot| place_header_button(slot, WIDTH - PAD - 52.0, placed.header_y));
    CLOSE.with(|slot| place_header_button(slot, WIDTH - PAD - 24.0, placed.header_y));
    place_well(placed.preview_y);
    apply_preview(placed.preview_y);
    META.with(|slot| set_label(slot, PAD + 2.0, placed.meta_y, inner - 4.0, META_H, &meta));
    META.with(|slot| {
        if let Some(label) = slot.borrow().as_ref() {
            label.setHidden(meta.is_empty());
        }
    });
    FIELD.with(|slot| {
        if let Some(field) = slot.borrow().as_ref() {
            field.setHidden(false);
            let (field_y, field_h, alpha) = if searching {
                (placed.search_y + 4.0, 28.0, 1.0)
            } else {
                (0.0, 0.0, 0.0)
            };
            field.setFrame(NSRect::new(
                NSPoint::new(PAD, field_y),
                NSSize::new(inner, field_h),
            ));
            field.setAlphaValue(alpha);
        }
    });
    PILLS.with(|slot| {
        if let Some(pills) = slot.borrow().as_ref() {
            pills.setFrame(NSRect::new(
                NSPoint::new(PAD, placed.chips_y),
                NSSize::new(inner, placed.chips_h),
            ));
            rebuild_pills(mtm, pills, &shown, &frames, show_empty);
        }
    });
    let selected = SELECTION.with(Cell::get);
    if shown.is_empty() {
        SELECTION.set(0);
    } else if selected >= shown.len() {
        SELECTION.set(shown.len() - 1);
    }
    SHOWN.with(|slot| slot.replace(shown));
    FRAMES.with(|slot| slot.replace(frames));
    paint_pills();
}

struct Sections {
    height: f64,
    header_y: f64,
    preview_y: f64,
    meta_y: f64,
    search_y: f64,
    chips_y: f64,
    chips_h: f64,
}

fn place_sections(meta: &str, searching: bool, frames: &[ChipFrame], show_empty: bool) -> Sections {
    let meta_h = if meta.is_empty() { 0.0 } else { META_H };
    let search_h = if searching { SEARCH_H } else { 0.0 };
    let chips_h = if show_empty {
        28.0
    } else {
        commands::chips_height(frames)
    };
    let gap_after_preview = if meta_h > 0.0 || search_h > 0.0 || chips_h > 0.0 {
        GAP
    } else {
        0.0
    };
    let gap_after_meta = if meta_h > 0.0 && (search_h > 0.0 || chips_h > 0.0) {
        GAP
    } else {
        0.0
    };
    let gap_after_search = if search_h > 0.0 && chips_h > 0.0 {
        4.0
    } else {
        0.0
    };
    let height = PAD
        + HEADER_H
        + GAP
        + PREVIEW_H
        + gap_after_preview
        + meta_h
        + gap_after_meta
        + search_h
        + gap_after_search
        + chips_h
        + PAD;
    let mut cursor = height - PAD;
    cursor -= HEADER_H;
    let header_y = cursor;
    cursor -= GAP + PREVIEW_H;
    let preview_y = cursor;
    cursor -= gap_after_preview + meta_h;
    let meta_y = cursor;
    cursor -= gap_after_meta + search_h;
    let search_y = cursor;
    cursor -= gap_after_search + chips_h;
    let chips_y = cursor;
    Sections {
        height,
        header_y,
        preview_y,
        meta_y,
        search_y,
        chips_y,
        chips_h,
    }
}

fn place_window(mtm: MainThreadMarker, height: f64, fresh: bool) {
    WINDOW.with(|slot| {
        let borrowed = slot.borrow();
        let Some(window) = borrowed.as_ref() else {
            return;
        };
        let frame = if fresh {
            cursor_frame(mtm, height)
        } else {
            let current = window.frame();
            let top = current.origin.y + current.size.height;
            NSRect::new(
                NSPoint::new(current.origin.x, top - height),
                NSSize::new(WIDTH, height),
            )
        };
        window.setFrame_display(frame, true);
    });
}

fn cursor_frame(mtm: MainThreadMarker, height: f64) -> NSRect {
    let cursor = NSEvent::mouseLocation();
    let visible = screen_for_point(mtm, cursor)
        .map(|screen| screen.visibleFrame())
        .unwrap_or_else(|| NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1440.0, 900.0)));
    let min_x = visible.origin.x + 8.0;
    let max_x = visible.origin.x + visible.size.width - WIDTH - 8.0;
    let x = clamp_axis(cursor.x - 36.0, min_x, max_x);
    let top = visible.origin.y + visible.size.height - 8.0;
    let mut y = cursor.y + 12.0;
    if y + height > top {
        y = cursor.y - 12.0 - height;
    }
    y = clamp_axis(
        y,
        visible.origin.y + 8.0,
        (top - height).max(visible.origin.y),
    );
    NSRect::new(NSPoint::new(x, y), NSSize::new(WIDTH, height))
}

fn screen_for_point(mtm: MainThreadMarker, point: NSPoint) -> Option<Retained<NSScreen>> {
    let screens = NSScreen::screens(mtm);
    for screen in screens.iter() {
        let frame = screen.frame();
        let inside = point.x >= frame.origin.x
            && point.x < frame.origin.x + frame.size.width
            && point.y >= frame.origin.y
            && point.y < frame.origin.y + frame.size.height;
        if inside {
            return Some(screen);
        }
    }
    NSScreen::mainScreen(mtm)
}

fn clamp_axis(value: f64, min: f64, max: f64) -> f64 {
    if max < min {
        min
    } else {
        value.clamp(min, max)
    }
}

fn place_well(y: f64) {
    WELL.with(|slot| {
        if let Some(well) = slot.borrow().as_ref() {
            well.setFrame(NSRect::new(
                NSPoint::new(PAD, y),
                NSSize::new(WIDTH - PAD * 2.0, PREVIEW_H),
            ));
        }
    });
}

fn resolved_meta() -> String {
    let page = LINK_PAGE.with(|slot| slot.borrow().clone());
    let caption = LINK_CAPTION.with(|slot| slot.borrow().clone());
    if let (Some(page), Some((cached, caption))) = (page, caption)
        && cached == page
        && !caption.is_empty()
    {
        return caption;
    }
    CARD_META.with(|slot| slot.borrow().clone())
}

fn apply_preview(y: f64) {
    let frame = NSRect::new(
        NSPoint::new(PAD + 8.0, y + 8.0),
        NSSize::new(WIDTH - PAD * 2.0 - 16.0, PREVIEW_H - 16.0),
    );
    let pasteboard = SHOWS_IMAGE.with(Cell::get);
    let link_image = if pasteboard {
        None
    } else {
        loaded_link_image()
    };
    let shows_picture = pasteboard || link_image.is_some();
    PREVIEW_IMAGE.with(|slot| {
        if let Some(view) = slot.borrow().as_ref() {
            view.setFrame(frame);
            view.setHidden(!shows_picture);
            if let Some(image) = link_image.as_ref() {
                view.setImage(Some(image));
            }
        }
    });
    PREVIEW_TEXT.with(|slot| {
        if let Some(view) = slot.borrow().as_ref() {
            view.setFrame(NSRect::new(
                NSPoint::new(PAD, y),
                NSSize::new(WIDTH - PAD * 2.0, PREVIEW_H),
            ));
            view.setHidden(shows_picture);
        }
    });
    if pasteboard {
        load_thumbnail();
        return;
    }
    if link_image.is_some() {
        THUMB_TOKEN.set(-1);
        return;
    }
    THUMB_TOKEN.set(-1);
    let excerpt = CARD_EXCERPT.with(|slot| slot.borrow().clone());
    let placeholder = CARD_PLACEHOLDER.with(|slot| slot.borrow().clone());
    let failed = link_preview_failed();
    let payload = !excerpt.is_empty() && !failed;
    let body = if failed {
        "No preview".to_string()
    } else if !excerpt.is_empty() {
        excerpt
    } else {
        placeholder
    };
    PREVIEW_TEXT.with(|slot| {
        let borrowed = slot.borrow();
        let Some(view) = borrowed.as_ref() else {
            return;
        };
        view.setString(&NSString::from_str(&body));
        let font = if payload {
            NSFont::userFixedPitchFontOfSize(12.0).unwrap_or_else(|| NSFont::systemFontOfSize(12.0))
        } else {
            NSFont::systemFontOfSize(13.0)
        };
        let color = if payload {
            NSColor::labelColor()
        } else {
            NSColor::secondaryLabelColor()
        };
        view.setFont(Some(&font));
        view.setTextColor(Some(&color));
    });
    ensure_link_preview();
}

fn loaded_link_image() -> Option<Retained<NSImage>> {
    let page = LINK_PAGE.with(|slot| slot.borrow().clone())?;
    LINK_IMAGE.with(|slot| {
        slot.borrow()
            .as_ref()
            .and_then(|(url, image)| (url == &page).then(|| image.clone()))
    })
}

fn ensure_link_preview() {
    let Some(page) = LINK_PAGE.with(|slot| slot.borrow().clone()) else {
        return;
    };
    if loaded_link_image().is_some() {
        return;
    }
    if LINK_IN_FLIGHT.with(|slot| slot.borrow().as_deref() == Some(page.as_str())) {
        return;
    }
    if LINK_FAILED.with(|slot| slot.borrow().as_deref() == Some(page.as_str())) {
        return;
    }
    let ticket = LINK_GEN.with(|cell| {
        let next = cell.get().wrapping_add(1);
        cell.set(next);
        next
    });
    LINK_IN_FLIGHT.with(|slot| slot.replace(Some(page.clone())));
    let thumb = LINK_THUMB.with(|slot| slot.borrow().clone());
    std::thread::spawn(move || {
        let (bytes, caption) =
            objc2::rc::autoreleasepool(|_| load_link_preview(&page, thumb.as_deref()));
        dispatch2::DispatchQueue::main().exec_async(move || {
            finish_link_preview(ticket, page, bytes, caption);
        });
    });
}

fn link_preview_failed() -> bool {
    let page = LINK_PAGE.with(|slot| slot.borrow().clone());
    LINK_FAILED.with(|slot| slot.borrow().as_deref() == page.as_deref() && page.is_some())
}

fn usable_thumbnail(bytes: &[u8]) -> bool {
    bytes.len() > 8_000 && (bytes.starts_with(b"\xFF\xD8") || bytes.starts_with(b"\x89PNG"))
}

fn load_link_preview(page: &str, thumb: Option<&str>) -> (Vec<u8>, Option<String>) {
    if let Some(id) = crate::youtube::video_id(page) {
        let bytes = crate::macos_fetch::get(&crate::youtube::wide_thumbnail_url(id))
            .filter(|bytes| usable_thumbnail(bytes))
            .or_else(|| thumb.and_then(crate::macos_fetch::get))
            .unwrap_or_default();
        let watch = crate::format::format_text(page);
        let caption = crate::macos_fetch::get(&crate::youtube::oembed_endpoint(&watch))
            .and_then(|body| crate::youtube::caption_from_oembed(&body));
        return (bytes, caption);
    }
    let fetch_at = crate::page_preview::canonical_url(page).unwrap_or_else(|| page.to_string());
    let html = crate::macos_fetch::get_document(&fetch_at).unwrap_or_default();
    let text = String::from_utf8_lossy(&html);
    let found = crate::page_preview::from_html(&text, &fetch_at);
    let bytes = found
        .image
        .as_deref()
        .and_then(crate::macos_fetch::get_asset)
        .unwrap_or_default();
    (bytes, found.title)
}

fn finish_link_preview(ticket: u64, page: String, bytes: Vec<u8>, caption: Option<String>) {
    if LINK_GEN.with(Cell::get) != ticket {
        return;
    }
    LINK_IN_FLIGHT.with(|slot| slot.replace(None));
    if let Some(caption) = caption {
        LINK_CAPTION.with(|slot| slot.replace(Some((page.clone(), caption))));
    }
    let Some(image) = nsimage_from_bytes(&bytes) else {
        LINK_FAILED.with(|slot| slot.replace(Some(page)));
        if OPEN.with(Cell::get) {
            layout(false);
        }
        return;
    };
    LINK_FAILED.with(|slot| slot.replace(None));
    LINK_IMAGE.with(|slot| slot.replace(Some((page, image))));
    if OPEN.with(Cell::get) {
        layout(false);
    }
}

fn load_thumbnail() {
    let change = crate::macos_pasteboard::change_count();
    if THUMB_TOKEN.with(Cell::get) == change {
        return;
    }
    THUMB_TOKEN.set(change);
    let image =
        crate::macos_pasteboard::image_thumbnail().and_then(|thumb| nsimage_from_clipboard(&thumb));
    PREVIEW_IMAGE.with(|slot| {
        if let Some(view) = slot.borrow().as_ref() {
            view.setImage(image.as_deref());
        }
    });
}

fn rebuild_pills(
    mtm: MainThreadMarker,
    list: &NSView,
    shown: &[Command],
    frames: &[ChipFrame],
    show_empty: bool,
) {
    while list.subviews().count() > 0 {
        list.subviews().objectAtIndex(0).removeFromSuperview();
    }
    if shown.is_empty() {
        if show_empty {
            let empty = static_label(mtm, 13.0, &NSColor::secondaryLabelColor());
            empty.setFrame(NSRect::new(
                NSPoint::new(2.0, 4.0),
                NSSize::new(WIDTH - PAD * 2.0, 20.0),
            ));
            empty.setStringValue(&NSString::from_str("No matching commands"));
            list.addSubview(&empty);
        }
        return;
    }
    let area_h = commands::chips_height(frames);
    for (index, cmd) in shown.iter().enumerate() {
        let Some(frame) = frames.get(index) else {
            continue;
        };
        let y = area_h - (frame.row as f64 + 1.0) * commands::CHIP_PITCH
            + (commands::CHIP_PITCH - commands::CHIP_PILL_H) / 2.0;
        let pill = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(
                NSPoint::new(frame.x, y),
                NSSize::new(frame.width, commands::CHIP_PILL_H),
            ),
        );
        let fill = filled_box(mtm, commands::CHIP_PILL_H / 2.0, &NSColor::clearColor());
        fill.setFrame(NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(frame.width, commands::CHIP_PILL_H),
        ));
        let title = static_label(mtm, 13.0, &NSColor::labelColor());
        title.setAlignment(NSTextAlignment::Center);
        title.setFrame(NSRect::new(
            NSPoint::new(8.0, 5.0),
            NSSize::new((frame.width - 16.0).max(8.0), 18.0),
        ));
        title.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
        title.setStringValue(&NSString::from_str(&cmd.title));
        let hit = NSButton::initWithFrame(
            NSButton::alloc(mtm),
            NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(frame.width, commands::CHIP_PILL_H),
            ),
        );
        hit.setBordered(false);
        hit.setTransparent(true);
        hit.setTitle(&NSString::from_str(""));
        hit.setTag(index as isize);
        wire_button(&hit, sel!(chipClicked:));
        pill.addSubview(&fill);
        pill.addSubview(&title);
        pill.addSubview(&hit);
        list.addSubview(&pill);
    }
}

fn paint_pills() {
    if SHOWN.with(|slot| slot.borrow().is_empty()) {
        return;
    }
    let selected = SELECTION.with(Cell::get);
    PILLS.with(|slot| {
        let borrowed = slot.borrow();
        let Some(list) = borrowed.as_ref() else {
            return;
        };
        for (index, pill) in list.subviews().iter().enumerate() {
            paint_pill(&pill, index == selected);
        }
    });
}

fn paint_pill(pill: &NSView, selected: bool) {
    let fill = if selected {
        NSColor::controlAccentColor()
    } else {
        NSColor::unemphasizedSelectedContentBackgroundColor()
    };
    let text = if selected {
        NSColor::whiteColor()
    } else {
        NSColor::labelColor()
    };
    for view in pill.subviews().iter() {
        if let Some(box_view) = view.downcast_ref::<NSBox>() {
            box_view.setFillColor(&fill);
        } else if let Some(label) = view.downcast_ref::<NSTextField>() {
            label.setTextColor(Some(&text));
        }
    }
}

fn nudge(dx: isize, dy: isize) {
    let frames = FRAMES.with(|slot| slot.borrow().clone());
    let next = commands::step_chip(&frames, SELECTION.with(Cell::get), dx, dy);
    if next == SELECTION.with(Cell::get) {
        return;
    }
    SELECTION.set(next);
    paint_pills();
}

fn activate_selected() {
    let cmd = SHOWN.with(|slot| slot.borrow().get(SELECTION.with(Cell::get)).cloned());
    let Some(cmd) = cmd else {
        return;
    };
    run_command(cmd);
}

fn overflow_item(mtm: MainThreadMarker, title: &str, index: usize) -> Retained<NSMenuItem> {
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str(title),
            Some(sel!(overflowClicked:)),
            &NSString::from_str(""),
        )
    };
    item.setTag(index as isize);
    DELEGATE.with(|slot| {
        if let Some(delegate) = slot.borrow().as_ref() {
            unsafe {
                item.setTarget(Some(delegate));
            }
        }
    });
    item
}

fn activate_overflow(index: usize) {
    let cmd = OVERFLOW.with(|slot| slot.borrow().get(index).cloned());
    let Some(cmd) = cmd else {
        return;
    };
    run_command(cmd);
}

fn run_command(cmd: Command) {
    let formatting_link =
        cmd.id == CommandId::Format && LINK_PAGE.with(|slot| slot.borrow().is_some());
    if !commands::keeps_card_open(&cmd.id) && !formatting_link {
        hide();
    }
    launcher::emit(UserEvent::Run(cmd.id));
}

fn pop_overflow() {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let items = OVERFLOW.with(|slot| slot.borrow().clone());
    let theme = THEME.with(Cell::get);
    let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str(""));
    menu.setAutoenablesItems(false);
    let mut saw_appearance = false;
    let mut saw_quit = false;
    let mut history_menu: Option<Retained<NSMenu>> = None;
    for (index, cmd) in items.iter().enumerate() {
        let in_history = matches!(cmd.id, CommandId::History(_) | CommandId::ClearHistory);
        if in_history {
            let submenu = history_menu.get_or_insert_with(|| {
                let parent = unsafe {
                    NSMenuItem::initWithTitle_action_keyEquivalent(
                        NSMenuItem::alloc(mtm),
                        &NSString::from_str("History"),
                        None,
                        &NSString::from_str(""),
                    )
                };
                let submenu =
                    NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("History"));
                submenu.setAutoenablesItems(false);
                parent.setSubmenu(Some(&submenu));
                menu.addItem(&parent);
                submenu
            });
            let item = overflow_item(mtm, &cmd.title, index);
            submenu.addItem(&item);
            continue;
        }
        if !saw_appearance && matches!(cmd.id, CommandId::Appearance(_)) {
            menu.addItem(&NSMenuItem::separatorItem(mtm));
            saw_appearance = true;
        }
        if !saw_quit && matches!(cmd.id, CommandId::Quit) {
            menu.addItem(&NSMenuItem::separatorItem(mtm));
            saw_quit = true;
        }
        let item = overflow_item(mtm, &cmd.title, index);
        if let CommandId::Appearance(item_theme) = cmd.id {
            let state = if item_theme == theme {
                NSControlStateValueOn
            } else {
                NSControlStateValueOff
            };
            item.setState(state);
        }
        DELEGATE.with(|slot| {
            if let Some(delegate) = slot.borrow().as_ref() {
                unsafe {
                    item.setTarget(Some(delegate));
                }
            }
        });
        menu.addItem(&item);
    }
    let button = MORE.with(|slot| slot.borrow().clone());
    let Some(button) = button else {
        return;
    };
    SUPPRESS_RESIGN.set(true);
    menu.popUpMenuPositioningItem_atLocation_inView(None, NSPoint::new(0.0, 0.0), Some(&*button));
    SUPPRESS_RESIGN.set(false);
    focus_card();
}

fn on_key(event: &NSEvent) {
    match event.keyCode() {
        123 => nudge(-1, 0),
        124 => nudge(1, 0),
        126 => nudge(0, -1),
        125 => nudge(0, 1),
        36 | 76 => activate_selected(),
        53 => {
            if SEARCHING.with(Cell::get) {
                close_search();
            } else {
                hide();
            }
        }
        _ => begin_search_from_key(event),
    }
}

fn begin_search_from_key(event: &NSEvent) {
    if SEARCHING.with(Cell::get) {
        return;
    }
    let flags = event.modifierFlags();
    if flags.contains(NSEventModifierFlags::Command)
        || flags.contains(NSEventModifierFlags::Control)
    {
        return;
    }
    let Some(text) = event.characters() else {
        return;
    };
    let text = text.to_string();
    if text == "/" {
        open_search("");
    } else if is_search_text(&text) {
        open_search(&text);
    }
}

fn is_search_text(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|ch| !ch.is_control())
}

fn open_search(initial: &str) {
    SEARCHING.set(true);
    set_query(initial);
    SELECTION.set(0);
    layout(false);
    focus_field();
}

fn close_search() {
    if !SEARCHING.with(Cell::get) {
        hide();
        return;
    }
    SEARCHING.set(false);
    set_query("");
    SELECTION.set(0);
    layout(false);
    focus_card();
}

fn current_query() -> String {
    FIELD.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|field| field.stringValue().to_string())
            .unwrap_or_default()
    })
}

fn set_query(text: &str) {
    FIELD.with(|slot| {
        if let Some(field) = slot.borrow().as_ref() {
            field.setStringValue(&NSString::from_str(text));
        }
    });
}

fn focus_card() {
    focus_field();
}

fn focus_field() {
    let field = FIELD.with(|slot| slot.borrow().clone());
    let window = WINDOW.with(|slot| slot.borrow().clone());
    if let (Some(field), Some(window)) = (field, window) {
        window.makeFirstResponder(Some(&field));
    }
}

fn set_label(
    slot: &RefCell<Option<Retained<NSTextField>>>,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    text: &str,
) {
    let borrowed = slot.borrow();
    let Some(label) = borrowed.as_ref() else {
        return;
    };
    label.setFrame(NSRect::new(NSPoint::new(x, y), NSSize::new(width, height)));
    label.setStringValue(&NSString::from_str(text));
}

fn place_header_button(slot: &RefCell<Option<Retained<NSButton>>>, x: f64, header_y: f64) {
    let borrowed = slot.borrow();
    let Some(button) = borrowed.as_ref() else {
        return;
    };
    let y = header_y + (HEADER_H - 22.0) / 2.0;
    button.setFrame(NSRect::new(NSPoint::new(x, y), NSSize::new(22.0, 22.0)));
}

fn search_field(mtm: MainThreadMarker) -> Retained<NSTextField> {
    let field = NSTextField::initWithFrame(NSTextField::alloc(mtm), NSRect::ZERO);
    field.setBordered(false);
    field.setDrawsBackground(false);
    field.setEditable(true);
    field.setSelectable(true);
    field.setFocusRingType(NSFocusRingType::None);
    field.setFont(Some(&NSFont::systemFontOfSize(16.0)));
    field.setTextColor(Some(&NSColor::labelColor()));
    field.setPlaceholderString(Some(&NSString::from_str("Search")));
    field.setAlphaValue(0.0);
    DELEGATE.with(|slot| {
        if let Some(delegate) = slot.borrow().as_ref() {
            unsafe {
                let _: () = msg_send![&*field, setDelegate: &**delegate];
            }
        }
    });
    field
}

fn payload_view(mtm: MainThreadMarker) -> Retained<NSTextView> {
    let text = NSTextView::initWithFrame(NSTextView::alloc(mtm), NSRect::ZERO);
    text.setEditable(false);
    text.setSelectable(false);
    text.setDrawsBackground(false);
    text.setRichText(false);
    text.setTextContainerInset(NSSize::new(12.0, 10.0));
    text.setFont(Some(&NSFont::systemFontOfSize(12.0)));
    text.setTextColor(Some(&NSColor::labelColor()));
    text
}

fn image_view(mtm: MainThreadMarker) -> Retained<NSImageView> {
    let view = NSImageView::initWithFrame(NSImageView::alloc(mtm), NSRect::ZERO);
    view.setEditable(false);
    view.setImageScaling(NSImageScaling::ScaleProportionallyUpOrDown);
    view.setImageAlignment(NSImageAlignment::AlignCenter);
    view.setHidden(true);
    view
}

fn icon_button(mtm: MainThreadMarker, title: &str, action: Sel) -> Retained<NSButton> {
    let button = NSButton::initWithFrame(NSButton::alloc(mtm), NSRect::ZERO);
    button.setBordered(false);
    button.setFocusRingType(NSFocusRingType::None);
    button.setTitle(&NSString::from_str(title));
    button.setFont(Some(&NSFont::systemFontOfSize(14.0)));
    wire_button(&button, action);
    button
}

fn wire_button(button: &NSButton, action: Sel) {
    DELEGATE.with(|slot| {
        if let Some(delegate) = slot.borrow().as_ref() {
            unsafe {
                button.setTarget(Some(delegate));
                button.setAction(Some(action));
            }
        }
    });
}

fn static_label(mtm: MainThreadMarker, size: f64, color: &NSColor) -> Retained<NSTextField> {
    let field = NSTextField::initWithFrame(NSTextField::alloc(mtm), NSRect::ZERO);
    field.setEditable(false);
    field.setSelectable(false);
    field.setBordered(false);
    field.setDrawsBackground(false);
    field.setFont(Some(&NSFont::systemFontOfSize(size)));
    field.setTextColor(Some(color));
    field.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    field.setUsesSingleLineMode(true);
    field
}

fn filled_box(mtm: MainThreadMarker, radius: f64, color: &NSColor) -> Retained<NSBox> {
    let fill = NSBox::initWithFrame(NSBox::alloc(mtm), NSRect::ZERO);
    fill.setBoxType(NSBoxType::Custom);
    fill.setBorderWidth(0.0);
    fill.setCornerRadius(radius);
    fill.setTitlePosition(NSTitlePosition::NoTitle);
    fill.setContentViewMargins(NSSize::new(0.0, 0.0));
    fill.setFillColor(color);
    fill
}

fn round_view(view: &NSView, radius: f64) {
    view.setWantsLayer(true);
    unsafe {
        let layer: *mut AnyObject = msg_send![view, layer];
        if !layer.is_null() {
            let _: () = msg_send![layer, setCornerRadius: radius];
            let _: () = msg_send![layer, setMasksToBounds: true];
        }
    }
}
