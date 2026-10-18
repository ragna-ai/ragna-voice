use crate::Recording;

/// A pre-Model check that drops near-silent Recordings, which would make
/// Whisper hallucinate ("Thank you.", "Subscribe!").
///
/// Uses peak amplitude, not RMS: long pauses between words drag RMS down but
/// leave the peak unaffected.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SilenceGuard {
    /// Minimum peak (max absolute sample). The default 0.02 (about -34 dBFS)
    /// clears speech with margin and rejects silence and the ambient noise floor.
    pub peak_threshold: f32,
}

impl Default for SilenceGuard {
    fn default() -> Self {
        Self {
            peak_threshold: 0.02,
        }
    }
}

impl SilenceGuard {
    /// `false` means the Recording should be Dropped.
    #[must_use]
    pub fn passes(&self, recording: &Recording) -> bool {
        self.is_speech(recording.samples())
    }

    /// Whether `samples` (any rate or channel layout) are loud enough to be speech.
    #[must_use]
    pub fn is_speech(&self, samples: &[f32]) -> bool {
        // `f32::max` ignores NaN, so corrupt samples can't pass.
        let peak = samples.iter().fold(0.0_f32, |peak, s| peak.max(s.abs()));
        !samples.is_empty() && peak >= self.peak_threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guard(peak_threshold: f32) -> SilenceGuard {
        SilenceGuard { peak_threshold }
    }

    #[test]
    fn empty_samples_fail() {
        assert!(!SilenceGuard::default().passes(&Recording::default()));
    }

    #[test]
    fn all_zero_samples_fail() {
        assert!(!SilenceGuard::default().passes(&Recording::new(vec![0.0; 16_000])));
    }

    #[test]
    fn near_silent_samples_below_threshold_fail() {
        assert!(!guard(0.02).passes(&Recording::new(vec![0.001; 16_000])));
    }

    #[test]
    fn loud_samples_above_threshold_pass() {
        assert!(guard(0.02).passes(&Recording::new(vec![0.5; 16_000])));
    }

    #[test]
    fn custom_threshold_below_is_rejected() {
        assert!(!guard(0.1).passes(&Recording::new(vec![0.05; 16_000])));
    }

    #[test]
    fn custom_threshold_above_passes() {
        assert!(guard(0.05).passes(&Recording::new(vec![0.1; 16_000])));
    }

    #[test]
    fn negative_peaks_count() {
        assert!(SilenceGuard::default().passes(&Recording::new(vec![-0.5; 16_000])));
    }

    /// The APP-47 regression: mostly silence with a few speech peaks. RMS is
    /// about 0.00175, which an RMS check would reject; the peak of 0.07 passes.
    #[test]
    fn sparse_peaks_pass_under_peak_metric() {
        let mut samples = vec![0.0; 16_000];
        for s in samples.iter_mut().step_by(160) {
            *s = 0.07;
        }
        assert!(SilenceGuard::default().passes(&Recording::new(samples)));
    }

    #[test]
    fn is_speech_uses_the_same_threshold_on_raw_buffers() {
        let guard = SilenceGuard::default();
        assert!(!guard.is_speech(&[]));
        assert!(!guard.is_speech(&[0.01, -0.019]));
        assert!(guard.is_speech(&[0.0, -0.02]));
    }

    #[test]
    fn nan_samples_do_not_pass() {
        assert!(!SilenceGuard::default().passes(&Recording::new(vec![f32::NAN; 16_000])));
    }
}
