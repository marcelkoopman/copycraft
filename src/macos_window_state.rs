thread_local! {
    static WINDOW: RefCell<Option<Retained<NSWindow>>> = const { RefCell::new(None) };
    static TARGET: RefCell<Option<Retained<PreviewTarget>>> = const { RefCell::new(None) };
    static TEXT: RefCell<Option<Retained<NSTextView>>> = const { RefCell::new(None) };
    static SCROLL: RefCell<Option<Retained<NSScrollView>>> = const { RefCell::new(None) };
    static IMAGE_VIEW: RefCell<Option<Retained<NSImageView>>> = const { RefCell::new(None) };
    static COPY_BUTTON: RefCell<Option<Retained<NSButton>>> = const { RefCell::new(None) };
    static ORIGINAL_BUTTON: RefCell<Option<Retained<NSButton>>> = const { RefCell::new(None) };
    static FORMAT_BUTTON: RefCell<Option<Retained<NSButton>>> = const { RefCell::new(None) };
    static CONVERT_BUTTON: RefCell<Option<Retained<NSButton>>> = const { RefCell::new(None) };
    static DECODE_BUTTON: RefCell<Option<Retained<NSButton>>> = const { RefCell::new(None) };
    static COMPRESS_BUTTON: RefCell<Option<Retained<NSButton>>> = const { RefCell::new(None) };
    static REDACT_BUTTON: RefCell<Option<Retained<NSButton>>> = const { RefCell::new(None) };
    static DATAFRAME_BUTTON: RefCell<Option<Retained<NSButton>>> = const { RefCell::new(None) };
    static INFO_BUTTON: RefCell<Option<Retained<NSButton>>> = const { RefCell::new(None) };
    static OCR_BUTTON: RefCell<Option<Retained<NSButton>>> = const { RefCell::new(None) };
    static QR_BUTTON: RefCell<Option<Retained<NSButton>>> = const { RefCell::new(None) };
    static SAVE_BUTTON: RefCell<Option<Retained<NSButton>>> = const { RefCell::new(None) };
    static SOURCE_TEXT: RefCell<String> = const { RefCell::new(String::new()) };
    static SOURCE_IMAGE: RefCell<Option<ClipboardImage>> = const { RefCell::new(None) };
    static IMAGE_JPEG: RefCell<Option<Vec<u8>>> = const { RefCell::new(None) };
    static IMAGE_OCR: RefCell<Option<String>> = const { RefCell::new(None) };
    static IMAGE_QR: RefCell<Option<String>> = const { RefCell::new(None) };
    static IMAGE_INFO: RefCell<Option<String>> = const { RefCell::new(None) };
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
            let copied = if showing_image_pixels() {
                copy_image_pixels()
            } else {
                PREVIEW_TEXT.with(|text| clipboard::write_clipboard(&text.borrow()))
            };
            let ok = copied.is_ok();
            flash_button(
                &COPY_BUTTON,
                "Copied  \u{2713}",
                "Copy",
                if ok {
                    copy_flash_color()
                } else {
                    error_flash_color()
                },
                ok,
            );
            reset_later(self, sel!(resetCopyLabel:));
        }

        #[unsafe(method(resetCopyLabel:))]
        fn reset_copy_label(&self, _sender: Option<&AnyObject>) {
            flash_button(&COPY_BUTTON, "Copied  \u{2713}", "Copy", copy_flash_color(), false);
        }

        #[unsafe(method(originalClicked:))]
        fn original_clicked(&self, _sender: Option<&AnyObject>) {
            select_mode(ViewMode::Original);
            if source_is_image() {
                apply_image_preview();
                return;
            }
            let body = SOURCE_TEXT.with(|src| src.borrow().clone());
            apply_preview(&body);
        }

        #[unsafe(method(formatClicked:))]
        fn format_clicked(&self, _sender: Option<&AnyObject>) {
            let body = SOURCE_TEXT.with(|src| clipboard::formatted(&src.borrow()));
            select_mode(ViewMode::Format);
            apply_preview(&body);
        }

        #[unsafe(method(convertClicked:))]
        fn convert_clicked(&self, _sender: Option<&AnyObject>) {
            let body = SOURCE_TEXT.with(|src| convert::try_convert(&src.borrow()));
            let Some(body) = body else {
                flash_button(&CONVERT_BUTTON, "Failed", "Convert", error_flash_color(), true);
                reset_later(self, sel!(resetConvertLabel:));
                return;
            };
            select_mode(ViewMode::Convert);
            apply_preview(&clipboard::formatted(&body));
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
            select_mode(ViewMode::Decode);
            apply_preview(&clipboard::formatted(&body));
        }

        #[unsafe(method(resetDecodeLabel:))]
        fn reset_decode_label(&self, _sender: Option<&AnyObject>) {
            paint_mode_buttons();
        }

        #[unsafe(method(compressClicked:))]
        fn compress_clicked(&self, _sender: Option<&AnyObject>) {
            if source_is_image() {
                if IMAGE_JPEG.with(|slot| slot.borrow().is_none()) {
                    flash_button(&COMPRESS_BUTTON, "Failed", "Compress", error_flash_color(), true);
                    reset_later(self, sel!(resetCompressLabel:));
                    return;
                }
                select_mode(ViewMode::Compress);
                apply_image_preview();
                return;
            }
            let body = SOURCE_TEXT.with(|src| compress::try_compress(&src.borrow()));
            let Some(body) = body else {
                flash_button(&COMPRESS_BUTTON, "Failed", "Compress", error_flash_color(), true);
                reset_later(self, sel!(resetCompressLabel:));
                return;
            };
            select_mode(ViewMode::Compress);
            apply_preview(&body);
        }

        #[unsafe(method(infoClicked:))]
        fn info_clicked(&self, _sender: Option<&AnyObject>) {
            let body = IMAGE_INFO.with(|slot| slot.borrow().clone()).or_else(|| {
                SOURCE_IMAGE.with(|slot| slot.borrow().as_ref().map(image_ops::info_dimensions))
            });
            let Some(body) = body else {
                flash_button(&INFO_BUTTON, "Failed", "Info", error_flash_color(), true);
                reset_later(self, sel!(resetInfoLabel:));
                return;
            };
            select_mode(ViewMode::Info);
            apply_preview_with_kind(&body, FormatKind::Image);
        }

        #[unsafe(method(resetInfoLabel:))]
        fn reset_info_label(&self, _sender: Option<&AnyObject>) {
            paint_mode_buttons();
        }

        #[unsafe(method(ocrClicked:))]
        fn ocr_clicked(&self, _sender: Option<&AnyObject>) {
            let body = IMAGE_OCR.with(|slot| slot.borrow().clone());
            let Some(body) = body else {
                flash_button(&OCR_BUTTON, "Failed", "Text", error_flash_color(), true);
                reset_later(self, sel!(resetOcrLabel:));
                return;
            };
            select_mode(ViewMode::Ocr);
            apply_preview_with_kind(&body, FormatKind::Text);
        }

        #[unsafe(method(resetOcrLabel:))]
        fn reset_ocr_label(&self, _sender: Option<&AnyObject>) {
            paint_mode_buttons();
        }

        #[unsafe(method(qrClicked:))]
        fn qr_clicked(&self, _sender: Option<&AnyObject>) {
            let body = IMAGE_QR.with(|slot| slot.borrow().clone());
            let Some(body) = body else {
                flash_button(&QR_BUTTON, "Failed", "QR", error_flash_color(), true);
                reset_later(self, sel!(resetQrLabel:));
                return;
            };
            select_mode(ViewMode::Qr);
            apply_preview_with_kind(&body, FormatKind::Url);
        }

        #[unsafe(method(resetQrLabel:))]
        fn reset_qr_label(&self, _sender: Option<&AnyObject>) {
            paint_mode_buttons();
        }

        #[unsafe(method(resetCompressLabel:))]
        fn reset_compress_label(&self, _sender: Option<&AnyObject>) {
            paint_mode_buttons();
        }

        #[unsafe(method(redactClicked:))]
        fn redact_clicked(&self, _sender: Option<&AnyObject>) {
            let body = SOURCE_TEXT.with(|src| redact::redact(&src.borrow()));
            select_mode(ViewMode::Redact);
            apply_preview(&body);
        }

        #[unsafe(method(dataframeClicked:))]
        fn dataframe_clicked(&self, _sender: Option<&AnyObject>) {
            let body = SOURCE_TEXT.with(|src| dataframe::try_format(&src.borrow()));
            let Some(body) = body else {
                flash_button(&DATAFRAME_BUTTON, "Failed", "Dataframe", error_flash_color(), true);
                reset_later(self, sel!(resetDataframeLabel:));
                return;
            };
            select_mode(ViewMode::Dataframe);
            apply_preview_with_kind(&body, FormatKind::Dataframe);
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
fn info_flash_color() -> Retained<NSColor> {
    NSColor::colorWithCalibratedRed_green_blue_alpha(0.96, 0.77, 0.26, 1.0)
}
fn ocr_flash_color() -> Retained<NSColor> {
    NSColor::colorWithCalibratedRed_green_blue_alpha(0.62, 0.55, 1.0, 1.0)
}
fn qr_flash_color() -> Retained<NSColor> {
    NSColor::colorWithCalibratedRed_green_blue_alpha(1.0, 0.55, 0.70, 1.0)
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
