//! egui views: Settings, first-run wizard, waveform overlay.

#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

pub mod i18n;
pub mod models;
pub mod overlay;
pub mod settings;
mod theme;

pub use theme::install;
