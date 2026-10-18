//! whisper.cpp bridge: turns a Recording into a Transcript.

#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

use std::fmt;
use std::path::{Path, PathBuf};

use ragna_voice_core::language::{self, Language};
use ragna_voice_core::{Recording, Transcript};
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};

/// whisper.cpp aborts the process on (near-)empty input. 1024 samples is 64 ms at 16 kHz.
const MIN_SAMPLES: usize = 1024;

#[derive(Debug)]
pub enum Error {
    ModelNotFound(PathBuf),
    /// whisper.cpp takes paths as UTF-8 strings.
    NonUtf8Path(PathBuf),
    Whisper(whisper_rs::WhisperError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ModelNotFound(path) => write!(f, "no Model at {}", path.display()),
            Self::NonUtf8Path(path) => write!(f, "Model path is not UTF-8: {}", path.display()),
            Self::Whisper(e) => write!(f, "whisper.cpp: {e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<whisper_rs::WhisperError> for Error {
    fn from(e: whisper_rs::WhisperError) -> Self {
        Self::Whisper(e)
    }
}

/// A loaded Model, reused for every Dictation. Inference runs on Metal when available.
pub struct Transcriber {
    state: WhisperState,
    threads: i32,
    /// English-only Models (`*.en`) can only transcribe English.
    multilingual: bool,
}

impl fmt::Debug for Transcriber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Transcriber")
            .field("threads", &self.threads)
            .field("multilingual", &self.multilingual)
            .finish_non_exhaustive()
    }
}

impl Transcriber {
    /// Loads a whisper.cpp `ggml-*.bin` Model. Slow (hundreds of ms): call it off the UI thread.
    ///
    /// # Errors
    ///
    /// The file is missing, its path is not UTF-8, or whisper.cpp rejects it.
    pub fn load(model: &Path) -> Result<Self, Error> {
        if !model.is_file() {
            return Err(Error::ModelNotFound(model.to_path_buf()));
        }
        let path = model
            .to_str()
            .ok_or_else(|| Error::NonUtf8Path(model.to_path_buf()))?;
        whisper_rs::install_logging_hooks();
        let context = WhisperContext::new_with_params(path, WhisperContextParameters::default())?;
        let threads = std::thread::available_parallelism()
            .map_or(4, |n| n.get().min(8))
            .try_into()
            .unwrap_or(4);
        Ok(Self {
            multilingual: context.is_multilingual(),
            state: context.create_state()?,
            threads,
        })
    }

    /// Transcribes `recording` in `language` ([`language::AUTOMATIC`] lets Whisper detect
    /// it). English-only Models always transcribe English. A Recording too short to hold
    /// speech gives an empty Transcript rather than an error.
    ///
    /// # Errors
    ///
    /// whisper.cpp inference failed.
    pub fn transcribe(
        &mut self,
        recording: &Recording,
        language: Language,
    ) -> Result<Transcript, Error> {
        let Some(samples) = prepare(recording) else {
            return Ok(Transcript::new(""));
        };
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some(language::for_model(language, self.multilingual).code));
        params.set_no_timestamps(true);
        params.set_n_threads(self.threads);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        self.state.full(params, &samples)?;
        let mut text = String::new();
        for segment in self.state.as_iter() {
            text.push_str(&segment.to_str_lossy()?);
        }
        Ok(Transcript::new(text.trim()))
    }
}

/// The samples to feed whisper.cpp, or `None` if there are too few to transcribe.
/// Non-finite samples become silence: ggml must never see NaN or infinity.
fn prepare(recording: &Recording) -> Option<Vec<f32>> {
    let samples = recording.samples();
    (samples.len() >= MIN_SAMPLES).then(|| {
        samples
            .iter()
            .map(|&s| if s.is_finite() { s } else { 0.0 })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_model_is_an_error_not_a_panic() {
        let err = Transcriber::load(Path::new("/nonexistent/ggml-none.bin")).unwrap_err();
        assert!(matches!(err, Error::ModelNotFound(_)), "{err}");
    }

    #[test]
    fn too_short_recordings_are_not_sent_to_whisper() {
        assert!(prepare(&Recording::new(vec![0.5; MIN_SAMPLES - 1])).is_none());
        assert!(prepare(&Recording::new(vec![0.5; MIN_SAMPLES])).is_some());
    }

    #[test]
    fn non_finite_samples_become_silence() {
        let mut samples = vec![0.5; MIN_SAMPLES];
        samples[0] = f32::NAN;
        samples[1] = f32::INFINITY;
        let prepared = prepare(&Recording::new(samples)).unwrap();
        assert_eq!(prepared[..3], [0.0, 0.0, 0.5]);
    }

    /// Needs a real Model: `RAGNA_VOICE_MODEL=path/to/ggml-base.en.bin cargo test -- --ignored`.
    #[test]
    #[ignore = "needs RAGNA_VOICE_MODEL"]
    fn transcribes_a_tone_and_silence_without_crashing() {
        let model = std::env::var_os("RAGNA_VOICE_MODEL").expect("RAGNA_VOICE_MODEL");
        let mut transcriber = Transcriber::load(Path::new(&model)).unwrap();
        let tone: Vec<f32> = (0..16_000u16)
            .map(|i| (f32::from(i) / 16_000.0 * 440.0 * std::f32::consts::TAU).sin() * 0.3)
            .collect();
        let auto = language::AUTOMATIC;
        transcriber.transcribe(&Recording::new(tone), auto).unwrap();
        transcriber
            .transcribe(&Recording::new(vec![0.0; 16_000]), auto)
            .unwrap();
        transcriber
            .transcribe(&Recording::new(vec![0.0; 10]), language::find("de"))
            .unwrap();
    }
}
