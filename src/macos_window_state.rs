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
            flash_button(&COPY_BUTTON, "Copied  \u{2713}", "Copy", copy_flash_color(), true);
            reset_later(self, sel!(resetCopyLabel:));
        }

        #[unsafe(method(resetCopyLabel:))]
        fn reset_copy_label(&self, _sender: Option<&AnyObject>) {
            flash_button(&COPY_BUTTON, "Copied  \u{2713}", "Copy", copy_flash_color(), false);
        }

        #[unsafe(method(originalClicked:))]
        fn original_clicked(&self, _sender: Option<&AnyObject>) {
            let body = SOURCE_TEXT.with(|src| src.borrow().clone());
            apply_preview(&body);
            flash_button(&ORIGINAL_BUTTON, "Original  \u{2713}", "Original", original_flash_color(), true);
            reset_later(self, sel!(resetOriginalLabel:));
        }

        #[unsafe(method(resetOriginalLabel:))]
        fn reset_original_label(&self, _sender: Option<&AnyObject>) {
            flash_button(&ORIGINAL_BUTTON, "Original  \u{2713}", "Original", original_flash_color(), false);
        }

        #[unsafe(method(formatClicked:))]
        fn format_clicked(&self, _sender: Option<&AnyObject>) {
            let body = SOURCE_TEXT.with(|src| clipboard::formatted(&src.borrow()));
            apply_preview(&body);
            flash_button(&FORMAT_BUTTON, "Formatted  \u{2713}", "Format", format_flash_color(), true);
            reset_later(self, sel!(resetFormatLabel:));
        }

        #[unsafe(method(resetFormatLabel:))]
        fn reset_format_label(&self, _sender: Option<&AnyObject>) {
            flash_button(&FORMAT_BUTTON, "Formatted  \u{2713}", "Format", format_flash_color(), false);
        }

        #[unsafe(method(redactClicked:))]
        fn redact_clicked(&self, _sender: Option<&AnyObject>) {
            let body = SOURCE_TEXT.with(|src| redact::redact(&src.borrow()));
            apply_preview(&body);
            flash_button(&REDACT_BUTTON, "Redacted  \u{2713}", "Redact", redact_flash_color(), true);
            reset_later(self, sel!(resetRedactLabel:));
        }

        #[unsafe(method(resetRedactLabel:))]
        fn reset_redact_label(&self, _sender: Option<&AnyObject>) {
            flash_button(&REDACT_BUTTON, "Redacted  \u{2713}", "Redact", redact_flash_color(), false);
        }

        #[unsafe(method(dataframeClicked:))]
        fn dataframe_clicked(&self, _sender: Option<&AnyObject>) {
            let body = SOURCE_TEXT.with(|src| dataframe::try_format(&src.borrow()));
            let Some(body) = body else {
                flash_button(
                    &DATAFRAME_BUTTON,
                    "Failed",
                    "Dataframe",
                    error_flash_color(),
                    true,
                );
                reset_later(self, sel!(resetDataframeLabel:));
                return;
            };
            apply_preview_with_kind(&body, FormatKind::Dataframe);
            flash_button(
                &DATAFRAME_BUTTON,
                "Dataframe  \u{2713}",
                "Dataframe",
                dataframe_flash_color(),
                true,
            );
            reset_later(self, sel!(resetDataframeLabel:));
        }

        #[unsafe(method(resetDataframeLabel:))]
        fn reset_dataframe_label(&self, _sender: Option<&AnyObject>) {
            flash_button(
                &DATAFRAME_BUTTON,
                "Dataframe  \u{2713}",
                "Dataframe",
                dataframe_flash_color(),
                false,
            );
        }

        #[unsafe(method(saveClicked:))]
        fn save_clicked(&self, _sender: Option<&AnyObject>) {
            let saved = save_preview_to_file();
            flash_button(&SAVE_BUTTON, "Saved  \u{2713}", "Save", save_flash_color(), saved);
            if saved {
                reset_later(self, sel!(resetSaveLabel:));
            }
        }

        #[unsafe(method(resetSaveLabel:))]
        fn reset_save_label(&self, _sender: Option<&AnyObject>) {
            flash_button(&SAVE_BUTTON, "Saved  \u{2713}", "Save", save_flash_color(), false);
        }
    }
);

impl PreviewTarget {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this: Allocated<Self> = Self::alloc(mtm);
        unsafe { msg_send![this, init] }
    }
}

fn reset_later(target: &PreviewTarget, selector: objc2::runtime::Sel) {
    unsafe {
        let _: () = msg_send![
            target,
            performSelector: selector,
            withObject: None::<&AnyObject>,
            afterDelay: 1.6
        ];
    }
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
fn error_flash_color() -> Retained<NSColor> {
    NSColor::colorWithCalibratedRed_green_blue_alpha(1.0, 0.35, 0.35, 1.0)
}

fn flash_button(
    slot: &'static std::thread::LocalKey<RefCell<Option<Retained<NSButton>>>>,
    done_label: &str,
    idle_label: &str,
    flash: Retained<NSColor>,
    done: bool,
) {
    slot.with(|cell| {
        let borrowed = cell.borrow();
        let Some(button) = borrowed.as_ref() else {
            return;
        };
        let label = if done { done_label } else { idle_label };
        let color = if done { flash } else { idle_button_color() };
        style_title_button(button, label, &color);
    });
}

include!("macos_window_ops.rs");
