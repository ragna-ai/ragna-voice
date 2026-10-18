//! Headless RAGNA Voice pipeline.
//!
//! `ragna-voice-cli transcribe <file.wav>` runs one Dictation through the same state
//! machine as the app, with the WAV file standing in for the microphone. The Model
//! comes from `RAGNA_VOICE_MODEL`. The Transcript goes to stdout, the outcome to stderr.

#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

use std::error::Error;
use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

use ragna_voice_core::{Dictation, Effect, Event, Outcome, Recording};
use ragna_voice_transcribe::Transcriber;

const USAGE: &str = "usage: ragna-voice-cli transcribe <file.wav>

The Model is read from RAGNA_VOICE_MODEL (a whisper.cpp ggml-*.bin file).";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [command, wav] = args.as_slice() else {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    };
    if command != "transcribe" {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    }
    match transcribe(Path::new(wav)) {
        Ok(Outcome::Inserted(transcript)) => {
            println!("{}", transcript.text());
            ExitCode::SUCCESS
        }
        Ok(outcome) => {
            eprintln!("no Transcript: {outcome:?}");
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn transcribe(wav: &Path) -> Result<Outcome, Box<dyn Error>> {
    let model = std::env::var_os("RAGNA_VOICE_MODEL").ok_or("RAGNA_VOICE_MODEL is not set")?;
    let started = Instant::now();
    let mut transcriber = Transcriber::load(Path::new(&model))?;
    eprintln!("Model loaded in {:?}", started.elapsed());
    let mut recording = Some(read_wav(wav)?);

    let mut dictation = Dictation::new();
    let mut queue = vec![Event::ModelReady, Event::HotkeyDown, Event::HotkeyUp];
    queue.reverse();
    while let Some(event) = queue.pop() {
        for effect in dictation.handle(event, Instant::now()) {
            match effect {
                Effect::StopRecording => {
                    queue.push(Event::RecordingStopped(
                        recording.take().unwrap_or_default(),
                    ));
                }
                Effect::Transcribe(recording) => {
                    let started = Instant::now();
                    queue.push(
                        match transcriber
                            .transcribe(&recording, ragna_voice_core::language::AUTOMATIC)
                        {
                            Ok(transcript) => Event::Transcribed(transcript),
                            Err(e) => {
                                eprintln!("transcription failed: {e}");
                                Event::TranscriptionFailed
                            }
                        },
                    );
                    eprintln!(
                        "transcribed {:?} of audio in {:?}",
                        recording.duration(),
                        started.elapsed()
                    );
                }
                // Headless: "inserting" means printing to stdout, which can't fail.
                Effect::Insert(_) => queue.push(Event::Inserted),
                Effect::Finished(outcome) => return Ok(outcome),
                Effect::StartRecording | Effect::StartEscTap | Effect::StopEscTap => {}
            }
        }
    }
    Err("the Dictation did not finish".into())
}

fn read_wav(path: &Path) -> Result<Recording, Box<dyn Error>> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>()?,
        hound::SampleFormat::Int => {
            let scale = 2f32.powi(i32::from(spec.bits_per_sample.saturating_sub(1)));
            reader
                .samples::<i32>()
                .map(|s| {
                    s.map(|s| {
                        #[allow(clippy::cast_precision_loss)]
                        let s = s as f32;
                        s / scale
                    })
                })
                .collect::<Result<_, _>>()?
        }
    };
    Ok(ragna_voice_audio::to_16k_mono(
        &samples,
        spec.channels,
        spec.sample_rate,
    )?)
}
