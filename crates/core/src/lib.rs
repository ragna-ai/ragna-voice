//! Dictation domain model and state machine.
//!
//! This crate is pure: no OS, audio, Model or UI dependencies. The vocabulary
//! follows `CONTEXT.md` (Dictation, Recording, Transcript, Dictation outcome).

#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

pub mod dictation;
pub mod language;
mod recording;
mod silence_guard;

pub use dictation::{Dictation, Effect, Event, Notice, Outcome, State};
pub use language::Language;
pub use recording::{Recording, SAMPLE_RATE, Transcript};
pub use silence_guard::SilenceGuard;
