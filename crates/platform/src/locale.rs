//! The user's languages, as macOS knows them.

/// The user's preferred languages, most preferred first, as BCP 47 tags (`de-DE`, `en`).
#[must_use]
pub fn preferred_languages() -> Vec<String> {
    imp::preferred_languages()
}

/// The name of the language `code` (e.g. `fr`) written in the language `in_language`
/// (e.g. `de` gives "Französisch"), or `None` if macOS doesn't know it.
#[must_use]
pub fn language_name(code: &str, in_language: &str) -> Option<String> {
    imp::language_name(code, in_language)
}

#[cfg(target_os = "macos")]
mod imp {
    use objc2_foundation::{NSLocale, NSString};

    pub fn preferred_languages() -> Vec<String> {
        NSLocale::preferredLanguages()
            .iter()
            .map(|tag| tag.to_string())
            .collect()
    }

    pub fn language_name(code: &str, in_language: &str) -> Option<String> {
        let locale = NSLocale::localeWithLocaleIdentifier(&NSString::from_str(in_language));
        let name = locale.localizedStringForLanguageCode(&NSString::from_str(code))?;
        Some(name.to_string())
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn preferred_languages() -> Vec<String> {
        Vec::new()
    }

    pub fn language_name(_code: &str, _in_language: &str) -> Option<String> {
        None
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn macos_names_languages_in_the_requested_language() {
        assert_eq!(language_name("de", "de").as_deref(), Some("Deutsch"));
        assert_eq!(language_name("fr", "de").as_deref(), Some("Französisch"));
        assert_eq!(language_name("de", "en").as_deref(), Some("German"));
    }
}
