pub fn show(source: &str, kind: FormatKind) -> Result<(), String> {
    let mtm = MainThreadMarker::new().ok_or("preview must run on the main thread")?;
    let source = source.to_string();
    let body = source.clone();
    let mode = ViewMode::Original;
    SOURCE_IMAGE.with(|slot| slot.replace(None));
    clear_image_actions();
    SOURCE_TEXT.with(|slot| slot.replace(source));
    PREVIEW_TEXT.with(|slot| slot.replace(body.clone()));
    PREVIEW_KIND.with(|slot| slot.replace(kind));
    SOURCE_KIND.with(|slot| slot.replace(kind));
    VIEW_MODE.with(|slot| slot.replace(mode));
    let title = window_title(kind, mode);
    activate_app(mtm);
    ensure_preview_window(mtm, &title);
    present_text_body();
    SCROLL.with(|slot| {
        if let Some(scroll) = slot.borrow().as_ref() {
            scroll.setWantsLayer(true);
            scroll.setAlphaValue(1.0);
        }
    });
    apply_toolbar_for_kind(kind);
    flash_button(&COPY_BUTTON, "Copied  \u{2713}", "Copy", copy_flash_color(), false);
    flash_button(&SAVE_BUTTON, "Saved  \u{2713}", "Save", save_flash_color(), false);
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn show_image(image: &ClipboardImage) -> Result<(), String> {
    begin_image_window()?;
    install_preview_image(image.clone(), None, 0);
    Ok(())
}

pub fn show_clipboard_image() -> Result<(), String> {
    begin_image_window()?;
    let scan_id = IMAGE_SCAN_GEN.load(Ordering::SeqCst);
    std::thread::spawn(move || {
        let decoded = crate::macos_pasteboard::decode_preview();
        if decoded.is_none() {
            eprintln!("image preview failed");
        }
        DispatchQueue::main().exec_async(move || {
            deliver_clipboard_preview(scan_id, decoded);
        });
    });
    Ok(())
}

fn begin_image_window() -> Result<(), String> {
    let mtm = MainThreadMarker::new().ok_or("preview must run on the main thread")?;
    let mode = ViewMode::Original;
    let kind = FormatKind::Image;
    SOURCE_IMAGE.with(|slot| slot.replace(None));
    clear_image_actions();
    SOURCE_TEXT.with(|slot| slot.replace(String::new()));
    PREVIEW_TEXT.with(|slot| slot.replace(String::new()));
    PREVIEW_KIND.with(|slot| slot.replace(kind));
    SOURCE_KIND.with(|slot| slot.replace(kind));
    VIEW_MODE.with(|slot| slot.replace(mode));
    activate_app(mtm);
    ensure_preview_window(mtm, "Image");
    present_image_body();
    IMAGE_VIEW.with(|slot| {
        if let Some(view) = slot.borrow().as_ref() {
            view.setWantsLayer(true);
            view.setAlphaValue(1.0);
        }
    });
    apply_toolbar_for_kind(kind);
    flash_button(&COPY_BUTTON, "Copied  \u{2713}", "Copy", copy_flash_color(), false);
    flash_button(&SAVE_BUTTON, "Saved  \u{2713}", "Save", save_flash_color(), false);
    Ok(())
}

const WINDOW_ALPHA: f64 = 0.94;

fn activate_app(mtm: MainThreadMarker) {
    let app = NSApplication::sharedApplication(mtm);
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);
}

