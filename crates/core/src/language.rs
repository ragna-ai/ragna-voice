//! Transcription languages: the Whisper language codes the app offers.

/// A language Whisper can transcribe, or [`AUTOMATIC`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Language {
    /// The Whisper code, e.g. `de`. Stable: preferences store it.
    pub code: &'static str,
    /// The English name, for the picker.
    pub name: &'static str,
}

/// Whisper detects the spoken language. Multilingual Models only.
pub const AUTOMATIC: Language = lang("auto", "Automatic");

/// English, the only language English-only (`*.en`) Models can transcribe.
pub const ENGLISH: Language = lang("en", "English");

/// [`AUTOMATIC`] first, then every language by name.
pub const LANGUAGES: &[Language] = &[
    AUTOMATIC,
    lang("af", "Afrikaans"),
    lang("sq", "Albanian"),
    lang("ar", "Arabic"),
    lang("hy", "Armenian"),
    lang("az", "Azerbaijani"),
    lang("eu", "Basque"),
    lang("be", "Belarusian"),
    lang("bn", "Bengali"),
    lang("bs", "Bosnian"),
    lang("bg", "Bulgarian"),
    lang("my", "Burmese"),
    lang("ca", "Catalan"),
    lang("zh", "Chinese"),
    lang("hr", "Croatian"),
    lang("cs", "Czech"),
    lang("da", "Danish"),
    lang("nl", "Dutch"),
    lang("en", "English"),
    lang("et", "Estonian"),
    lang("fi", "Finnish"),
    lang("fr", "French"),
    lang("gl", "Galician"),
    lang("ka", "Georgian"),
    lang("de", "German"),
    lang("el", "Greek"),
    lang("he", "Hebrew"),
    lang("hi", "Hindi"),
    lang("hu", "Hungarian"),
    lang("is", "Icelandic"),
    lang("id", "Indonesian"),
    lang("it", "Italian"),
    lang("ja", "Japanese"),
    lang("kn", "Kannada"),
    lang("kk", "Kazakh"),
    lang("ko", "Korean"),
    lang("lv", "Latvian"),
    lang("lt", "Lithuanian"),
    lang("mk", "Macedonian"),
    lang("ms", "Malay"),
    lang("ml", "Malayalam"),
    lang("mt", "Maltese"),
    lang("mi", "Maori"),
    lang("mr", "Marathi"),
    lang("ne", "Nepali"),
    lang("no", "Norwegian"),
    lang("fa", "Persian"),
    lang("pl", "Polish"),
    lang("pt", "Portuguese"),
    lang("ro", "Romanian"),
    lang("ru", "Russian"),
    lang("sr", "Serbian"),
    lang("sk", "Slovak"),
    lang("sl", "Slovenian"),
    lang("es", "Spanish"),
    lang("sw", "Swahili"),
    lang("sv", "Swedish"),
    lang("tl", "Tagalog / Filipino"),
    lang("ta", "Tamil"),
    lang("te", "Telugu"),
    lang("th", "Thai"),
    lang("tr", "Turkish"),
    lang("uk", "Ukrainian"),
    lang("ur", "Urdu"),
    lang("vi", "Vietnamese"),
    lang("cy", "Welsh"),
];

const fn lang(code: &'static str, name: &'static str) -> Language {
    Language { code, name }
}

/// The language with this code; [`AUTOMATIC`] for an unknown code, so a stale or
/// corrupt preference can't break transcription.
#[must_use]
pub fn find(code: &str) -> Language {
    LANGUAGES
        .iter()
        .copied()
        .find(|language| language.code == code)
        .unwrap_or(AUTOMATIC)
}

/// The language to pass to the Model: the user's choice, but always English for an
/// English-only Model, which can't transcribe anything else.
#[must_use]
pub fn for_model(chosen: Language, multilingual: bool) -> Language {
    if multilingual { chosen } else { ENGLISH }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn languages_are_unique_and_sorted_after_automatic() {
        assert_eq!(LANGUAGES[0], AUTOMATIC);
        let rest = &LANGUAGES[1..];
        assert!(
            rest.windows(2).all(|w| w[0].name < w[1].name),
            "sorted by name"
        );
        for (i, a) in LANGUAGES.iter().enumerate() {
            assert!(
                LANGUAGES[i + 1..].iter().all(|b| b.code != a.code),
                "duplicate {}",
                a.code
            );
        }
        assert!(LANGUAGES.contains(&ENGLISH));
        assert!(
            LANGUAGES
                .iter()
                .any(|l| l.code == "de" && l.name == "German")
        );
    }

    #[test]
    fn unknown_codes_fall_back_to_automatic() {
        assert_eq!(find("de").name, "German");
        assert_eq!(find("xx"), AUTOMATIC);
        assert_eq!(find(""), AUTOMATIC);
    }

    #[test]
    fn english_only_models_always_get_english() {
        assert_eq!(for_model(find("de"), false), ENGLISH);
        assert_eq!(for_model(AUTOMATIC, false), ENGLISH);
        assert_eq!(for_model(find("de"), true).code, "de");
        assert_eq!(for_model(AUTOMATIC, true), AUTOMATIC);
    }
}
