fn apply_preview(body: &str) {
    apply_preview_with_kind(body, format::detect(body));
}

fn apply_preview_with_kind(body: &str, kind: FormatKind) {
    PREVIEW_KIND.with(|slot| slot.replace(kind));
    PREVIEW_TEXT.with(|slot| slot.replace(body.to_string()));
    WINDOW.with(|slot| {
        if let Some(window) = slot.borrow().as_ref() {
            window.setTitle(&NSString::from_str(kind.preview_heading()));
        }
    });
    fade_scroll(0.0, 0.16);
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
    flash_button(&COPY_BUTTON, "Copied  \u{2713}", "Copy", copy_flash_color(), false);
}

fn reveal_preview_body() {
    let kind = PREVIEW_KIND.with(|slot| *slot.borrow());
    let body = PREVIEW_TEXT.with(|slot| slot.borrow().clone());
    TEXT.with(|slot| {
        if let Some(text) = slot.borrow().as_ref() {
            set_body(text, &body, kind);
        }
    });
    fade_scroll(1.0, 0.28);
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

fn make_toolbar_button(
    mtm: MainThreadMarker,
    frame: NSRect,
    target: &PreviewTarget,
    action: objc2::runtime::Sel,
    stick_right: bool,
) -> Retained<NSButton> {
    let button = NSButton::initWithFrame(NSButton::alloc(mtm), frame);
    button.setBordered(true);
    let mask = if stick_right {
        NSAutoresizingMaskOptions::ViewMinXMargin | NSAutoresizingMaskOptions::ViewMinYMargin
    } else {
        NSAutoresizingMaskOptions::ViewMaxXMargin | NSAutoresizingMaskOptions::ViewMinYMargin
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
    let kind = PREVIEW_KIND.with(|slot| *slot.borrow());
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
