//! The interface languages: English and German. Every user-facing text is a field of
//! [`Text`], so a missing translation is a compile error, not a gap at runtime. German
//! uses "Sie", like the rest of RAGNA's German copy.

use ragna_voice_core::Notice;
use ragna_voice_models::Model;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    English,
    German,
}

impl Locale {
    /// BCP 47 language code, e.g. for asking macOS for language names.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::German => "de",
        }
    }

    /// The first of the user's preferred languages (`de-DE`, `en-US`, …) that the app
    /// speaks; English if none.
    #[must_use]
    pub fn from_preferred(languages: &[String]) -> Self {
        languages
            .iter()
            .find_map(|tag| match tag.split(['-', '_']).next() {
                Some("de") => Some(Self::German),
                Some("en") => Some(Self::English),
                _ => None,
            })
            .unwrap_or(Self::English)
    }

    #[must_use]
    pub fn text(self) -> &'static Text {
        match self {
            Self::English => &ENGLISH,
            Self::German => &GERMAN,
        }
    }

    /// The Model's name: its size, then what it transcribes.
    #[must_use]
    pub fn model_name(self, model: &Model) -> String {
        let size = model.name.split(" (").next().unwrap_or(model.name);
        let t = self.text();
        let kind = if model.multilingual {
            t.multilingual
        } else if *model == ragna_voice_models::BUNDLED {
            t.english_compact
        } else {
            t.english
        };
        format!("{size} ({kind})")
    }

    #[must_use]
    pub fn notice(self, notice: Notice) -> (&'static str, &'static str) {
        let t = self.text();
        match notice {
            Notice::CapReached => (t.notice_cap_title, t.notice_cap_body),
            Notice::PasteFailed => (t.notice_paste_title, t.notice_paste_body),
            Notice::TranscriptionFailed => (t.notice_failed_title, ""),
        }
    }

    /// A short explanation of a Model download or delete failure.
    #[must_use]
    pub fn model_error(self, error: &ragna_voice_models::Error) -> &'static str {
        use ragna_voice_models::Error;
        let t = self.text();
        match error {
            Error::Io(_) => t.error_disk,
            Error::Http(_) => t.error_network,
            Error::Mismatch { .. } => t.error_damaged,
            Error::TooLarge => t.error_too_large,
            Error::Cancelled => t.error_cancelled,
            Error::Bundled => t.error_bundled,
        }
    }
}

/// The interface language setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    /// Follow macOS.
    System,
    Fixed(Locale),
}

impl Choice {
    pub const ALL: [Self; 3] = [
        Self::System,
        Self::Fixed(Locale::English),
        Self::Fixed(Locale::German),
    ];

    /// Stable: preferences store it.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Fixed(locale) => locale.code(),
        }
    }

    #[must_use]
    pub fn from_code(code: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|choice| choice.code() == code)
            .unwrap_or(Self::System)
    }

    /// The label in the picker. Languages name themselves, so a user who can't read
    /// the current language still finds their own.
    #[must_use]
    pub fn label(self, current: Locale) -> &'static str {
        match self {
            Self::System => current.text().system_language,
            Self::Fixed(Locale::English) => "English",
            Self::Fixed(Locale::German) => "Deutsch",
        }
    }
}

#[derive(Debug)]
pub struct Text {
    pub settings_title: &'static str,
    pub menu_settings: &'static str,
    pub menu_quit: &'static str,

    pub version: &'static str,
    pub hint_hold: &'static str,
    pub hint_to_dictate: &'static str,
    pub hint_press: &'static str,
    pub hint_to_discard: &'static str,
    pub key_control: &'static str,
    pub key_option: &'static str,
    pub key_space: &'static str,
    pub key_esc: &'static str,

    pub section_general: &'static str,
    pub app_language: &'static str,
    pub app_language_description: &'static str,
    pub system_language: &'static str,

