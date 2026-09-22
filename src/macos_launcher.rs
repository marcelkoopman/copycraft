#![cfg(target_os = "macos")]

use std::cell::{Cell, RefCell};

use objc2::rc::Retained;
use objc2::runtime::{NSObject, Sel};
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSBackingStoreType, NSButton, NSColor, NSControl, NSFloatingWindowLevel, NSFocusRingType,
    NSFont, NSLineBreakMode, NSScreen, NSTextAlignment, NSTextField, NSTextView, NSView,
    NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView,
    NSWindow, NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_foundation::{NSNotification, NSPoint, NSRect, NSSize, NSString};

use crate::commands::{self, Command, CommandId, LaunchData};
use crate::launcher::{self, UserEvent};

const WIDTH: f64 = 680.0;
const ROW_H: f64 = 36.0;
const SEARCH_H: f64 = 46.0;
const HEADER_H: f64 = 28.0;
const PAD: f64 = 10.0;
const TOP_GAP: f64 = 88.0;

thread_local! {
    static OPEN: Cell<bool> = const { Cell::new(false) };
    static SUPPRESS_RESIGN: Cell<bool> = const { Cell::new(false) };
    static SELECTION: Cell<usize> = const { Cell::new(0) };
    static COMMANDS: RefCell<Vec<Command>> = const { RefCell::new(Vec::new()) };
    static VISIBLE_ROWS: RefCell<Vec<Command>> = const { RefCell::new(Vec::new()) };
    static CONTEXT: RefCell<String> = const { RefCell::new(String::new()) };
    static WINDOW: RefCell<Option<Retained<LauncherWindow>>> = const { RefCell::new(None) };
    static FIELD: RefCell<Option<Retained<NSTextField>>> = const { RefCell::new(None) };
    static HEADER: RefCell<Option<Retained<NSTextField>>> = const { RefCell::new(None) };
    static HOTKEY: RefCell<Option<Retained<NSTextField>>> = const { RefCell::new(None) };
    static LIST: RefCell<Option<Retained<NSView>>> = const { RefCell::new(None) };
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
                nudge(1);
                true
            } else if command == sel!(moveUp:) {
                nudge(-1);
                true
            } else if command == sel!(insertNewline:) {
                activate_selected();
                true
            } else if command == sel!(cancelOperation:) {
                hide();
                true
            } else {
                false
            }
        }

        #[unsafe(method(rowClicked:))]
        fn row_clicked(&self, sender: Option<&NSButton>) {
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
    focus_field();
}

fn store(data: LaunchData) {
    CONTEXT.with(|slot| slot.replace(commands::context_line(&data)));
    COMMANDS.with(|slot| slot.replace(commands::list(&data)));
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
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WIDTH, 240.0));
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

    let content = window.contentView().expect("content view");
    content.setWantsLayer(true);
    round_view(&content, 14.0);

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
    round_view(&frosted, 14.0);
    content.addSubview(&frosted);

    let header = static_label(mtm, 13.0, &NSColor::secondaryLabelColor());
    let hotkey = static_label(mtm, 12.0, &NSColor::secondaryLabelColor());
    hotkey.setAlignment(NSTextAlignment::Right);
    hotkey.setStringValue(&NSString::from_str("⌃⌥⌘F"));
    let field = NSTextField::initWithFrame(
        NSTextField::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WIDTH - PAD * 2.0, 28.0)),
    );
    field.setBordered(false);
    field.setDrawsBackground(false);
    field.setEditable(true);
    field.setSelectable(true);
    field.setFocusRingType(NSFocusRingType::None);
    field.setFont(Some(&NSFont::systemFontOfSize(20.0)));
    field.setTextColor(Some(&NSColor::labelColor()));
    field.setPlaceholderString(Some(&NSString::from_str("Format, convert, decode…")));
    unsafe {
        let _: () = msg_send![&*field, setDelegate: &*delegate];
    }

    let list = NSView::initWithFrame(
        NSView::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0)),
    );
    content.addSubview(&header);
    content.addSubview(&hotkey);
    content.addSubview(&field);
    content.addSubview(&list);

    HEADER.with(|slot| slot.replace(Some(header)));
    HOTKEY.with(|slot| slot.replace(Some(hotkey)));
    FIELD.with(|slot| slot.replace(Some(field)));
    LIST.with(|slot| slot.replace(Some(list)));
    DELEGATE.with(|slot| slot.replace(Some(delegate)));
    WINDOW.with(|slot| slot.replace(Some(window)));
}

