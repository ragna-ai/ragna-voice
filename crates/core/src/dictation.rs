//! The Dictation state machine, sans-IO (ADR-0002).
//!
//! The runner feeds in [`Event`]s with the current time and executes the
//! [`Effect`]s that come back. This module never touches the clock, a thread
//! or the OS, so every transition is deterministic and testable.

use std::time::{Duration, Instant};

use crate::{Recording, SilenceGuard, Transcript};

/// Recordings shorter than this are Dropped without notification.
pub const MIN_RECORDING: Duration = Duration::from_millis(200);
/// After this long the Recording continues, but the state turns to [`State::SoftCapWarning`].
pub const SOFT_CAP: Duration = Duration::from_secs(60);
/// After this long the Recording stops by itself and the outcome is Capped.
pub const HARD_CAP: Duration = Duration::from_mins(5);
/// Audio kept before the first detected speech when the silence before it is trimmed.
/// Detection fires on the first loud peak, a little after the word's soft onset.
pub const SPEECH_PREROLL: Duration = Duration::from_millis(300);

/// Something that happened, reported by the runner.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    HotkeyDown,
    HotkeyUp,
    /// Esc pressed while the Hotkey is held.
    EscPressed,
    /// The first audio arrived. Until then the microphone is still starting up
    /// (about 90 ms for a wired mic, up to a second for Bluetooth).
    AudioStarted,
    /// The first audio loud enough to be speech ([`SilenceGuard::is_speech`]) arrived.
    SpeechStarted,
    /// The microphone disappeared mid-Recording.
    DeviceLost,
    /// The [`Effect::StartRecording`] failed (e.g. microphone permission denied).
    RecordingFailedToStart,
    /// The result of [`Effect::StopRecording`].
    RecordingStopped(Recording),
    /// The result of [`Effect::Transcribe`].
    Transcribed(Transcript),
    TranscriptionFailed,
    /// The result of [`Effect::Insert`].
    Inserted,
    /// Insertion failed; the Transcript was left on the clipboard.
    InsertionFailed,
    TogglePause,
    ModelLoading,
    ModelReady,
    ModelUnavailable,
}

/// Work for the runner. Results come back as [`Event`]s.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    StartRecording,
    /// Stop capture, then report [`Event::RecordingStopped`].
    StopRecording,
    StartEscTap,
    StopEscTap,
    /// Report [`Event::Transcribed`] or [`Event::TranscriptionFailed`].
    Transcribe(Recording),
    /// Report [`Event::Inserted`] or [`Event::InsertionFailed`].
    Insert(Transcript),
    /// The Dictation ended. Post [`Outcome::notice`], if any.
    Finished(Outcome),
}

/// The terminal state of a Dictation (`CONTEXT.md`). Exactly one per Dictation.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Inserted(Transcript),
    /// Insertion failed; the Transcript is on the clipboard.
    Notified(Transcript),
    /// Esc pressed, or the microphone disappeared.
    Cancelled,
    /// Shorter than [`MIN_RECORDING`], rejected by the Silence Guard, or the Model heard no words.
    Dropped,
    /// Hit [`HARD_CAP`]; the captured portion was inserted.
    Capped(Transcript),
    /// The Model failed; no Transcript.
    Failed,
}

impl Outcome {
    /// The user-facing notification for this outcome, if it gets one.
    #[must_use]
    pub fn notice(&self) -> Option<Notice> {
        match self {
            Self::Capped(_) => Some(Notice::CapReached),
            Self::Notified(_) => Some(Notice::PasteFailed),
            Self::Failed => Some(Notice::TranscriptionFailed),
            Self::Inserted(_) | Self::Cancelled | Self::Dropped => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notice {
    CapReached,
    PasteFailed,
    TranscriptionFailed,
}

impl Notice {
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::CapReached => "Hit the 5-minute limit",
            Self::PasteFailed => "Paste failed",
            Self::TranscriptionFailed => "Transcription failed",
        }
    }

    #[must_use]
    pub fn body(self) -> &'static str {
        match self {
            Self::CapReached => "Transcribed the first portion.",
            Self::PasteFailed => {
                "Transcript is on your clipboard. Grant Accessibility for auto-paste."
            }
            Self::TranscriptionFailed => "",
        }
    }
}

