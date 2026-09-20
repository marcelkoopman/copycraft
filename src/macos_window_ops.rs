fn apply_preview(body: &str) {
    apply_preview_with_kind(body, format::detect(body));
}

fn window_title(kind: FormatKind, mode: ViewMode) -> String {
    if kind == FormatKind::Image {
        if mode == ViewMode::Convert {
            return "Data URI".to_string();
        }
        return SOURCE_IMAGE.with(|slot| {
            slot.borrow()
                .as_ref()
                .map(|image| format!("Image {}×{}", image.width, image.height))
                .unwrap_or_else(|| "Image".to_string())
        });
    }
    if mode == ViewMode::Format {
        kind.preview_heading()
    } else {
        kind.source_heading()
    }
    .to_string()
}

fn apply_image_preview() {
    PREVIEW_KIND.with(|slot| slot.replace(FormatKind::Image));
    PREVIEW_TEXT.with(|slot| slot.replace(String::new()));
    let mode = VIEW_MODE.with(|slot| *slot.borrow());
    WINDOW.with(|slot| {
        if let Some(window) = slot.borrow().as_ref() {
            window.setTitle(&NSString::from_str(&window_title(FormatKind::Image, mode)));
        }
    });
    fade_visible_surface(0.0, 0.16);
    schedule_fade_in();
    flash_button(&COPY_BUTTON, "Copied  \u{2713}", "Copy", copy_flash_color(), false);
}

fn apply_preview_with_kind(body: &str, kind: FormatKind) {
    PREVIEW_KIND.with(|slot| slot.replace(kind));
    PREVIEW_TEXT.with(|slot| slot.replace(body.to_string()));
    let mode = VIEW_MODE.with(|slot| *slot.borrow());
    WINDOW.with(|slot| {
        if let Some(window) = slot.borrow().as_ref() {
            window.setTitle(&NSString::from_str(&window_title(kind, mode)));
        }
    });
    fade_visible_surface(0.0, 0.16);
    schedule_fade_in();
    flash_button(&COPY_BUTTON, "Copied  \u{2713}", "Copy", copy_flash_color(), false);
}

fn schedule_fade_in() {
    TARGET.with(|slot| {
        if let Some(target) = slot.borrow().as_ref() {
            unsafe {
                let _: () = msg_send![
                    &**target,
                    performSelector: sel!(fadePreviewIn:),
                    withObject: None::<&AnyObject>,
                    afterDelay: 0.16
                ];
            }
        }
    });
}

fn source_is_image() -> bool {
    SOURCE_KIND.with(|slot| *slot.borrow() == FormatKind::Image)
        && SOURCE_IMAGE.with(|slot| slot.borrow().is_some())
}

fn showing_original_image() -> bool {
    VIEW_MODE.with(|slot| *slot.borrow() == ViewMode::Original) && source_is_image()
}

fn reveal_preview_body() {
    if showing_original_image() {
        present_image_body();
        fade_image(1.0, 0.28);
        return;
    }
    present_text_body();
    fade_scroll(1.0, 0.28);
}

fn present_text_body() {
    set_image_hidden(true);
    set_view_hidden(&SCROLL, false);
    let kind = PREVIEW_KIND.with(|slot| *slot.borrow());
    let body = PREVIEW_TEXT.with(|slot| slot.borrow().clone());
    TEXT.with(|slot| {
        if let Some(text) = slot.borrow().as_ref() {
            set_body(text, &body, kind);
        }
    });
}

fn present_image_body() {
    set_view_hidden(&SCROLL, true);
    set_image_hidden(false);
    let nsimage = SOURCE_IMAGE.with(|slot| {
        slot.borrow()
            .as_ref()
            .and_then(nsimage_from_clipboard)
    });
    IMAGE_VIEW.with(|slot| {
        if let Some(view) = slot.borrow().as_ref() {
            view.setImage(nsimage.as_deref());
        }
    });
}

fn fade_visible_surface(alpha: f64, duration: f64) {
    fade_scroll(alpha, duration);
    fade_image(alpha, duration);
}

fn fade_scroll(alpha: f64, duration: f64) {
    SCROLL.with(|slot| {
        let borrowed = slot.borrow();
        let Some(scroll) = borrowed.as_ref() else {
            return;
        };
        scroll.setWantsLayer(true);
        NSAnimationContext::beginGrouping();
        NSAnimationContext::currentContext().setDuration(duration);
        scroll.animator().setAlphaValue(alpha);
        NSAnimationContext::endGrouping();
    });
}

fn fade_image(alpha: f64, duration: f64) {
    IMAGE_VIEW.with(|slot| {
        let borrowed = slot.borrow();
        let Some(view) = borrowed.as_ref() else {
            return;
        };
        view.setWantsLayer(true);
        NSAnimationContext::beginGrouping();
        NSAnimationContext::currentContext().setDuration(duration);
        view.animator().setAlphaValue(alpha);
        NSAnimationContext::endGrouping();
    });
}

