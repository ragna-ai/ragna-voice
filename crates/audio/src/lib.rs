//! Microphone capture into an in-RAM Recording.

#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use ragna_voice_core::{Recording, SAMPLE_RATE};
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Resampler};

#[derive(Debug)]
pub enum Error {
    NoInputDevice,
    UnsupportedSampleFormat(SampleFormat),
    Device(cpal::Error),
    Resample(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoInputDevice => write!(f, "no microphone found"),
            Self::UnsupportedSampleFormat(format) => {
                write!(f, "unsupported microphone sample format {format:?}")
            }
            Self::Device(e) => write!(f, "microphone error: {e}"),
            Self::Resample(e) => write!(f, "resampling failed: {e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<cpal::Error> for Error {
    fn from(e: cpal::Error) -> Self {
        Self::Device(e)
    }
}

/// Capture in progress from the default microphone. Samples stay in RAM.
///
/// The cpal stream is not `Send` on every platform: stop it on the thread that started it.
pub struct ActiveRecording {
    stream: cpal::Stream,
    samples: Arc<Mutex<Vec<f32>>>,
    channels: u16,
    sample_rate: u32,
}

impl fmt::Debug for ActiveRecording {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ActiveRecording")
            .field("channels", &self.channels)
            .field("sample_rate", &self.sample_rate)
            .finish_non_exhaustive()
    }
}

/// Starts capturing from the default input device in its native format.
/// `on_device_lost` runs on an audio thread if the microphone disappears; `on_buffer`
/// sees every captured buffer (interleaved, native rate), e.g. for a level meter.
/// Both run on the audio thread: keep them short.
///
/// # Errors
///
/// No microphone, an unsupported sample format, or a stream the OS refuses to start.
pub fn start(
    on_device_lost: impl FnMut() + Send + 'static,
    on_buffer: impl FnMut(&[f32]) + Send + 'static,
) -> Result<ActiveRecording, Error> {
    let device = cpal::default_host()
        .default_input_device()
        .ok_or(Error::NoInputDevice)?;
    let config = device.default_input_config()?;
    let samples = Arc::new(Mutex::new(Vec::new()));
    let stream = match config.sample_format() {
        SampleFormat::F32 => build::<f32>(&device, &config, &samples, on_device_lost, on_buffer)?,
        SampleFormat::I16 => build::<i16>(&device, &config, &samples, on_device_lost, on_buffer)?,
        SampleFormat::I32 => build::<i32>(&device, &config, &samples, on_device_lost, on_buffer)?,
        other => return Err(Error::UnsupportedSampleFormat(other)),
    };
    stream.play()?;
    Ok(ActiveRecording {
        stream,
        samples,
        channels: config.channels(),
        sample_rate: config.sample_rate(),
    })
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    samples: &Arc<Mutex<Vec<f32>>>,
    mut on_device_lost: impl FnMut() + Send + 'static,
    mut on_buffer: impl FnMut(&[f32]) + Send + 'static,
) -> Result<cpal::Stream, Error>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let sink = Arc::clone(samples);
    let stream = device.build_input_stream::<T, _, _>(
        config.config(),
        move |data: &[T], _| {
            let mut sink = sink.lock().unwrap_or_else(PoisonError::into_inner);
            let start = sink.len();
            sink.extend(data.iter().map(|&s| s.to_sample::<f32>()));
            on_buffer(sink.get(start..).unwrap_or_default());
        },
        move |e: cpal::Error| {
            if e.kind() == cpal::ErrorKind::DeviceNotAvailable {
                on_device_lost();
            }
        },
        None,
    )?;
    Ok(stream)
}

impl ActiveRecording {
    /// Stops capture and converts what was captured into a 16 kHz mono Recording.
    ///
    /// # Errors
    ///
    /// The resampler rejected the device's sample rate.
    pub fn stop(self) -> Result<Recording, Error> {
        drop(self.stream);
        let samples =
            std::mem::take(&mut *self.samples.lock().unwrap_or_else(PoisonError::into_inner));
        to_16k_mono(&samples, self.channels, self.sample_rate)
    }
}

/// Root mean square of `samples`: 0.0 for silence (or no samples), up to 1.0.
#[must_use]
pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    #[allow(clippy::cast_precision_loss)]
    let n = samples.len() as f32;
    let mean_square = samples.iter().map(|s| s * s).sum::<f32>() / n;
    if mean_square.is_finite() {
        mean_square.sqrt().min(1.0)
    } else {
        0.0
    }
}

