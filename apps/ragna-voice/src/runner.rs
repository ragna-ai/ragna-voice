//! The one thread that owns the [`Dictation`] state machine and executes its effects
//! (ADR-0002). Everything else talks to it through the [`Runner`] channel.

use std::collections::VecDeque;
use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use ragna_voice_core::language::{self, Language};
use ragna_voice_core::{Dictation, Effect, Event, Notice, Recording, SilenceGuard, State};
use ragna_voice_platform::permissions::{self, Permission, Status};
use ragna_voice_transcribe::Transcriber;

/// What the UI shows, published by the runner. Read it from any thread.
#[derive(Debug)]
pub struct Live {
    snapshot: Mutex<Snapshot>,
    /// Microphone level as `f32` bits, written by the audio thread.
    level: AtomicU32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Snapshot {
    pub state: State,
    /// First audio of the current Recording; `None` while the microphone starts up.
    pub capturing_since: Option<Instant>,
    /// First speech of the current Recording; the overlay's timer counts from here.
    pub speaking_since: Option<Instant>,
}

impl Default for Live {
    fn default() -> Self {
        Self {
            snapshot: Mutex::new(Snapshot {
                state: Dictation::new().state(),
                capturing_since: None,
                speaking_since: None,
            }),
            level: AtomicU32::new(0),
        }
    }
}

impl Live {
    pub fn snapshot(&self) -> Snapshot {
        *self.snapshot.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Microphone level for the waveform, 0.0 to 1.0.
    pub fn level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed))
    }

    fn set_level(&self, level: f32) {
        self.level.store(level.to_bits(), Ordering::Relaxed);
    }

    /// Returns whether anything changed.
    fn publish(&self, snapshot: Snapshot) -> bool {
        let mut current = self.snapshot.lock().unwrap_or_else(PoisonError::into_inner);
        let changed = *current != snapshot;
        *current = snapshot;
        changed
    }
}

/// Maps RMS to a 0..1 meter on a -50..-10 dBFS scale, which spans quiet to loud speech.
fn meter(rms: f32) -> f32 {
    if rms.is_nan() || rms <= 0.0 {
        return 0.0;
    }
    ((20.0 * rms.log10() + 50.0) / 40.0).clamp(0.0, 1.0)
}

/// What the runner needs from the UI thread. Called from the runner thread.
pub trait Ui: Send + 'static {
    /// The [`Live`] snapshot changed: repaint.
    fn changed(&self);
    /// Show Settings, e.g. when a missing permission stopped a Dictation.
    fn attention(&self);
    /// Turn Esc-to-cancel on or off. Hotkeys can only be (un)registered on the main
    /// thread, so the UI does it.
    fn escape(&self, on: bool);
    /// Tell the user something went wrong (a Notification Center notice).
    fn notice(&self, notice: Notice);
}

/// Handle to the runner thread. Cloning it is cheap.
#[derive(Debug, Clone)]
pub struct Runner {
    tx: Sender<(Event, Instant)>,
    jobs: Sender<Job>,
    /// The transcription language, set from Settings.
    language: Arc<Mutex<Language>>,
}

impl Runner {
    /// Spawns the runner thread. It publishes to `live` and reports to `ui`.
    ///
    /// # Errors
    ///
    /// The OS refused to spawn the thread.
    pub fn spawn(live: Arc<Live>, ui: impl Ui) -> std::io::Result<Self> {
        let (tx, rx) = mpsc::channel::<(Event, Instant)>();
        let transcriber = spawn_transcriber(tx.clone())?;
        let runner = Self {
            tx,
            jobs: transcriber.clone(),
            language: Arc::new(Mutex::new(language::AUTOMATIC)),
        };
        let handle = runner.clone();
        thread::Builder::new()
            .name("dictation".into())
            .spawn(move || {
                let mut exec = Executor {
                    runner: handle,
                    dictation: Dictation::new(),
                    recording: None,
                    transcriber,
                    live,
                    ui: Box::new(ui),
                };
                loop {
                    let next = match exec.dictation.poll_timeout() {
                        Some(deadline) => {
                            match rx
                                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                            {
                                Ok(msg) => Some(msg),
                                Err(RecvTimeoutError::Timeout) => None,
                                Err(RecvTimeoutError::Disconnected) => break,
                            }
                        }
                        None => match rx.recv() {
                            Ok(msg) => Some(msg),
                            Err(_) => break,
                        },
                    };
                    if let Some((event, at)) = next {
                        exec.run(event, at);
                    } else {
                        let effects = exec.dictation.handle_timeout(Instant::now());
                        exec.execute(effects);
                    }
                }
            })?;
        Ok(runner)
    }

