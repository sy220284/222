//! Rust exports derived from original: src/hooks/useChromeExtensionNotification.tsx
//!
//! Full equivalence of the flag parsing: `--chrome` forces on, `--no-chrome`
//! forces off, neither leaves the decision to the auth/extension chain.
//! The notification body rides the vendor chrome-extension surface
//! (excluded); the startup-notification delivery is the shared hook.

/// `getChromeFlag` — argv wins over configuration.
pub fn get_chrome_flag(argv: &[String]) -> Option<bool> {
    if argv.iter().any(|a| a == "--chrome") {
        return Some(true);
    }
    if argv.iter().any(|a| a == "--no-chrome") {
        return Some(false);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_precedence() {
        assert_eq!(get_chrome_flag(&["--chrome".into()]), Some(true));
        assert_eq!(get_chrome_flag(&["--no-chrome".into()]), Some(false));
        assert_eq!(get_chrome_flag(&[]), None);
    }
}
