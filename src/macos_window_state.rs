thread_local! {
    static WINDOW: RefCell<Option<Retained<NSWindow>>> = const { RefCell::new(None) };
    static TARGET: RefCell<Option<Retained<PreviewTarget>>> = const { RefCell::new(None) };
    static TEXT: RefCell<Option<Retained<NSTextView>>> = const { RefCell::new(None) };
    static SCROLL: RefCell<Option<Retained<NSScrollView>>> = const { RefCell::new(None) };
    static COPY_BUTTON: RefCell<Option<Retained<NSButton>>> = const { RefCell::new(None) };
    static ORIGINAL_BUTTON: RefCell<Option<Retained<NSButton>>> = const { RefCell::new(None) };
    static FORMAT_BUTTON: RefCell<Option<Retained<NSButton>>> = const { RefCell::new(None) };
    static CONVERT_BUTTON: RefCell<Option<Retained<NSButton>>> = const { RefCell::new(None) };
    static DECODE_BUTTON: RefCell<Option<Retained<NSButton>>> = const { RefCell::new(None) };
    static COMPRESS_BUTTON: RefCell<Option<Retained<NSButton>>> = const { RefCell::new(None) };
    static REDACT_BUTTON: RefCell<Option<Retained<NSButton>>> = const { RefCell::new(None) };
    static DATAFRAME_BUTTON: RefCell<Option<Retained<NSButton>>> = const { RefCell::new(None) };
    static SAVE_BUTTON: RefCell<Option<Retained<NSButton>>> = const { RefCell::new(None) };
    static SOURCE_TEXT: RefCell<String> = const { RefCell::new(String::new()) };
    static PREVIEW_TEXT: RefCell<String> = const { RefCell::new(String::new()) };
    static PREVIEW_KIND: RefCell<FormatKind> = const { RefCell::new(FormatKind::Plain) };
    static SOURCE_KIND: RefCell<FormatKind> = const { RefCell::new(FormatKind::Plain) };
    static VIEW_MODE: RefCell<ViewMode> = const { RefCell::new(ViewMode::Format) };
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
            select_mode(ViewMode::Original);
        }

        #[unsafe(method(formatClicked:))]
        fn format_clicked(&self, _sender: Option<&AnyObject>) {
            let body = SOURCE_TEXT.with(|src| clipboard::formatted(&src.borrow()));
            apply_preview(&body);
            select_mode(ViewMode::Format);
        }

        #[unsafe(method(convertClicked:))]
        fn convert_clicked(&self, _sender: Option<&AnyObject>) {
            let body = SOURCE_TEXT.with(|src| convert::try_convert(&src.borrow()));
            let Some(body) = body else {
                flash_button(&CONVERT_BUTTON, "Failed", "Convert", error_flash_color(), true);
                reset_later(self, sel!(resetConvertLabel:));
                return;
            };
            apply_preview(&clipboard::formatted(&body));
            select_mode(ViewMode::Convert);
        }

        #[unsafe(method(resetConvertLabel:))]
        fn reset_convert_label(&self, _sender: Option<&AnyObject>) {
            paint_mode_buttons();
        }

        #[unsafe(method(decodeClicked:))]
        fn decode_clicked(&self, _sender: Option<&AnyObject>) {
            let body = SOURCE_TEXT.with(|src| decode::try_decode(&src.borrow()));
            let Some(body) = body else {
                flash_button(&DECODE_BUTTON, "Failed", "Decode", error_flash_color(), true);
                reset_later(self, sel!(resetDecodeLabel:));
                return;
            };
            apply_preview(&clipboard::formatted(&body));
            select_mode(ViewMode::Decode);
        }

        #[unsafe(method(resetDecodeLabel:))]
        fn reset_decode_label(&self, _sender: Option<&AnyObject>) {
            paint_mode_buttons();
        }

        #[unsafe(method(compressClicked:))]
        fn compress_clicked(&self, _sender: Option<&AnyObject>) {
            let body = SOURCE_TEXT.with(|src| compress::try_compress(&src.borrow()));
            let Some(body) = body else {
                flash_button(&COMPRESS_BUTTON, "Failed", "Compress", error_flash_color(), true);
                reset_later(self, sel!(resetCompressLabel:));
                return;
            };
            apply_preview(&body);
            select_mode(ViewMode::Compress);
        }

        #[unsafe(method(resetCompressLabel:))]
        fn reset_compress_label(&self, _sender: Option<&AnyObject>) {
            paint_mode_buttons();
        }

        #[unsafe(method(redactClicked:))]
        fn redact_clicked(&self, _sender: Option<&AnyObject>) {
            let body = SOURCE_TEXT.with(|src| redact::redact(&src.borrow()));
            apply_preview(&body);
            select_mode(ViewMode::Redact);
        }

        #[unsafe(method(dataframeClicked:))]
        fn dataframe_clicked(&self, _sender: Option<&AnyObject>) {
            let body = SOURCE_TEXT.with(|src| dataframe::try_format(&src.borrow()));
            let Some(body) = body else {
                flash_button(&DATAFRAME_BUTTON, "Failed", "Dataframe", error_flash_color(), true);
                reset_later(self, sel!(resetDataframeLabel:));
                return;
            };
            apply_preview_with_kind(&body, FormatKind::Dataframe);
            select_mode(ViewMode::Dataframe);
        }

        #[unsafe(method(resetDataframeLabel:))]
        fn reset_dataframe_label(&self, _sender: Option<&AnyObject>) {
            paint_mode_buttons();
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

        #[unsafe(method(fadePreviewIn:))]
        fn fade_preview_in(&self, _sender: Option<&AnyObject>) {
            reveal_preview_body();
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
fn convert_flash_color() -> Retained<NSColor> {
    NSColor::colorWithCalibratedRed_green_blue_alpha(1.0, 0.55, 0.70, 1.0)
}
fn decode_flash_color() -> Retained<NSColor> {
    NSColor::colorWithCalibratedRed_green_blue_alpha(0.62, 0.55, 1.0, 1.0)
}
fn original_flash_color() -> Retained<NSColor> {
    NSColor::colorWithCalibratedRed_green_blue_alpha(0.35, 0.78, 0.98, 1.0)
}
fn compress_flash_color() -> Retained<NSColor> {
    NSColor::colorWithCalibratedRed_green_blue_alpha(0.42, 0.88, 0.72, 1.0)
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