    /// Loads `model`, replacing the current one; `None` means there is no Model to load.
    /// The Hotkey does nothing until the load finishes.
    pub fn load_model(&self, model: Option<PathBuf>) {
        let now = Instant::now();
        match model {
            Some(path) => {
                self.send(Event::ModelLoading, now);
                if self.jobs.send(Job::Load(path)).is_err() {
                    self.send(Event::ModelUnavailable, now);
                }
            }
            None => self.send(Event::ModelUnavailable, now),
        }
    }

    /// Frees the Model before the process exits. Call when quitting: if a whisper.cpp
    /// context is still alive when C++ static destructors run, ggml's Metal backend
    /// aborts the process, and every Quit becomes a crash report. Waits for a
    /// transcription in progress, up to `timeout`.
    pub fn shutdown(&self, timeout: Duration) {
        let (done, acked) = mpsc::channel();
        if self.jobs.send(Job::Shutdown(done)).is_ok() && acked.recv_timeout(timeout).is_err() {
            log::warn!("the Model was not freed within {timeout:?}; quitting anyway");
        }
    }

    /// The language for the next transcriptions. English-only Models ignore it.
    pub fn set_language(&self, language: Language) {
        *self.language.lock().unwrap_or_else(PoisonError::into_inner) = language;
    }

    fn language(&self) -> Language {
        *self.language.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Reports an event that happened at `at`. Never blocks.
    pub fn send(&self, event: Event, at: Instant) {
        if self.tx.send((event, at)).is_err() {
            log::error!("dictation runner is gone; event dropped");
        }
    }
}

/// Work for the transcriber thread, done in order.
enum Job {
    /// Replace the current Model (or load the first one).
    Load(PathBuf),
    Transcribe(Recording, Language),
    /// Free the Model and stop, then acknowledge. See [`Runner::shutdown`].
    Shutdown(Sender<()>),
}

/// Loads Models and transcribes Recordings on its own thread, so neither blocks the
/// Hotkey or the cap timers. Reports back as `events`.
fn spawn_transcriber(events: Sender<(Event, Instant)>) -> std::io::Result<Sender<Job>> {
    let (jobs, rx) = mpsc::channel::<Job>();
    thread::Builder::new()
        .name("transcriber".into())
        .spawn(move || {
            let mut transcriber: Option<Transcriber> = None;
            for job in rx {
                let event = match job {
                    Job::Shutdown(done) => {
                        drop(transcriber.take());
                        let _ = done.send(());
                        break;
                    }
                    Job::Load(path) => {
                        // Free the old Model first: two large ones may not fit in memory.
                        transcriber = None;
                        let started = Instant::now();
                        match panic::catch_unwind(|| Transcriber::load(&path)) {
                            Ok(Ok(loaded)) => {
                                log::info!(
                                    "Model {} loaded in {:?}",
                                    path.display(),
                                    started.elapsed()
                                );
                                transcriber = Some(loaded);
                                Event::ModelReady
                            }
                            Ok(Err(e)) => {
                                log::error!("could not load the Model: {e}");
                                Event::ModelUnavailable
                            }
                            Err(_) => {
                                log::error!("loading the Model panicked");
                                Event::ModelUnavailable
                            }
                        }
                    }
                    Job::Transcribe(recording, language) => {
                        transcribe(transcriber.as_mut(), &recording, language)
                    }
                };
                if events.send((event, Instant::now())).is_err() {
                    break; // The runner is gone: the app is shutting down.
                }
            }
        })?;
    Ok(jobs)
}

fn transcribe(
    transcriber: Option<&mut Transcriber>,
    recording: &Recording,
    language: Language,
) -> Event {
    let Some(transcriber) = transcriber else {
        log::error!("no Model loaded");
        return Event::TranscriptionFailed;
    };
    let started = Instant::now();
    // A panic must end this Dictation as Failed, not strand it in Transcribing.
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        transcriber.transcribe(recording, language)
    }));
    log::info!(
        "transcribed {:?} of audio in {:?}",
        recording.duration(),
        started.elapsed()
    );
    match result {
        Ok(Ok(transcript)) => Event::Transcribed(transcript),
        Ok(Err(e)) => {
            log::error!("transcription failed: {e}");
            Event::TranscriptionFailed
        }
        Err(_) => {
            log::error!("transcription panicked");
            Event::TranscriptionFailed
        }
    }
}

struct Executor {
    runner: Runner,
    dictation: Dictation,
    recording: Option<ragna_voice_audio::ActiveRecording>,
    transcriber: Sender<Job>,
    live: Arc<Live>,
    ui: Box<dyn Ui>,
}

impl Executor {
    fn run(&mut self, event: Event, at: Instant) {
        log::debug!("event {event:?}");
        let effects = self.dictation.handle(event, at);
        self.execute(effects);
    }