/// What the UI shows (menu-bar icon, waveform overlay).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Idle,
    Recording,
    /// Recording past [`SOFT_CAP`]; still recording.
    SoftCapWarning,
    /// Hotkey released; the Recording is being stopped, transcribed and inserted.
    Transcribing,
    Paused,
    ModelLoading,
    /// The Model failed to load. The Hotkey does nothing until it loads.
    ModelUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Model {
    Loading,
    Ready,
    Unavailable,
}

#[derive(Debug)]
enum Phase {
    Idle,
    Recording {
        /// Hotkey-down. The caps count from here.
        started_at: Instant,
        /// First audio, once it arrived. The Recording's samples start here.
        capturing_since: Option<Instant>,
        /// First speech, once heard.
        speaking_since: Option<Instant>,
        soft_cap_reached: bool,
    },
    Stopping {
        cancelled: bool,
        capped: bool,
        /// Silence before the first speech, to trim before transcribing.
        lead_silence: Duration,
    },
    Transcribing {
        capped: bool,
    },
    Inserting {
        transcript: Transcript,
        capped: bool,
    },
}

/// One Dictation at a time, from Hotkey-down to [`Effect::Finished`].
#[derive(Debug)]
pub struct Dictation {
    phase: Phase,
    model: Model,
    paused: bool,
    silence_guard: SilenceGuard,
}

impl Default for Dictation {
    fn default() -> Self {
        Self::new()
    }
}

impl Dictation {
    /// Starts in [`State::ModelLoading`]; send [`Event::ModelReady`] once the Model is loaded.
    #[must_use]
    pub fn new() -> Self {
        Self {
            phase: Phase::Idle,
            model: Model::Loading,
            paused: false,
            silence_guard: SilenceGuard::default(),
        }
    }

    #[must_use]
    pub fn state(&self) -> State {
        match self.phase {
            Phase::Recording {
                soft_cap_reached: false,
                ..
            } => State::Recording,
            Phase::Recording {
                soft_cap_reached: true,
                ..
            } => State::SoftCapWarning,
            // A cancelled Dictation is over for the user the moment they cancel; only
            // the recorder still has to shut down.
            Phase::Stopping {
                cancelled: false, ..
            }
            | Phase::Transcribing { .. }
            | Phase::Inserting { .. } => State::Transcribing,
            Phase::Idle
            | Phase::Stopping {
                cancelled: true, ..
            } => match self.model {
                Model::Unavailable => State::ModelUnavailable,
                Model::Loading => State::ModelLoading,
                Model::Ready if self.paused => State::Paused,
                Model::Ready => State::Idle,
            },
        }
    }

    /// When the microphone started delivering audio, while recording. `None` while it
    /// is still starting up: the user's first words would be lost, so the UI should not
    /// count yet.
    #[must_use]
    pub fn capturing_since(&self) -> Option<Instant> {
        match self.phase {
            Phase::Recording {
                capturing_since, ..
            } => capturing_since,
            _ => None,
        }
    }

    /// When the user started speaking, while recording. The UI's timer counts from here.
    #[must_use]
    pub fn speaking_since(&self) -> Option<Instant> {
        match self.phase {
            Phase::Recording { speaking_since, .. } => speaking_since,
            _ => None,
        }
    }

    /// When the runner must call [`Self::handle_timeout`] next, if ever.
    #[must_use]
    pub fn poll_timeout(&self) -> Option<Instant> {
        match self.phase {
            Phase::Recording {
                started_at,
                soft_cap_reached,
                ..
            } => started_at.checked_add(if soft_cap_reached { HARD_CAP } else { SOFT_CAP }),
            _ => None,
        }
    }