fn set_view_hidden(
    slot: &'static std::thread::LocalKey<RefCell<Option<Retained<NSScrollView>>>>,
    hidden: bool,
) {
    slot.with(|cell| {
        if let Some(view) = cell.borrow().as_ref() {
            view.setHidden(hidden);
        }
    });
}

fn set_image_hidden(hidden: bool) {
    IMAGE_VIEW.with(|cell| {
        if let Some(view) = cell.borrow().as_ref() {
            view.setHidden(hidden);
        }
    });
}

fn select_mode(mode: ViewMode) {
    VIEW_MODE.with(|slot| slot.replace(mode));
    paint_mode_buttons();
}

fn paint_mode_buttons() {
    let mode = VIEW_MODE.with(|slot| *slot.borrow());
    paint_mode_button(
        &ORIGINAL_BUTTON,
        "Original",
        original_flash_color(),
        mode == ViewMode::Original,
    );
    paint_mode_button(
        &FORMAT_BUTTON,
        "Format",
        format_flash_color(),
        mode == ViewMode::Format,
    );
    paint_mode_button(
        &CONVERT_BUTTON,
        "Convert",
        convert_flash_color(),
        mode == ViewMode::Convert,
    );
    paint_mode_button(
        &DECODE_BUTTON,
        "Decode",
        decode_flash_color(),
        mode == ViewMode::Decode,
    );
    paint_mode_button(
        &COMPRESS_BUTTON,
        "Compress",
        compress_flash_color(),
        mode == ViewMode::Compress,
    );
    paint_mode_button(
        &REDACT_BUTTON,
        "Redact",
        redact_flash_color(),
        mode == ViewMode::Redact,
    );
    paint_mode_button(
        &DATAFRAME_BUTTON,
        "Dataframe",
        dataframe_flash_color(),
        mode == ViewMode::Dataframe,
    );
}

fn paint_mode_button(
    slot: &'static std::thread::LocalKey<RefCell<Option<Retained<NSButton>>>>,
    label: &str,
    selected: Retained<NSColor>,
    on: bool,
) {
    slot.with(|cell| {
        let borrowed = cell.borrow();
        let Some(button) = borrowed.as_ref() else {
            return;
        };
        let color = if on { selected } else { idle_button_color() };
        style_title_button(button, label, &color);
    });
}

const TOOLBAR_PAD: f64 = 10.0;
const TOOLBAR_GAP: f64 = 6.0;
const TOOLBAR_BTN_W: f64 = 84.0;
const TOOLBAR_BTN_H: f64 = 24.0;
const TOOLBAR_H: f64 = 36.0;

fn apply_toolbar_for_kind(kind: FormatKind) {
    let source = SOURCE_TEXT.with(|slot| slot.borrow().clone());
    let show_format = kind != FormatKind::Image && toolbar_visibility::shows_format(&source);
    let show_convert =
        kind == FormatKind::Image || toolbar_visibility::shows_convert(&source);
    let show_redact = toolbar_visibility::shows_redact(kind, &source);
    let show_df =
        toolbar_visibility::shows_dataframe(kind) || dataframe::try_format(&source).is_some();
    let show_compress =
        toolbar_visibility::shows_compress(kind) && compress::is_large_enough(&source);
    let show_decode =
        toolbar_visibility::shows_decode(kind) && decode::try_decode(&source).is_some();
    if !show_format {
        VIEW_MODE.with(|slot| {
            if *slot.borrow() == ViewMode::Format {
                slot.replace(ViewMode::Original);
            }
        });
    }
    if !show_convert {
        VIEW_MODE.with(|slot| {
            if *slot.borrow() == ViewMode::Convert {
                slot.replace(ViewMode::Original);
            }
        });
    }
    let show_original = show_format
        || show_convert
        || show_decode
        || show_compress
        || show_redact
        || show_df;
    set_button_hidden(&ORIGINAL_BUTTON, !show_original);
    set_button_hidden(&FORMAT_BUTTON, !show_format);
    set_button_hidden(&CONVERT_BUTTON, !show_convert);
    set_button_hidden(&REDACT_BUTTON, !show_redact);
    set_button_hidden(&DATAFRAME_BUTTON, !show_df);
    set_button_hidden(&COMPRESS_BUTTON, !show_compress);
    set_button_hidden(&DECODE_BUTTON, !show_decode);

    let y = (TOOLBAR_H - TOOLBAR_BTN_H) / 2.0;
    let mut x = TOOLBAR_PAD;
    if show_original {
        place_button(&ORIGINAL_BUTTON, x, y);
        x += TOOLBAR_BTN_W + TOOLBAR_GAP;
    }
    if show_format {
        place_button(&FORMAT_BUTTON, x, y);
        x += TOOLBAR_BTN_W + TOOLBAR_GAP;
    }
    if show_convert {
        place_button(&CONVERT_BUTTON, x, y);
        x += TOOLBAR_BTN_W + TOOLBAR_GAP;
    }
    if show_decode {
        place_button(&DECODE_BUTTON, x, y);
        x += TOOLBAR_BTN_W + TOOLBAR_GAP;
    }
    if show_compress {
        place_button(&COMPRESS_BUTTON, x, y);
        x += TOOLBAR_BTN_W + TOOLBAR_GAP;
    }
    if show_redact {
        place_button(&REDACT_BUTTON, x, y);
        x += TOOLBAR_BTN_W + TOOLBAR_GAP;
    }
    if show_df {
        place_button(&DATAFRAME_BUTTON, x, y);
    }
    paint_mode_buttons();
}

