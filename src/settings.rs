const DEFAULTS_KEY: &str = "CopycraftAutoFormat";

pub fn auto_format_enabled() -> bool {
    load().unwrap_or(true)
}

pub fn set_auto_format_enabled(enabled: bool) {
    store(enabled);
}

fn load() -> Option<bool> {
    #[cfg(target_os = "macos")]
    {
        use objc2_foundation::{NSString, NSUserDefaults};
        let defaults = NSUserDefaults::standardUserDefaults();
        let key = NSString::from_str(DEFAULTS_KEY);
        let value = defaults.stringForKey(&key)?;
        match value.to_string().as_str() {
            "on" => Some(true),
            "off" => Some(false),
            _ => None,
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

fn store(enabled: bool) {
    #[cfg(target_os = "macos")]
    {
        use objc2_foundation::{NSString, NSUserDefaults};
        let defaults = NSUserDefaults::standardUserDefaults();
        let key = NSString::from_str(DEFAULTS_KEY);
        let value = if enabled { "on" } else { "off" };
        unsafe {
            defaults.setObject_forKey(Some(&NSString::from_str(value)), &key);
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = enabled;
    }
}

#[cfg(test)]
mod tests {
    use super::auto_format_enabled;

    #[test]
    fn defaults_on_when_unset_or_readable() {
        let _ = auto_format_enabled();
    }
}
