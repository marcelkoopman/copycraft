/// The menu bar icon is a badge. The command popup is the product, so the
/// badge stays hidden until someone asks for it.
pub const DEFAULT_SHOWN: bool = false;

const DEFAULTS_KEY: &str = "CopycraftMenuBarBadge";

pub fn is_shown() -> bool {
    load().unwrap_or(DEFAULT_SHOWN)
}

pub fn set_shown(shown: bool) {
    store(shown);
}

fn load() -> Option<bool> {
    #[cfg(target_os = "macos")]
    {
        use objc2_foundation::{NSString, NSUserDefaults};
        let defaults = NSUserDefaults::standardUserDefaults();
        let key = NSString::from_str(DEFAULTS_KEY);
        defaults.objectForKey(&key)?;
        Some(defaults.boolForKey(&key))
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

fn store(shown: bool) {
    #[cfg(target_os = "macos")]
    {
        use objc2_foundation::{NSString, NSUserDefaults};
        let defaults = NSUserDefaults::standardUserDefaults();
        let key = NSString::from_str(DEFAULTS_KEY);
        defaults.setBool_forKey(shown, &key);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = shown;
    }
}

#[cfg(test)]
mod tests {
    use super::DEFAULT_SHOWN;

    #[test]
    fn menu_bar_badge_is_hidden_by_default() {
        assert!(!DEFAULT_SHOWN);
    }
}