fn layout(fresh_place: bool) {
    let query = current_query();
    let visible = COMMANDS.with(|slot| commands::visible(&slot.borrow(), &query));
    let row_count = visible.len().max(1);
    let height = PAD + HEADER_H + SEARCH_H + (row_count as f64) * ROW_H + PAD;
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    place_window(mtm, height, fresh_place);
    let width = WIDTH;
    let header_y = height - PAD - HEADER_H;
    let search_y = header_y - SEARCH_H + 8.0;
    HEADER.with(|slot| {
        if let Some(header) = slot.borrow().as_ref() {
            header.setFrame(NSRect::new(
                NSPoint::new(PAD + 6.0, header_y),
                NSSize::new(width - PAD * 2.0 - 72.0, HEADER_H),
            ));
            header.setStringValue(&NSString::from_str(
                &CONTEXT.with(|text| text.borrow().clone()),
            ));
        }
    });
    HOTKEY.with(|slot| {
        if let Some(hotkey) = slot.borrow().as_ref() {
            hotkey.setFrame(NSRect::new(
                NSPoint::new(width - PAD - 78.0, header_y),
                NSSize::new(68.0, HEADER_H),
            ));
        }
    });
    FIELD.with(|slot| {
        if let Some(field) = slot.borrow().as_ref() {
            field.setFrame(NSRect::new(
                NSPoint::new(PAD + 4.0, search_y),
                NSSize::new(width - PAD * 2.0 - 8.0, 32.0),
            ));
        }
    });
    let list_h = (row_count as f64) * ROW_H;
    LIST.with(|slot| {
        if let Some(list) = slot.borrow().as_ref() {
            list.setFrame(NSRect::new(
                NSPoint::new(0.0, PAD),
                NSSize::new(width, list_h),
            ));
            rebuild_rows(mtm, list, &visible);
        }
    });
    let selected = SELECTION.with(Cell::get);
    if visible.is_empty() {
        SELECTION.set(0);
    } else if selected >= visible.len() {
        SELECTION.set(visible.len() - 1);
    }
    VISIBLE_ROWS.with(|slot| slot.replace(visible));
    paint_selection();
}

fn place_window(mtm: MainThreadMarker, height: f64, fresh: bool) {
    WINDOW.with(|slot| {
        let borrowed = slot.borrow();
        let Some(window) = borrowed.as_ref() else {
            return;
        };
        let frame = if fresh {
            let visible = NSScreen::mainScreen(mtm).map(|screen| screen.visibleFrame());
            let visible = visible
                .unwrap_or_else(|| NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1440.0, 900.0)));
            let x = visible.origin.x + (visible.size.width - WIDTH) / 2.0;
            let top = visible.origin.y + visible.size.height - TOP_GAP;
            NSRect::new(NSPoint::new(x, top - height), NSSize::new(WIDTH, height))
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

fn rebuild_rows(mtm: MainThreadMarker, list: &NSView, visible: &[Command]) {
    while list.subviews().count() > 0 {
        list.subviews().objectAtIndex(0).removeFromSuperview();
    }
    if visible.is_empty() {
        let empty = static_label(mtm, 14.0, &NSColor::secondaryLabelColor());
        empty.setFrame(NSRect::new(
            NSPoint::new(PAD + 8.0, 4.0),
            NSSize::new(WIDTH - PAD * 2.0, ROW_H - 8.0),
        ));
        empty.setStringValue(&NSString::from_str("No matching commands"));
        list.addSubview(&empty);
        return;
    }
    let list_h = list.frame().size.height;
    for (index, cmd) in visible.iter().enumerate() {
        let y = list_h - ((index + 1) as f64) * ROW_H;
        let row = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(
                NSPoint::new(6.0, y + 2.0),
                NSSize::new(WIDTH - 12.0, ROW_H - 4.0),
            ),
        );
        row.setWantsLayer(true);
        round_view(&row, 8.0);

        let title = static_label(mtm, 14.0, &NSColor::labelColor());
        title.setFrame(NSRect::new(
            NSPoint::new(10.0, 6.0),
            NSSize::new(WIDTH - 220.0, 20.0),
        ));
        title.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
        title.setUsesSingleLineMode(true);
        title.setStringValue(&NSString::from_str(&cmd.title));

        let detail = static_label(mtm, 12.0, &NSColor::secondaryLabelColor());
        detail.setAlignment(NSTextAlignment::Right);
        detail.setFrame(NSRect::new(
            NSPoint::new(WIDTH - 12.0 - 180.0, 7.0),
            NSSize::new(168.0, 18.0),
        ));
        detail.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
        detail.setUsesSingleLineMode(true);
        detail.setStringValue(&NSString::from_str(&cmd.detail));

        let hit = NSButton::initWithFrame(
            NSButton::alloc(mtm),
            NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(WIDTH - 12.0, ROW_H - 4.0),
            ),
        );
        hit.setBordered(false);
        hit.setTransparent(true);
        hit.setTitle(&NSString::from_str(""));
        hit.setTag(index as isize);
        DELEGATE.with(|slot| {
            if let Some(delegate) = slot.borrow().as_ref() {
                unsafe {
                    hit.setTarget(Some(delegate));
                    hit.setAction(Some(sel!(rowClicked:)));
                }
            }
        });

        row.addSubview(&title);
        row.addSubview(&detail);
        row.addSubview(&hit);
        list.addSubview(&row);
    }
}

