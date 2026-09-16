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
    TEXT.with(|slot| {
        if let Some(text) = slot.borrow().as_ref() {
            fade_swap_text(text, body, kind);
        }
    });
    flash_button(&COPY_BUTTON, "Copied  \u{2713}", "Copy", copy_flash_color(), false);
}

fn fade_swap_text(text: &NSTextView, body: &str, kind: FormatKind) {
    text.setAlphaValue(0.0);
    set_body(text, body, kind);
    NSAnimationContext::beginGrouping();
    NSAnimationContext::currentContext().setDuration(0.28);
    text.animator().setAlphaValue(1.0);
    NSAnimationContext::endGrouping();
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