fn set_button_hidden(
    slot: &'static std::thread::LocalKey<RefCell<Option<Retained<NSButton>>>>,
    hidden: bool,
) {
    slot.with(|cell| {
        if let Some(button) = cell.borrow().as_ref() {
            button.setHidden(hidden);
        }
    });
}

fn place_button(
    slot: &'static std::thread::LocalKey<RefCell<Option<Retained<NSButton>>>>,
    x: f64,
    y: f64,
) {
    slot.with(|cell| {
        if let Some(button) = cell.borrow().as_ref() {
            let mut frame = button.frame();
            frame.origin = NSPoint::new(x, y);
            button.setFrame(frame);
        }
    });
}

fn make_toolbar_button(
    mtm: MainThreadMarker,
    frame: NSRect,
    target: &PreviewTarget,
    action: objc2::runtime::Sel,
    stick_right: bool,
) -> Retained<NSButton> {
    let button = NSButton::initWithFrame(NSButton::alloc(mtm), frame);
    button.setBordered(true);
    button.setEnabled(true);
    let mask = if stick_right {
        NSAutoresizingMaskOptions::ViewMinXMargin | NSAutoresizingMaskOptions::ViewMaxYMargin
    } else {
        NSAutoresizingMaskOptions::ViewMaxXMargin | NSAutoresizingMaskOptions::ViewMaxYMargin
    };
    button.setAutoresizingMask(mask);
    unsafe {
        button.setTarget(Some(target));
        button.setAction(Some(action));
    }
    button
}

fn save_payload(kind: FormatKind) -> String {
    if kind == FormatKind::Dataframe {
        let source = SOURCE_TEXT.with(|slot| slot.borrow().clone());
        if let Some(csv) = dataframe::try_csv_text(&source) {
            return csv;
        }
    }
    PREVIEW_TEXT.with(|slot| slot.borrow().clone())
}

fn save_preview_to_file() -> bool {
    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    if showing_original_image() {
        return save_image_png(mtm);
    }
    let kind = if source_is_image() {
        FormatKind::Plain
    } else {
        PREVIEW_KIND.with(|slot| *slot.borrow())
    };
    let body = save_payload(kind);
    if body.is_empty() {
        return false;
    }

    let panel = NSSavePanel::savePanel(mtm);
    panel.setCanCreateDirectories(true);
    panel.setExtensionHidden(false);
    panel.setNameFieldStringValue(&NSString::from_str(&kind.suggested_filename()));
    panel.setTitle(Some(&NSString::from_str("Save clipboard")));
    let ext = NSString::from_str(kind.suggested_extension());
    let types = NSArray::from_slice(&[&*ext]);
    #[allow(deprecated)]
    panel.setAllowedFileTypes(Some(&types));

    if panel.runModal() != NSModalResponseOK {
        return false;
    }
    let Some(url) = panel.URL() else {
        return false;
    };
    let Some(path) = url.path() else {
        return false;
    };
    std::fs::write(path.to_string(), body.as_bytes()).is_ok()
}

fn save_image_png(mtm: MainThreadMarker) -> bool {
    let png = SOURCE_IMAGE.with(|slot| {
        slot.borrow()
            .as_ref()
            .and_then(|image| image.png_bytes().ok())
    });
    let Some(png) = png else {
        return false;
    };
    let kind = FormatKind::Image;
    let panel = NSSavePanel::savePanel(mtm);
    panel.setCanCreateDirectories(true);
    panel.setExtensionHidden(false);
    panel.setNameFieldStringValue(&NSString::from_str(&kind.suggested_filename()));
    panel.setTitle(Some(&NSString::from_str("Save clipboard")));
    let ext = NSString::from_str(kind.suggested_extension());
    let types = NSArray::from_slice(&[&*ext]);
    #[allow(deprecated)]
    panel.setAllowedFileTypes(Some(&types));

    if panel.runModal() != NSModalResponseOK {
        return false;
    }
    let Some(url) = panel.URL() else {
        return false;
    };
    let Some(path) = url.path() else {
        return false;
    };
    std::fs::write(path.to_string(), png).is_ok()
}