    pub fn handle_timeout(&mut self, now: Instant) -> Vec<Effect> {
        let Phase::Recording {
            started_at,
            soft_cap_reached,
            ..
        } = &mut self.phase
        else {
            return Vec::new();
        };
        let elapsed = now.saturating_duration_since(*started_at);
        if elapsed >= SOFT_CAP {
            *soft_cap_reached = true;
        }
        if elapsed >= HARD_CAP {
            return self.stop_recording(true, false);
        }
        Vec::new()
    }

    pub fn handle(&mut self, event: Event, now: Instant) -> Vec<Effect> {
        match event {
            Event::HotkeyDown => self.start_recording(now),
            Event::HotkeyUp => self.stop_recording(false, false),
            Event::AudioStarted => {
                if let Phase::Recording {
                    capturing_since: since @ None,
                    ..
                } = &mut self.phase
                {
                    *since = Some(now);
                }
                Vec::new()
            }
            Event::SpeechStarted => {
                if let Phase::Recording {
                    speaking_since: since @ None,
                    ..
                } = &mut self.phase
                {
                    *since = Some(now);
                }
                Vec::new()
            }
            // Stop at once, so the overlay disappears as feedback; the Hotkey's release
            // then finds nothing to stop.
            Event::EscPressed | Event::DeviceLost => self.stop_recording(false, true),
            Event::RecordingFailedToStart => {
                if matches!(self.phase, Phase::Recording { .. }) {
                    self.phase = Phase::Idle;
                    vec![Effect::StopEscTap]
                } else {
                    Vec::new()
                }
            }
            Event::RecordingStopped(recording) => self.on_recording_stopped(recording),
            Event::Transcribed(transcript) => match self.phase {
                // Pasting "" would only wipe the clipboard.
                Phase::Transcribing { .. } if transcript.text().trim().is_empty() => {
                    self.finish(Outcome::Dropped)
                }
                Phase::Transcribing { capped } => {
                    self.phase = Phase::Inserting {
                        transcript: transcript.clone(),
                        capped,
                    };
                    vec![Effect::Insert(transcript)]
                }
                _ => Vec::new(),
            },
            Event::TranscriptionFailed => match self.phase {
                Phase::Transcribing { .. } => self.finish(Outcome::Failed),
                _ => Vec::new(),
            },
            Event::Inserted | Event::InsertionFailed => {
                match std::mem::replace(&mut self.phase, Phase::Idle) {
                    Phase::Inserting { transcript, capped } => {
                        self.finish(if event == Event::InsertionFailed {
                            Outcome::Notified(transcript)
                        } else if capped {
                            Outcome::Capped(transcript)
                        } else {
                            Outcome::Inserted(transcript)
                        })
                    }
                    other => {
                        self.phase = other;
                        Vec::new()
                    }
                }
            }
            Event::TogglePause => {
                if self.paused {
                    self.paused = false;
                } else if self.state() == State::Idle {
                    self.paused = true;
                }
                Vec::new()
            }
            Event::ModelLoading => self.set_model(Model::Loading),
            Event::ModelReady => self.set_model(Model::Ready),
            Event::ModelUnavailable => self.set_model(Model::Unavailable),
        }
    }

    fn start_recording(&mut self, now: Instant) -> Vec<Effect> {
        // The phase check matters after a cancel: the state already reads Idle while
        // the previous Recording is still being stopped.
        if self.state() != State::Idle || !matches!(self.phase, Phase::Idle) {
            return Vec::new();
        }
        self.phase = Phase::Recording {
            started_at: now,
            capturing_since: None,
            speaking_since: None,
            soft_cap_reached: false,
        };
        vec![Effect::StartRecording, Effect::StartEscTap]
    }

    fn stop_recording(&mut self, capped: bool, cancelled: bool) -> Vec<Effect> {
        let Phase::Recording {
            capturing_since,
            speaking_since,
            ..
        } = self.phase
        else {
            return Vec::new();
        };
        let lead_silence = match (capturing_since, speaking_since) {
            (Some(capturing), Some(speaking)) => speaking
                .saturating_duration_since(capturing)
                .saturating_sub(SPEECH_PREROLL),
            _ => Duration::ZERO,
        };
        self.phase = Phase::Stopping {
            cancelled,
            capped,
            lead_silence,
        };
        vec![Effect::StopEscTap, Effect::StopRecording]
    }