fn paint_selection() {
    let selected = SELECTION.with(Cell::get);
    LIST.with(|slot| {
        let borrowed = slot.borrow();
        let Some(list) = borrowed.as_ref() else {
            return;
        };
        let rows = list.subviews();
        for (index, row) in rows.iter().enumerate() {
            let on = !VISIBLE_ROWS.with(|cmds| cmds.borrow().is_empty()) && index == selected;
            if on {
                let color = NSColor::selectedContentBackgroundColor().colorWithAlphaComponent(0.92);
                set_background(&row, &color);
            } else {
                set_background(&row, &NSColor::clearColor());
            }
            paint_row_text(&row, on);
        }
    });
}

fn paint_row_text(row: &NSView, selected: bool) {
    let title_color = if selected {
        NSColor::alternateSelectedControlTextColor()
    } else {
        NSColor::labelColor()
    };
    let detail_color = if selected {
        NSColor::alternateSelectedControlTextColor()
    } else {
        NSColor::secondaryLabelColor()
    };
    let subs = row.subviews();
    let mut labels = 0;
    for view in subs.iter() {
        let Some(field) = view.downcast_ref::<NSTextField>() else {
            continue;
        };
        let color = if labels == 0 {
            &title_color
        } else {
            &detail_color
        };
        field.setTextColor(Some(color));
        labels += 1;
    }
}

fn nudge(delta: isize) {
    let len = VISIBLE_ROWS.with(|slot| slot.borrow().len());
    if len == 0 {
        return;
    }
    let current = SELECTION.with(Cell::get) as isize;
    let next = (current + delta).clamp(0, len as isize - 1) as usize;
    if next == SELECTION.with(Cell::get) {
        return;
    }
    SELECTION.set(next);
    paint_selection();
}

fn activate_selected() {
    let cmd = VISIBLE_ROWS.with(|slot| slot.borrow().get(SELECTION.with(Cell::get)).cloned());
    let Some(cmd) = cmd else {
        return;
    };
    let keep = matches!(cmd.id, CommandId::ToggleMenuBar | CommandId::Appearance(_));
    if !keep {
        hide();
    }
    launcher::emit(UserEvent::Run(cmd.id));
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

fn focus_field() {
    let field = FIELD.with(|slot| slot.borrow().clone());
    let window = WINDOW.with(|slot| slot.borrow().clone());
    if let (Some(field), Some(window)) = (field, window) {
        unsafe {
            let _: bool = msg_send![&*window, makeFirstResponder: &*field];
        }
    }
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

fn round_view(view: &NSView, radius: f64) {
    view.setWantsLayer(true);
    unsafe {
        let layer: *mut objc2::runtime::AnyObject = msg_send![view, layer];
        if !layer.is_null() {
            let _: () = msg_send![layer, setCornerRadius: radius];
            let _: () = msg_send![layer, setMasksToBounds: true];
        }
    }
}

fn set_background(view: &NSView, color: &NSColor) {
    view.setWantsLayer(true);
    unsafe {
        let layer: *mut objc2::runtime::AnyObject = msg_send![view, layer];
        if layer.is_null() {
            return;
        }
        let cg: *mut objc2::runtime::AnyObject = msg_send![color, CGColor];
        let _: () = msg_send![layer, setBackgroundColor: cg];
    }
}
