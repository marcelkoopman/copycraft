pub fn show(formatted: &str, kind: FormatKind) -> Result<(), String> {
    let mtm = MainThreadMarker::new().ok_or("preview must run on the main thread")?;
    let title = kind.preview_heading();
    let body = formatted.to_string();
    SOURCE_TEXT.with(|slot| slot.replace(body.clone()));
    PREVIEW_TEXT.with(|slot| slot.replace(body.clone()));
    PREVIEW_KIND.with(|slot| slot.replace(kind));
    SOURCE_KIND.with(|slot| slot.replace(kind));
    VIEW_MODE.with(|slot| slot.replace(ViewMode::Format));

    let app = NSApplication::sharedApplication(mtm);
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);

    let reused = WINDOW.with(|slot| slot.borrow().is_some());
    if reused {
        WINDOW.with(|slot| {
            if let Some(window) = slot.borrow().as_ref() {
                window.setTitle(&NSString::from_str(title));
                window.setAlphaValue(0.86);
                window.makeKeyAndOrderFront(None);
                window.orderFrontRegardless();
            }
        });
        TEXT.with(|slot| {
            if let Some(text) = slot.borrow().as_ref() {
                set_body(text, &body, kind);
            }
        });
        SCROLL.with(|slot| {
            if let Some(scroll) = slot.borrow().as_ref() {
                scroll.setWantsLayer(true);
                scroll.setAlphaValue(1.0);
            }
        });
        apply_toolbar_for_kind(kind);
        flash_button(&COPY_BUTTON, "Copied  \u{2713}", "Copy", copy_flash_color(), false);
        flash_button(&SAVE_BUTTON, "Saved  \u{2713}", "Save", save_flash_color(), false);
        return Ok(());
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
    window.setAlphaValue(0.86);

    let bounds = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, height));
    let frosted = NSVisualEffectView::initWithFrame(NSVisualEffectView::alloc(mtm), bounds);
    frosted.setMaterial(NSVisualEffectMaterial::UnderWindowBackground);
    frosted.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
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

    let scroll = NSScrollView::initWithFrame(
        NSScrollView::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, toolbar_h), NSSize::new(width, height - toolbar_h)),
    );
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
    set_body(&text, &body, kind);
    scroll.setDocumentView(Some(&text));

    frosted.addSubview(&scroll);
    frosted.addSubview(&original_button);
    frosted.addSubview(&format_button);
    frosted.addSubview(&decode_button);
    frosted.addSubview(&compress_button);
    frosted.addSubview(&redact_button);
    frosted.addSubview(&dataframe_button);
    frosted.addSubview(&save_button);
    frosted.addSubview(&copy_button);
    window.setContentView(Some(&frosted));

    window.center();
    window.makeKeyAndOrderFront(None);
    window.orderFrontRegardless();

    TARGET.with(|slot| slot.replace(Some(target)));
    TEXT.with(|slot| slot.replace(Some(text)));
    SCROLL.with(|slot| slot.replace(Some(scroll)));
    ORIGINAL_BUTTON.with(|slot| slot.replace(Some(original_button)));
    FORMAT_BUTTON.with(|slot| slot.replace(Some(format_button)));
    DECODE_BUTTON.with(|slot| slot.replace(Some(decode_button)));
    COMPRESS_BUTTON.with(|slot| slot.replace(Some(compress_button)));
    REDACT_BUTTON.with(|slot| slot.replace(Some(redact_button)));
    DATAFRAME_BUTTON.with(|slot| slot.replace(Some(dataframe_button)));
    SAVE_BUTTON.with(|slot| slot.replace(Some(save_button)));
    COPY_BUTTON.with(|slot| slot.replace(Some(copy_button)));
    WINDOW.with(|slot| slot.replace(Some(window)));
    apply_toolbar_for_kind(kind);
    flash_button(&COPY_BUTTON, "Copied  \u{2713}", "Copy", copy_flash_color(), false);
    flash_button(&SAVE_BUTTON, "Saved  \u{2713}", "Save", save_flash_color(), false);
    Ok(())
}
