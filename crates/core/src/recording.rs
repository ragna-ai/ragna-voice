use std::fmt;
use std::time::Duration;

/// Sample rate of every Recording: 16 kHz mono, what whisper.cpp expects.
pub const SAMPLE_RATE: u32 = 16_000;

/// The audio captured between Hotkey-down and Hotkey-up: 16 kHz mono
/// normalised PCM. Lives in RAM only (`AGENTS.md` §2).
#[derive(Clone, PartialEq, Default)]
pub struct Recording {
    samples: Vec<f32>,
}

impl Recording {
    #[must_use]
    pub fn new(samples: Vec<f32>) -> Self {
        Self { samples }
    }

    #[must_use]
    pub fn samples(&self) -> &[f32] {
        &self.samples
    }

    /// This Recording without its first `skip` of audio (all of it, if shorter).
    #[must_use]
    pub fn without_start(mut self, skip: Duration) -> Self {
        let rate = u128::from(SAMPLE_RATE);
        let skip = usize::try_from(skip.as_nanos() * rate / 1_000_000_000).unwrap_or(usize::MAX);
        self.samples.drain(..skip.min(self.samples.len()));
        self
    }

    #[must_use]
    pub fn duration(&self) -> Duration {
        let len = u64::try_from(self.samples.len()).unwrap_or(u64::MAX);
        let rate = u64::from(SAMPLE_RATE);
        Duration::from_secs(len / rate) + Duration::from_nanos((len % rate) * 1_000_000_000 / rate)
    }
}

// Never print samples: a Debug dump would be both huge and a copy of the user's voice.
impl fmt::Debug for Recording {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Recording")
            .field("duration", &self.duration())
            .finish()
    }
}

/// The text the Model produced from a Recording.
#[derive(Clone, PartialEq, Eq)]
pub struct Transcript {
    text: String,
}

impl Transcript {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

// Redacted so a stray `{:?}` in a log line can't leak what the user said (`AGENTS.md` §2).
impl fmt::Debug for Transcript {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Transcript")
            .field("chars", &self.text.chars().count())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_follows_sample_count() {
        assert_eq!(
            Recording::new(vec![0.0; 16_000]).duration(),
            Duration::from_secs(1)
        );
        assert_eq!(
            Recording::new(vec![0.0; 3_200]).duration(),
            Duration::from_millis(200)
        );
        assert_eq!(Recording::default().duration(), Duration::ZERO);
    }

    #[test]
    fn without_start_drops_leading_audio() {
        let recording = Recording::new((0..16_000u16).map(f32::from).collect());
        let trimmed = recording.clone().without_start(Duration::from_millis(250));
        assert_eq!(trimmed.duration(), Duration::from_millis(750));
        assert_eq!(trimmed.samples().first().copied(), Some(4_000.0));
        assert_eq!(recording.clone().without_start(Duration::ZERO), recording);
        assert_eq!(
            recording.without_start(Duration::from_secs(9)).duration(),
            Duration::ZERO
        );
    }

    #[test]
    fn transcript_debug_does_not_leak_text() {
        let debug = format!("{:?}", Transcript::new("my secret"));
        assert!(!debug.contains("secret"), "{debug}");
    }

    #[test]
    fn recording_debug_does_not_dump_samples() {
        let debug = format!("{:?}", Recording::new(vec![0.123; 16]));
        assert!(!debug.contains("0.123"), "{debug}");
    }
}
