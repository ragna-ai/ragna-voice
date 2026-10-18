//! OS integration: Hotkey, Esc cancel, Insertion, permissions, notifications.

#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]
// OS APIs without safe wrappers live here and nowhere else (AGENTS.md §4).
#![allow(unsafe_code)]

pub mod app;
pub mod hotkey;
pub mod insert;
pub mod locale;
pub mod notify;
pub mod overlay;
pub mod permissions;
