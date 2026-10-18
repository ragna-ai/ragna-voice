//! Permissions. macOS sends no notification when a permission changes, so callers poll.
//!
//! Grants are tied to the app's code signature: run the bundled app (`cargo xtask run`),
//! not the bare binary, or macOS attributes them to the terminal. Ad-hoc signed builds
//! get a new signature on every rebuild, which drops every grant (see `CLAUDE.md`).

use std::io;
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    /// Needed to record.
    Microphone,
    /// Needed to paste with a synthesised ⌘V. Without it the Transcript stays on the clipboard.
    Accessibility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Granted,
    /// The user has not been asked yet.
    NotAsked,
    /// Denied, restricted, or (for Accessibility) not granted. macOS can't tell
    /// "never asked" from "denied" for Accessibility.
    Denied,
}

impl Permission {
    #[must_use]
    pub fn status(self) -> Status {
        match self {
            Self::Microphone => imp::microphone(),
            Self::Accessibility => imp::accessibility(),
        }
    }

    /// The `tccutil` service name.
    fn service(self) -> &'static str {
        match self {
            Self::Microphone => "Microphone",
            Self::Accessibility => "Accessibility",
        }
    }

    fn settings_url(self) -> &'static str {
        match self {
            Self::Microphone => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
            }
            Self::Accessibility => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
            }
        }
    }

    /// Opens this permission's page in System Settings.
    ///
    /// # Errors
    ///
    /// `open` could not be started.
    pub fn open_settings(self) -> io::Result<()> {
        Command::new("open")
            .arg(self.settings_url())
            .spawn()
            .map(drop)
    }

    /// Clears this app's entry for the permission, so the next request prompts again.
    /// This is the fix for a stale grant that System Settings shows as on but macOS no
    /// longer honours (it belonged to a previous signature).
    ///
    /// # Errors
    ///
    /// `tccutil` could not run or rejected the reset.
    pub fn reset(self, bundle_id: &str) -> io::Result<()> {
        // Never call tccutil without a bundle id: that resets the permission for every app.
        if bundle_id.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "empty bundle id",
            ));
        }
        let status = Command::new("tccutil")
            .args(["reset", self.service(), bundle_id])
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(io::Error::other(format!("tccutil reset failed: {status}")))
        }
    }
}

/// Shows the system microphone prompt if the user hasn't been asked yet.
/// `on_done(granted)` runs on an arbitrary thread once the user answers (or at once,
/// if they answered before).
pub fn request_microphone(on_done: impl Fn(bool) + Send + 'static) {
    imp::request_microphone(on_done);
}

/// Shows the system Accessibility prompt, which adds the app to the list in
/// System Settings and offers to open it. Returns the current status.
#[must_use]
pub fn request_accessibility() -> Status {
    imp::request_accessibility()
}

#[cfg(target_os = "macos")]
mod imp {
    use block2::RcBlock;
    use objc2::runtime::Bool;
    use objc2_application_services::{
        AXIsProcessTrusted, AXIsProcessTrustedWithOptions, kAXTrustedCheckOptionPrompt,
    };
    use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};
    use objc2_core_foundation::{CFBoolean, CFDictionary, CFString};

    use super::Status;

    fn audio() -> Option<&'static objc2_av_foundation::AVMediaType> {
        // SAFETY: reading an immutable framework constant.
        unsafe { AVMediaTypeAudio }
    }

    pub fn microphone() -> Status {
        let Some(audio) = audio() else {
            return Status::Denied;
        };
        // SAFETY: `audio` is AVMediaTypeAudio, one of the two media types this accepts
        // (any other throws).
        let status = unsafe { AVCaptureDevice::authorizationStatusForMediaType(audio) };
        match status {
            AVAuthorizationStatus::Authorized => Status::Granted,
            AVAuthorizationStatus::NotDetermined => Status::NotAsked,
            _ => Status::Denied,
        }
    }

    pub fn request_microphone(on_done: impl Fn(bool) + Send + 'static) {
        let Some(audio) = audio() else {
            on_done(false);
            return;
        };
        let handler = RcBlock::new(move |granted: Bool| on_done(granted.as_bool()));
        // SAFETY: valid media type (see `microphone`); AVFoundation copies the block and
        // calls it once on its own queue, which `on_done: Send` allows.
        unsafe { AVCaptureDevice::requestAccessForMediaType_completionHandler(audio, &handler) };
    }

    pub fn accessibility() -> Status {
        // SAFETY: plain C call with no arguments and no preconditions.
        if unsafe { AXIsProcessTrusted() } {
            Status::Granted
        } else {
            Status::Denied
        }
    }

    pub fn request_accessibility() -> Status {
        // SAFETY: reading immutable framework constants.
        let (key, yes) = unsafe {
            (
                kAXTrustedCheckOptionPrompt,
                objc2_core_foundation::kCFBooleanTrue,
            )
        };
        let Some(yes) = yes else {
            return accessibility();
        };
        let options = CFDictionary::<CFString, CFBoolean>::from_slices(&[key], &[yes]);
        // SAFETY: the options dictionary maps CFString keys to CFBoolean values, as the
        // API requires.
        if unsafe { AXIsProcessTrustedWithOptions(Some(options.as_opaque())) } {
            Status::Granted
        } else {
            Status::Denied
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::Status;

    pub fn microphone() -> Status {
        Status::Granted
    }

    pub fn request_microphone(on_done: impl Fn(bool) + Send + 'static) {
        on_done(true);
    }

    pub fn accessibility() -> Status {
        Status::Granted
    }

    pub fn request_accessibility() -> Status {
        Status::Granted
    }
}