    fn on_recording_stopped(&mut self, recording: Recording) -> Vec<Effect> {
        let Phase::Stopping {
            cancelled,
            capped,
            lead_silence,
        } = self.phase
        else {
            return Vec::new();
        };
        if cancelled {
            return self.finish(Outcome::Cancelled);
        }
        if recording.duration() < MIN_RECORDING || !self.silence_guard.passes(&recording) {
            return self.finish(Outcome::Dropped);
        }
        self.phase = Phase::Transcribing { capped };
        vec![Effect::Transcribe(recording.without_start(lead_silence))]
    }

    fn finish(&mut self, outcome: Outcome) -> Vec<Effect> {
        self.phase = Phase::Idle;
        vec![Effect::Finished(outcome)]
    }

    // A Model change never interrupts a Dictation in flight: it only gates the next Hotkey-down.
    fn set_model(&mut self, model: Model) -> Vec<Effect> {
        self.model = model;
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 16 samples per millisecond at 16 kHz.
    fn loud(ms: usize) -> Recording {
        Recording::new(vec![0.5; ms * 16])
    }

    fn silent(ms: usize) -> Recording {
        Recording::new(vec![0.0; ms * 16])
    }

    fn ready(t0: Instant) -> Dictation {
        let mut d = Dictation::new();
        d.handle(Event::ModelReady, t0);
        d
    }

    /// Hotkey-down, then Hotkey-up `held` later, then the recorder hands back `recording`.
    fn dictate(
        d: &mut Dictation,
        t0: Instant,
        held: Duration,
        recording: Recording,
    ) -> Vec<Effect> {
        d.handle(Event::HotkeyDown, t0);
        d.handle(Event::HotkeyUp, t0 + held);
        d.handle(Event::RecordingStopped(recording), t0 + held)
    }

    fn hello() -> Transcript {
        Transcript::new("hello")
    }

    // Hotkey lifecycle

    #[test]
    fn capturing_since_is_the_first_audio_not_the_hotkey_down() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        assert_eq!(d.capturing_since(), None);
        d.handle(Event::HotkeyDown, t0);
        assert_eq!(d.capturing_since(), None, "microphone still starting");
        let first_audio = t0 + Duration::from_millis(90);
        d.handle(Event::AudioStarted, first_audio);
        assert_eq!(d.capturing_since(), Some(first_audio));
        d.handle(Event::AudioStarted, first_audio + Duration::from_secs(1));
        assert_eq!(
            d.capturing_since(),
            Some(first_audio),
            "only the first audio counts"
        );
        d.handle(Event::HotkeyUp, t0 + Duration::from_secs(2));
        assert_eq!(d.capturing_since(), None);
    }