/// Downmixes interleaved audio to mono and resamples it to [`SAMPLE_RATE`].
///
/// # Errors
///
/// A zero channel count or sample rate, or a resampler failure.
pub fn to_16k_mono(
    interleaved: &[f32],
    channels: u16,
    sample_rate: u32,
) -> Result<Recording, Error> {
    if channels == 0 || sample_rate == 0 {
        return Err(Error::Resample(format!(
            "invalid input: {channels} channels at {sample_rate} Hz"
        )));
    }
    let mono: Vec<f32> = interleaved
        .chunks_exact(usize::from(channels))
        .map(|frame| frame.iter().sum::<f32>() / f32::from(channels))
        .collect();
    if sample_rate == SAMPLE_RATE || mono.is_empty() {
        return Ok(Recording::new(mono));
    }
    let resample = |e: &dyn fmt::Display| Error::Resample(e.to_string());
    let from = usize::try_from(sample_rate).map_err(|e| resample(&e))?;
    let to = usize::try_from(SAMPLE_RATE).map_err(|e| resample(&e))?;
    let mut resampler =
        Fft::<f32>::new(from, to, 1024, 1, FixedSync::Input).map_err(|e| resample(&e))?;
    let input = InterleavedSlice::new(&mono, 1, mono.len()).map_err(|e| resample(&e))?;
    let output = resampler
        .process_all(&input, mono.len(), None)
        .map_err(|e| resample(&e))?;
    Ok(Recording::new(output.take_data()))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    /// One second of a 440 Hz sine at `rate`, interleaved over `channels`.
    fn sine(rate: u32, channels: u16) -> Vec<f32> {
        (0..rate)
            .flat_map(|i| {
                let t = f64::from(i) / f64::from(rate);
                #[allow(clippy::cast_possible_truncation)]
                let s = ((t * 440.0 * std::f64::consts::TAU).sin() * 0.5) as f32;
                std::iter::repeat_n(s, usize::from(channels))
            })
            .collect()
    }

    fn assert_about_one_second(recording: &Recording) {
        let d = recording.duration();
        assert!(
            d > Duration::from_millis(990) && d < Duration::from_millis(1_010),
            "{d:?}"
        );
    }

    #[test]
    fn mono_16k_passes_through_unchanged() {
        let input = sine(16_000, 1);
        let recording = to_16k_mono(&input, 1, 16_000).unwrap();
        assert_eq!(recording.samples(), input.as_slice());
    }

    #[test]
    fn stereo_is_downmixed_by_averaging() {
        let recording = to_16k_mono(&[0.2, 0.4, -1.0, 1.0], 2, 16_000).unwrap();
        assert_eq!(recording.samples().len(), 2);
        assert!((recording.samples()[0] - 0.3).abs() < 1e-6);
        assert!(recording.samples()[1].abs() < 1e-6);
    }

    #[test]
    fn stereo_48k_becomes_one_second_of_16k_mono() {
        let recording = to_16k_mono(&sine(48_000, 2), 2, 48_000).unwrap();
        assert_about_one_second(&recording);
        let peak = recording
            .samples()
            .iter()
            .fold(0.0_f32, |p, s| p.max(s.abs()));
        assert!(
            peak > 0.4 && peak < 0.6,
            "amplitude survives resampling: {peak}"
        );
    }

    #[test]
    fn mono_44_1k_becomes_one_second_of_16k_mono() {
        assert_about_one_second(&to_16k_mono(&sine(44_100, 1), 1, 44_100).unwrap());
    }

    #[test]
    fn empty_input_is_an_empty_recording() {
        assert_eq!(
            to_16k_mono(&[], 2, 48_000).unwrap().duration(),
            Duration::ZERO
        );
    }

    #[test]
    fn rms_of_silence_and_of_a_full_scale_square_wave() {
        assert!(rms(&[]).abs() < f32::EPSILON);
        assert!(rms(&[0.0; 64]).abs() < f32::EPSILON);
        assert!((rms(&[1.0, -1.0, 1.0, -1.0]) - 1.0).abs() < 1e-6);
        assert!(
            rms(&[f32::NAN; 4]).abs() < f32::EPSILON,
            "corrupt samples read as silence"
        );
    }

    #[test]
    fn zero_channels_or_rate_is_an_error_not_a_panic() {
        assert!(to_16k_mono(&[0.1], 0, 48_000).is_err());
        assert!(to_16k_mono(&[0.1], 1, 0).is_err());
    }
}