    /// Runs effects in order. Results that come back synchronously are fed in at once.
    fn execute(&mut self, effects: Vec<Effect>) {
        let mut queue = VecDeque::from(effects);
        while let Some(effect) = queue.pop_front() {
            log::debug!("effect {effect:?}");
            if let Some(result) = self.perform(effect) {
                queue.extend(self.dictation.handle(result, Instant::now()));
            }
        }
        let snapshot = Snapshot {
            state: self.dictation.state(),
            capturing_since: self.dictation.capturing_since(),
            speaking_since: self.dictation.speaking_since(),
        };
        if self.live.publish(snapshot) {
            log::debug!("state {:?}", snapshot.state);
            self.ui.changed();
        }
    }

    fn perform(&mut self, effect: Effect) -> Option<Event> {
        match effect {
            Effect::StartRecording => {
                // Without access macOS feeds silence, which would end as a silent Dropped.
                let microphone = Permission::Microphone.status();
                if microphone != Status::Granted {
                    log::warn!("not recording: microphone access is {microphone:?}");
                    if microphone == Status::NotAsked {
                        permissions::request_microphone(|_| {});
                    }
                    self.ui.attention();
                    return Some(Event::RecordingFailedToStart);
                }
                let (lost, events) = (self.runner.clone(), self.runner.clone());
                let live = Arc::clone(&self.live);
                let guard = SilenceGuard::default();
                let (mut first_audio, mut first_speech) = (true, true);
                match ragna_voice_audio::start(
                    move || lost.send(Event::DeviceLost, Instant::now()),
                    move |buffer| {
                        if std::mem::take(&mut first_audio) {
                            events.send(Event::AudioStarted, Instant::now());
                        }
                        if first_speech && guard.is_speech(buffer) {
                            first_speech = false;
                            events.send(Event::SpeechStarted, Instant::now());
                        }
                        live.set_level(meter(ragna_voice_audio::rms(buffer)));
                    },
                ) {
                    Ok(recording) => {
                        self.recording = Some(recording);
                        None
                    }
                    Err(e) => {
                        log::error!("could not start recording: {e}");
                        Some(Event::RecordingFailedToStart)
                    }
                }
            }
            Effect::StopRecording => {
                self.live.set_level(0.0);
                let recording = match self
                    .recording
                    .take()
                    .map(ragna_voice_audio::ActiveRecording::stop)
                {
                    Some(Ok(recording)) => recording,
                    Some(Err(e)) => {
                        log::error!("could not finish recording: {e}");
                        Recording::default()
                    }
                    None => Recording::default(),
                };
                Some(Event::RecordingStopped(recording))
            }
            Effect::StartEscTap => {
                self.ui.escape(true);
                None
            }
            Effect::StopEscTap => {
                self.ui.escape(false);
                None
            }
            // The result comes back from the transcriber thread as an event.
            Effect::Transcribe(recording) => self
                .transcriber
                .send(Job::Transcribe(recording, self.runner.language()))
                .is_err()
                .then_some(Event::TranscriptionFailed),
            // Runs on its own thread and reports back like the transcriber.
            Effect::Insert(transcript) => {
                let runner = self.runner.clone();
                let spawned = thread::Builder::new().name("insert".into()).spawn(move || {
                    let event = match ragna_voice_platform::insert::insert(transcript.text()) {
                        Ok(()) => Event::Inserted,
                        Err(e) => {
                            log::warn!("insertion failed, Transcript left on the clipboard: {e}");
                            Event::InsertionFailed
                        }
                    };
                    runner.send(event, Instant::now());
                });
                spawned
                    .inspect_err(|e| log::error!("could not start insertion: {e}"))
                    .is_err()
                    .then_some(Event::InsertionFailed)
            }
            Effect::Finished(outcome) => {
                log::info!("dictation finished: {outcome:?}");
                if let Some(notice) = outcome.notice() {
                    log::info!("notice: {}: {}", notice.title(), notice.body());
                    self.ui.notice(notice);
                }
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meter_spans_quiet_to_loud_speech() {
        assert!(meter(0.0).abs() < f32::EPSILON);
        assert!(meter(-1.0).abs() < f32::EPSILON);
        assert!(
            meter(0.001).abs() < f32::EPSILON,
            "-60 dBFS is below the scale"
        );
        assert!((meter(0.1) - 0.75).abs() < 1e-3, "-20 dBFS");
        assert!((meter(1.0) - 1.0).abs() < f32::EPSILON);
        assert!(
            meter(f32::NAN).abs() < f32::EPSILON,
            "corrupt input reads as silence"
        );
    }
}