    pub section_permissions: &'static str,
    pub microphone: &'static str,
    pub microphone_description: &'static str,
    pub accessibility: &'static str,
    pub accessibility_description: &'static str,
    pub open_system_settings: &'static str,
    pub granted: &'static str,
    pub grant: &'static str,
    pub not_granted: &'static str,

    pub section_overlay: &'static str,
    pub show_timer: &'static str,
    pub show_timer_description: &'static str,
    pub transcribing: &'static str,

    pub section_model: &'static str,
    pub language: &'static str,
    pub language_description: &'static str,
    pub language_english_only: &'static str,
    pub automatic: &'static str,
    pub english: &'static str,
    pub english_compact: &'static str,
    pub multilingual: &'static str,
    pub in_use: &'static str,
    pub use_model: &'static str,
    pub delete: &'static str,
    pub download: &'static str,
    pub cancel: &'static str,
    pub models_hint: &'static str,

    pub notice_cap_title: &'static str,
    pub notice_cap_body: &'static str,
    pub notice_paste_title: &'static str,
    pub notice_paste_body: &'static str,
    pub notice_failed_title: &'static str,

    pub error_disk: &'static str,
    pub error_network: &'static str,
    pub error_damaged: &'static str,
    pub error_too_large: &'static str,
    pub error_cancelled: &'static str,
    pub error_bundled: &'static str,
}

pub static ENGLISH: Text = Text {
    settings_title: "RAGNA Voice Settings",
    menu_settings: "Settings…",
    menu_quit: "Quit RAGNA Voice",

    version: "Version",
    hint_hold: "Hold",
    hint_to_dictate: "to dictate.",
    hint_press: "Press",
    hint_to_discard: "while holding to discard.",
    key_control: "Control",
    key_option: "Option",
    key_space: "Space",
    key_esc: "Esc",

    section_general: "General",
    app_language: "App language",
    app_language_description: "The language of menus and Settings.",
    system_language: "System",

    section_permissions: "Permissions",
    microphone: "Microphone",
    microphone_description: "Records your voice while you hold the Hotkey.",
    accessibility: "Accessibility",
    accessibility_description: "Pastes text into the app you're using.",
    open_system_settings: "Open System Settings",
    granted: "Granted",
    grant: "Grant…",
    not_granted: "Not granted yet",

    section_overlay: "Overlay",
    show_timer: "Show recording time",
    show_timer_description: "Counts the seconds from your first word.",
    transcribing: "Transcribing…",

    section_model: "Model",
    language: "Language",
    language_description: "The language you speak. Automatic detects it.",
    language_english_only: "This Model only transcribes English.",
    automatic: "Automatic",
    english: "English",
    english_compact: "English, compact",
    multilingual: "multilingual",
    in_use: "In use",
    use_model: "Use",
    delete: "Delete",
    download: "Download",
    cancel: "Cancel",
    models_hint: "Larger Models are more accurate but slower. English Models only transcribe English.",

    notice_cap_title: "Hit the 5-minute limit",
    notice_cap_body: "Transcribed the first portion.",
    notice_paste_title: "Paste failed",
    notice_paste_body: "Transcript is on your clipboard. Grant Accessibility for auto-paste.",
    notice_failed_title: "Transcription failed",

    error_disk: "Could not write the Model to disk.",
    error_network: "Download failed. Check your internet connection.",
    error_damaged: "The download was damaged. Please try again.",
    error_too_large: "The download was larger than expected and was stopped.",
    error_cancelled: "Cancelled.",
    error_bundled: "The built-in Model can't be deleted.",
};