fn ensure_preview_window(mtm: MainThreadMarker, title: &str) {
    let reused = WINDOW.with(|slot| slot.borrow().is_some());
    if reused {
        WINDOW.with(|slot| {
            if let Some(window) = slot.borrow().as_ref() {
                window.setTitle(&NSString::from_str(title));
                window.setAlphaValue(WINDOW_ALPHA);
                window.makeKeyAndOrderFront(None);
                window.orderFrontRegardless();
            }
        });
        return;
    }

    let width = 780.0;
    let height = 520.0;
    let toolbar_h = 36.0;
    let frame = NSRect::new(NSPoint::new(240.0, 180.0), NSSize::new(width, height));
    let style = NSWindowStyleMask::Titled
        | NSWindowStyleMask::Closable
        | NSWindowStyleMask::Resizable
        | NSWindowStyleMask::Miniaturizable;
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            frame,
            style,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    unsafe { window.setReleasedWhenClosed(false) };
    window.setTitle(&NSString::from_str(title));
    window.setTitlebarAppearsTransparent(false);
    window.setTitleVisibility(NSWindowTitleVisibility::Visible);
    window.setOpaque(false);
    window.setHasShadow(true);
    window.setBackgroundColor(Some(&NSColor::clearColor()));
    window.setAlphaValue(WINDOW_ALPHA);

    let bounds = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, height));
    let frosted = NSVisualEffectView::initWithFrame(NSVisualEffectView::alloc(mtm), bounds);
    frosted.setMaterial(NSVisualEffectMaterial::WindowBackground);
    frosted.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
    frosted.setEmphasized(true);
    frosted.setState(NSVisualEffectState::Active);
    frosted.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );

    let target = PreviewTarget::new(mtm);
    let button_h = 24.0;
    let button_w = 84.0;
    let pad = 10.0;
    let button_y = (toolbar_h - button_h) / 2.0;
    let original_x = pad;
    let format_x = original_x + button_w + 6.0;
    let copy_x = width - pad - button_w;
    let save_x = copy_x - 6.0 - button_w;

    let original_button = make_toolbar_button(
        mtm,
        NSRect::new(NSPoint::new(original_x, button_y), NSSize::new(button_w, button_h)),
        &target,
        sel!(originalClicked:),
        false,
    );
    style_title_button(&original_button, "Original", &idle_button_color());
    let format_button = make_toolbar_button(
        mtm,
        NSRect::new(NSPoint::new(format_x, button_y), NSSize::new(button_w, button_h)),
        &target,
        sel!(formatClicked:),
        false,
    );
    style_title_button(&format_button, "Format", &format_flash_color());
    let convert_button = make_toolbar_button(
        mtm,
        NSRect::new(NSPoint::new(format_x, button_y), NSSize::new(button_w, button_h)),
        &target,
        sel!(convertClicked:),
        false,
    );
    style_title_button(&convert_button, "Convert", &idle_button_color());
    let decode_button = make_toolbar_button(
        mtm,
        NSRect::new(NSPoint::new(format_x, button_y), NSSize::new(button_w, button_h)),
        &target,
        sel!(decodeClicked:),
        false,
    );
    style_title_button(&decode_button, "Decode", &idle_button_color());
    let compress_button = make_toolbar_button(
        mtm,
        NSRect::new(NSPoint::new(format_x, button_y), NSSize::new(button_w, button_h)),
        &target,
        sel!(compressClicked:),
        false,
    );
    style_title_button(&compress_button, "Compress", &idle_button_color());
    let redact_button = make_toolbar_button(
        mtm,
        NSRect::new(NSPoint::new(format_x, button_y), NSSize::new(button_w, button_h)),
        &target,
        sel!(redactClicked:),
        false,
    );
    style_title_button(&redact_button, "Redact", &idle_button_color());
    let dataframe_button = make_toolbar_button(
        mtm,
        NSRect::new(NSPoint::new(format_x, button_y), NSSize::new(button_w, button_h)),
        &target,
        sel!(dataframeClicked:),
        false,
    );
    style_title_button(&dataframe_button, "Dataframe", &idle_button_color());
    let info_button = make_toolbar_button(
        mtm,
        NSRect::new(NSPoint::new(format_x, button_y), NSSize::new(button_w, button_h)),
        &target,
        sel!(infoClicked:),
        false,
    );
    style_title_button(&info_button, "Info", &idle_button_color());
    let ocr_button = make_toolbar_button(
        mtm,
        NSRect::new(NSPoint::new(format_x, button_y), NSSize::new(button_w, button_h)),
        &target,
        sel!(ocrClicked:),
        false,
    );
    style_title_button(&ocr_button, "Text", &idle_button_color());
    let qr_button = make_toolbar_button(
        mtm,
        NSRect::new(NSPoint::new(format_x, button_y), NSSize::new(button_w, button_h)),
        &target,
        sel!(qrClicked:),
        false,
    );
    style_title_button(&qr_button, "QR", &idle_button_color());
    let save_button = make_toolbar_button(
        mtm,
        NSRect::new(NSPoint::new(save_x, button_y), NSSize::new(button_w, button_h)),
        &target,
        sel!(saveClicked:),
        true,
    );
    style_title_button(&save_button, "Save", &idle_button_color());
    let copy_button = make_toolbar_button(
        mtm,
        NSRect::new(NSPoint::new(copy_x, button_y), NSSize::new(button_w, button_h)),
        &target,
        sel!(copyClicked:),
        true,
    );

    let content_frame = NSRect::new(
        NSPoint::new(0.0, toolbar_h),
        NSSize::new(width, height - toolbar_h),
    );
    let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), content_frame);
    scroll.setHasVerticalScroller(true);
    scroll.setHasHorizontalScroller(true);
    scroll.setAutohidesScrollers(false);
    scroll.setDrawsBackground(false);
    scroll.setBackgroundColor(&NSColor::clearColor());
    scroll.setWantsLayer(true);
    scroll.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );

    let text = NSTextView::initWithFrame(
        NSTextView::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, height - toolbar_h)),
    );
    text.setEditable(false);
    text.setSelectable(true);
    text.setDrawsBackground(false);
    text.setBackgroundColor(&NSColor::clearColor());
    text.setTextContainerInset(NSSize::new(10.0, 12.0));
    text.setFont(Some(&editor_font()));
    configure_scrolling_text(&text);
    scroll.setDocumentView(Some(&text));

    let image_view = NSImageView::initWithFrame(NSImageView::alloc(mtm), content_frame);
    image_view.setEditable(false);
    image_view.setImageScaling(NSImageScaling::ScaleProportionallyUpOrDown);
    image_view.setImageAlignment(NSImageAlignment::AlignCenter);
    image_view.setWantsLayer(true);
    image_view.setHidden(true);
    image_view.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );

    frosted.addSubview(&scroll);
    frosted.addSubview(&image_view);
    frosted.addSubview(&original_button);
    frosted.addSubview(&format_button);
    frosted.addSubview(&convert_button);
    frosted.addSubview(&decode_button);
    frosted.addSubview(&compress_button);
    frosted.addSubview(&redact_button);
    frosted.addSubview(&dataframe_button);
    frosted.addSubview(&info_button);
    frosted.addSubview(&ocr_button);
    frosted.addSubview(&qr_button);
    frosted.addSubview(&save_button);
    frosted.addSubview(&copy_button);
    window.setContentView(Some(&frosted));

    window.center();
    window.makeKeyAndOrderFront(None);
    window.orderFrontRegardless();

    TARGET.with(|slot| slot.replace(Some(target)));
    TEXT.with(|slot| slot.replace(Some(text)));
    SCROLL.with(|slot| slot.replace(Some(scroll)));
    IMAGE_VIEW.with(|slot| slot.replace(Some(image_view)));
    ORIGINAL_BUTTON.with(|slot| slot.replace(Some(original_button)));
    FORMAT_BUTTON.with(|slot| slot.replace(Some(format_button)));
    CONVERT_BUTTON.with(|slot| slot.replace(Some(convert_button)));
    DECODE_BUTTON.with(|slot| slot.replace(Some(decode_button)));
    COMPRESS_BUTTON.with(|slot| slot.replace(Some(compress_button)));
    REDACT_BUTTON.with(|slot| slot.replace(Some(redact_button)));
    DATAFRAME_BUTTON.with(|slot| slot.replace(Some(dataframe_button)));
    INFO_BUTTON.with(|slot| slot.replace(Some(info_button)));
    OCR_BUTTON.with(|slot| slot.replace(Some(ocr_button)));
    QR_BUTTON.with(|slot| slot.replace(Some(qr_button)));
    SAVE_BUTTON.with(|slot| slot.replace(Some(save_button)));
    COPY_BUTTON.with(|slot| slot.replace(Some(copy_button)));
    WINDOW.with(|slot| slot.replace(Some(window)));
}