    #[test]
    fn speaking_since_is_the_first_speech() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        d.handle(Event::HotkeyDown, t0);
        d.handle(Event::AudioStarted, t0);
        assert_eq!(
            d.speaking_since(),
            None,
            "microphone live, but nobody spoke yet"
        );
        let speech = t0 + Duration::from_secs(2);
        d.handle(Event::SpeechStarted, speech);
        d.handle(Event::SpeechStarted, speech + Duration::from_secs(1));
        assert_eq!(
            d.speaking_since(),
            Some(speech),
            "only the first speech counts"
        );
        d.handle(Event::HotkeyUp, t0 + Duration::from_secs(4));
        assert_eq!(d.speaking_since(), None);
    }

    #[test]
    fn silence_before_speech_is_trimmed_keeping_a_preroll() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        d.handle(Event::HotkeyDown, t0);
        let capturing = t0 + Duration::from_millis(100);
        d.handle(Event::AudioStarted, capturing);
        d.handle(
            Event::SpeechStarted,
            capturing + Duration::from_millis(1_500),
        );
        d.handle(Event::HotkeyUp, t0 + Duration::from_secs(3));
        let fx = d.handle(Event::RecordingStopped(loud(2_900)), t0);
        // 1.5 s of lead silence minus the 0.3 s preroll = 1.2 s trimmed.
        assert_eq!(fx, [Effect::Transcribe(loud(1_700))]);
    }

    #[test]
    fn nothing_is_trimmed_without_detected_speech() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        d.handle(Event::HotkeyDown, t0);
        d.handle(Event::AudioStarted, t0);
        d.handle(Event::HotkeyUp, t0 + Duration::from_secs(1));
        let fx = d.handle(Event::RecordingStopped(loud(1_000)), t0);
        assert_eq!(fx, [Effect::Transcribe(loud(1_000))]);
    }

    #[test]
    fn speech_within_the_preroll_trims_nothing() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        d.handle(Event::HotkeyDown, t0);
        d.handle(Event::AudioStarted, t0);
        d.handle(Event::SpeechStarted, t0 + Duration::from_millis(200));
        d.handle(Event::HotkeyUp, t0 + Duration::from_secs(1));
        let fx = d.handle(Event::RecordingStopped(loud(1_000)), t0);
        assert_eq!(fx, [Effect::Transcribe(loud(1_000))]);
    }

    #[test]
    fn caps_count_from_hotkey_down_not_first_audio() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        d.handle(Event::HotkeyDown, t0);
        d.handle(Event::AudioStarted, t0 + Duration::from_secs(1));
        assert_eq!(d.poll_timeout(), Some(t0 + SOFT_CAP));
    }

    #[test]
    fn late_audio_started_is_ignored() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        assert_eq!(d.handle(Event::AudioStarted, t0), []);
        d.handle(Event::HotkeyDown, t0);
        d.handle(Event::HotkeyUp, t0);
        assert_eq!(d.handle(Event::AudioStarted, t0), []);
        assert_eq!(d.state(), State::Transcribing);
    }

    #[test]
    fn hotkey_down_starts_recording_and_esc_tap() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        assert_eq!(d.state(), State::Idle);
        let fx = d.handle(Event::HotkeyDown, t0);
        assert_eq!(fx, [Effect::StartRecording, Effect::StartEscTap]);
        assert_eq!(d.state(), State::Recording);
    }

    #[test]
    fn hotkey_up_stops_esc_tap_and_recording() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        d.handle(Event::HotkeyDown, t0);
        let fx = d.handle(Event::HotkeyUp, t0 + Duration::from_secs(1));
        assert_eq!(fx, [Effect::StopEscTap, Effect::StopRecording]);
        assert_eq!(d.state(), State::Transcribing);
    }

    #[test]
    fn redundant_hotkey_down_is_ignored() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        d.handle(Event::HotkeyDown, t0);
        assert_eq!(d.handle(Event::HotkeyDown, t0), []);
        assert_eq!(d.state(), State::Recording);
    }

    #[test]
    fn hotkey_up_when_idle_is_ignored() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        assert_eq!(d.handle(Event::HotkeyUp, t0), []);
        assert_eq!(d.state(), State::Idle);
    }

    #[test]
    fn hotkey_down_while_transcribing_is_ignored() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        dictate(&mut d, t0, Duration::from_secs(1), loud(1_000));
        assert_eq!(d.handle(Event::HotkeyDown, t0), []);
        assert_eq!(d.state(), State::Transcribing);
    }

    #[test]
    fn recording_failed_to_start_returns_to_idle() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        d.handle(Event::HotkeyDown, t0);
        assert_eq!(
            d.handle(Event::RecordingFailedToStart, t0),
            [Effect::StopEscTap]
        );
        assert_eq!(d.state(), State::Idle);
        assert_eq!(
            d.handle(Event::HotkeyUp, t0),
            [],
            "the release of that press is a no-op"
        );
    }

    #[test]
    fn hotkey_down_waits_until_a_cancelled_recording_has_stopped() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        d.handle(Event::HotkeyDown, t0);
        d.handle(Event::EscPressed, t0);
        assert_eq!(d.state(), State::Idle);
        assert_eq!(
            d.handle(Event::HotkeyDown, t0),
            [],
            "recorder still shutting down"
        );
        d.handle(Event::RecordingStopped(loud(1_000)), t0);
        assert_eq!(
            d.handle(Event::HotkeyDown, t0),
            [Effect::StartRecording, Effect::StartEscTap]
        );
    }

    // Happy path

    #[test]
    fn normal_cycle_transcribes_inserts_and_finishes_inserted() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        let recording = loud(1_000);
        let fx = dictate(&mut d, t0, Duration::from_secs(1), recording.clone());
        assert_eq!(fx, [Effect::Transcribe(recording)]);
        assert_eq!(
            d.handle(Event::Transcribed(hello()), t0),
            [Effect::Insert(hello())]
        );
        assert_eq!(
            d.handle(Event::Inserted, t0),
            [Effect::Finished(Outcome::Inserted(hello()))]
        );
        assert_eq!(d.state(), State::Idle);
    }

    // Cancelled

    #[test]
    fn esc_while_holding_hotkey_produces_cancelled() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        d.handle(Event::HotkeyDown, t0);
        let fx = d.handle(Event::EscPressed, t0);
        assert_eq!(
            fx,
            [Effect::StopEscTap, Effect::StopRecording],
            "stops at once"
        );
        assert_eq!(d.state(), State::Idle, "the overlay disappears");
        assert_eq!(
            d.handle(Event::HotkeyUp, t0),
            [],
            "the release finds nothing to stop"
        );
        let fx = d.handle(Event::RecordingStopped(loud(1_000)), t0);
        assert_eq!(
            fx,
            [Effect::Finished(Outcome::Cancelled)],
            "Cancelled beats transcription"
        );
        assert_eq!(d.state(), State::Idle);
    }

    #[test]
    fn device_lost_mid_recording_produces_cancelled() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        d.handle(Event::HotkeyDown, t0);
        let fx = d.handle(Event::DeviceLost, t0);
        assert_eq!(fx, [Effect::StopEscTap, Effect::StopRecording]);
        d.handle(Event::HotkeyUp, t0);
        let fx = d.handle(Event::RecordingStopped(loud(1_000)), t0);
        assert_eq!(fx, [Effect::Finished(Outcome::Cancelled)]);
    }

    #[test]
    fn esc_after_release_does_not_cancel() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        d.handle(Event::HotkeyDown, t0);
        d.handle(Event::HotkeyUp, t0);
        d.handle(Event::EscPressed, t0);
        let fx = d.handle(Event::RecordingStopped(loud(1_000)), t0);
        assert_eq!(fx, [Effect::Transcribe(loud(1_000))]);
    }

    // Dropped

    #[test]
    fn recording_under_200ms_is_dropped() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        let fx = dictate(&mut d, t0, Duration::from_millis(100), loud(100));
        assert_eq!(fx, [Effect::Finished(Outcome::Dropped)]);
        assert_eq!(d.state(), State::Idle);
    }

    #[test]
    fn recording_of_exactly_200ms_is_not_dropped() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        let fx = dictate(&mut d, t0, Duration::from_millis(200), loud(200));
        assert_eq!(fx, [Effect::Transcribe(loud(200))]);
    }

    #[test]
    fn silent_recording_is_dropped_by_silence_guard() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        let fx = dictate(&mut d, t0, Duration::from_secs(1), silent(1_000));
        assert_eq!(fx, [Effect::Finished(Outcome::Dropped)]);
    }

    // Soft and hard cap

    #[test]
    fn poll_timeout_points_at_soft_then_hard_cap() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        assert_eq!(d.poll_timeout(), None);
        d.handle(Event::HotkeyDown, t0);
        assert_eq!(d.poll_timeout(), Some(t0 + SOFT_CAP));
        d.handle_timeout(t0 + SOFT_CAP);
        assert_eq!(d.poll_timeout(), Some(t0 + HARD_CAP));
        d.handle(Event::HotkeyUp, t0 + SOFT_CAP);
        assert_eq!(d.poll_timeout(), None);
    }

    #[test]
    fn soft_cap_turns_state_to_warning_and_keeps_recording() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        d.handle(Event::HotkeyDown, t0);
        assert_eq!(d.handle_timeout(t0 + Duration::from_millis(59_999)), []);
        assert_eq!(d.state(), State::Recording);
        assert_eq!(d.handle_timeout(t0 + SOFT_CAP), []);
        assert_eq!(d.state(), State::SoftCapWarning);
    }

    #[test]
    fn release_after_soft_cap_is_inserted_not_capped() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        d.handle(Event::HotkeyDown, t0);
        d.handle_timeout(t0 + SOFT_CAP);
        d.handle(Event::HotkeyUp, t0 + Duration::from_secs(90));
        d.handle(Event::RecordingStopped(loud(1_000)), t0);
        d.handle(Event::Transcribed(hello()), t0);
        assert_eq!(
            d.handle(Event::Inserted, t0),
            [Effect::Finished(Outcome::Inserted(hello()))]
        );
    }

    #[test]
    fn hard_cap_stops_recording_and_finishes_capped() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        d.handle(Event::HotkeyDown, t0);
        d.handle_timeout(t0 + SOFT_CAP);
        let fx = d.handle_timeout(t0 + HARD_CAP);
        assert_eq!(fx, [Effect::StopEscTap, Effect::StopRecording]);
        assert_eq!(d.state(), State::Transcribing);

        assert_eq!(
            d.handle(Event::HotkeyUp, t0 + HARD_CAP),
            [],
            "late release is a no-op"
        );
        d.handle(Event::RecordingStopped(loud(1_000)), t0);
        d.handle(Event::Transcribed(hello()), t0);
        let fx = d.handle(Event::Inserted, t0);
        assert_eq!(fx, [Effect::Finished(Outcome::Capped(hello()))]);
        assert_eq!(d.state(), State::Idle);
    }

    #[test]
    fn late_timeout_applies_soft_and_hard_cap_together() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        d.handle(Event::HotkeyDown, t0);
        let fx = d.handle_timeout(t0 + HARD_CAP + Duration::from_secs(1));
        assert_eq!(fx, [Effect::StopEscTap, Effect::StopRecording]);
    }

    #[test]
    fn capped_but_insertion_failed_is_notified() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        d.handle(Event::HotkeyDown, t0);
        d.handle_timeout(t0 + HARD_CAP);
        d.handle(Event::RecordingStopped(loud(1_000)), t0);
        d.handle(Event::Transcribed(hello()), t0);
        let fx = d.handle(Event::InsertionFailed, t0);
        assert_eq!(fx, [Effect::Finished(Outcome::Notified(hello()))]);
    }

    #[test]
    fn empty_transcript_is_dropped_not_inserted() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        dictate(&mut d, t0, Duration::from_secs(1), loud(1_000));
        let fx = d.handle(Event::Transcribed(Transcript::new("  ")), t0);
        assert_eq!(fx, [Effect::Finished(Outcome::Dropped)]);
        assert_eq!(d.state(), State::Idle);
    }

    // Failed and Notified

    #[test]
    fn transcription_failed_finishes_failed() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        dictate(&mut d, t0, Duration::from_secs(1), loud(1_000));
        let fx = d.handle(Event::TranscriptionFailed, t0);
        assert_eq!(fx, [Effect::Finished(Outcome::Failed)]);
        assert_eq!(d.state(), State::Idle);
    }

    #[test]
    fn insertion_failed_finishes_notified() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        dictate(&mut d, t0, Duration::from_secs(1), loud(1_000));
        d.handle(Event::Transcribed(hello()), t0);
        let fx = d.handle(Event::InsertionFailed, t0);
        assert_eq!(fx, [Effect::Finished(Outcome::Notified(hello()))]);
    }

    #[test]
    fn stale_results_are_ignored() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        assert_eq!(d.handle(Event::RecordingStopped(loud(1_000)), t0), []);
        assert_eq!(d.handle(Event::Transcribed(hello()), t0), []);
        assert_eq!(d.handle(Event::TranscriptionFailed, t0), []);
        assert_eq!(d.handle(Event::Inserted, t0), []);
        assert_eq!(d.handle(Event::InsertionFailed, t0), []);
        assert_eq!(d.state(), State::Idle);
    }

    // Notification policy

    #[test]
    fn only_capped_notified_and_failed_post_a_notice() {
        assert_eq!(Outcome::Inserted(hello()).notice(), None);
        assert_eq!(Outcome::Cancelled.notice(), None);
        assert_eq!(Outcome::Dropped.notice(), None);
        assert_eq!(Outcome::Capped(hello()).notice(), Some(Notice::CapReached));
        assert_eq!(
            Outcome::Notified(hello()).notice(),
            Some(Notice::PasteFailed)
        );
        assert_eq!(Outcome::Failed.notice(), Some(Notice::TranscriptionFailed));
    }

    #[test]
    fn notice_titles_match_the_product_copy() {
        assert_eq!(Notice::CapReached.title(), "Hit the 5-minute limit");
        assert_eq!(Notice::PasteFailed.title(), "Paste failed");
        assert_eq!(Notice::TranscriptionFailed.title(), "Transcription failed");
    }

    // Model

    #[test]
    fn starts_in_model_loading_and_ignores_hotkey() {
        let t0 = Instant::now();
        let mut d = Dictation::new();
        assert_eq!(d.state(), State::ModelLoading);
        assert_eq!(d.handle(Event::HotkeyDown, t0), []);
        assert_eq!(d.state(), State::ModelLoading);
    }

    #[test]
    fn model_unavailable_ignores_hotkey() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        d.handle(Event::ModelUnavailable, t0);
        assert_eq!(d.handle(Event::HotkeyDown, t0), []);
        assert_eq!(d.handle(Event::HotkeyUp, t0), []);
        assert_eq!(d.state(), State::ModelUnavailable);
    }

    #[test]
    fn model_reload_mid_recording_does_not_strand_the_dictation() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        d.handle(Event::HotkeyDown, t0);
        d.handle(Event::ModelLoading, t0);
        assert_eq!(
            d.handle(Event::HotkeyUp, t0),
            [Effect::StopEscTap, Effect::StopRecording]
        );
        d.handle(Event::RecordingStopped(loud(1_000)), t0);
        d.handle(Event::TranscriptionFailed, t0);
        assert_eq!(d.state(), State::ModelLoading);
    }

    // Paused

    #[test]
    fn toggle_pause_from_idle_and_back() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        d.handle(Event::TogglePause, t0);
        assert_eq!(d.state(), State::Paused);
        d.handle(Event::TogglePause, t0);
        assert_eq!(d.state(), State::Idle);
    }

    #[test]
    fn toggle_pause_is_a_noop_when_model_unavailable() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        d.handle(Event::ModelUnavailable, t0);
        d.handle(Event::TogglePause, t0);
        assert_eq!(d.state(), State::ModelUnavailable);
        d.handle(Event::ModelReady, t0);
        assert_eq!(d.state(), State::Idle, "the no-op toggle did not pause");
    }

    #[test]
    fn toggle_pause_is_a_noop_while_recording() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        d.handle(Event::HotkeyDown, t0);
        d.handle(Event::TogglePause, t0);
        assert_eq!(d.state(), State::Recording);
    }

    #[test]
    fn hotkey_is_ignored_while_paused() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        d.handle(Event::TogglePause, t0);
        assert_eq!(d.handle(Event::HotkeyDown, t0), []);
        assert_eq!(d.handle(Event::HotkeyUp, t0), []);
        assert_eq!(d.state(), State::Paused);
    }

    #[test]
    fn unpause_reenables_hotkey() {
        let t0 = Instant::now();
        let mut d = ready(t0);
        d.handle(Event::TogglePause, t0);
        d.handle(Event::TogglePause, t0);
        assert_eq!(
            d.handle(Event::HotkeyDown, t0),
            [Effect::StartRecording, Effect::StartEscTap]
        );
    }
}