pub static GERMAN: Text = Text {
    settings_title: "RAGNA Voice Einstellungen",
    menu_settings: "Einstellungen…",
    menu_quit: "RAGNA Voice beenden",

    version: "Version",
    hint_hold: "Halten Sie",
    hint_to_dictate: "gedrückt, um zu diktieren.",
    hint_press: "Drücken Sie",
    hint_to_discard: "beim Halten, um zu verwerfen.",
    key_control: "Control",
    key_option: "Option",
    key_space: "Leertaste",
    key_esc: "Esc",

    section_general: "Allgemein",
    app_language: "App-Sprache",
    app_language_description: "Die Sprache von Menüs und Einstellungen.",
    system_language: "System",

    section_permissions: "Berechtigungen",
    microphone: "Mikrofon",
    microphone_description: "Nimmt Ihre Stimme auf, während Sie den Hotkey halten.",
    accessibility: "Bedienungshilfen",
    accessibility_description: "Fügt Text in die App ein, die Sie gerade verwenden.",
    open_system_settings: "Systemeinstellungen öffnen",
    granted: "Erteilt",
    grant: "Erteilen…",
    not_granted: "Noch nicht erteilt",

    section_overlay: "Overlay",
    show_timer: "Aufnahmezeit anzeigen",
    show_timer_description: "Zählt die Sekunden ab Ihrem ersten Wort.",
    transcribing: "Wird transkribiert…",

    section_model: "Modell",
    language: "Sprache",
    language_description: "Die Sprache, die Sie sprechen. Automatisch erkennt sie selbst.",
    language_english_only: "Dieses Modell transkribiert nur Englisch.",
    automatic: "Automatisch",
    english: "Englisch",
    english_compact: "Englisch, kompakt",
    multilingual: "mehrsprachig",
    in_use: "Aktiv",
    use_model: "Verwenden",
    delete: "Löschen",
    download: "Laden",
    cancel: "Abbrechen",
    models_hint: "Größere Modelle sind genauer, aber langsamer. Englische Modelle transkribieren nur Englisch.",

    notice_cap_title: "5-Minuten-Grenze erreicht",
    notice_cap_body: "Der erste Teil wurde transkribiert.",
    notice_paste_title: "Einfügen fehlgeschlagen",
    notice_paste_body: "Der Text liegt in der Zwischenablage. Erteilen Sie Bedienungshilfen für automatisches Einfügen.",
    notice_failed_title: "Transkription fehlgeschlagen",

    error_disk: "Das Modell konnte nicht gespeichert werden.",
    error_network: "Download fehlgeschlagen. Bitte prüfen Sie Ihre Internetverbindung.",
    error_damaged: "Der Download war beschädigt. Bitte versuchen Sie es erneut.",
    error_too_large: "Der Download war größer als erwartet und wurde gestoppt.",
    error_cancelled: "Abgebrochen.",
    error_bundled: "Das mitgelieferte Modell kann nicht gelöscht werden.",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_the_first_supported_preferred_language() {
        let tags = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(
            Locale::from_preferred(&tags(&["de-DE", "en-US"])),
            Locale::German
        );
        assert_eq!(
            Locale::from_preferred(&tags(&["fr-FR", "de-AT"])),
            Locale::German
        );
        assert_eq!(
            Locale::from_preferred(&tags(&["en-GB", "de-DE"])),
            Locale::English
        );
        assert_eq!(Locale::from_preferred(&tags(&["fr-FR"])), Locale::English);
        assert_eq!(Locale::from_preferred(&[]), Locale::English);
    }

    #[test]
    fn choice_round_trips_through_its_code() {
        for choice in Choice::ALL {
            assert_eq!(Choice::from_code(choice.code()), choice);
        }
        assert_eq!(Choice::from_code("xx"), Choice::System);
    }

    #[test]
    fn model_names_are_translated() {
        let tiny = ragna_voice_models::find("ggml-tiny").unwrap();
        let base_en = ragna_voice_models::find("ggml-base.en").unwrap();
        assert_eq!(Locale::German.model_name(tiny), "Tiny (mehrsprachig)");
        assert_eq!(Locale::German.model_name(base_en), "Base (Englisch)");
        assert_eq!(
            Locale::English.model_name(&ragna_voice_models::BUNDLED),
            "Base (English, compact)"
        );
        assert_eq!(Locale::English.model_name(tiny), "Tiny (multilingual)");
    }
}
