//! Insertion: puts a Transcript into the Target App with the clipboard and a synthesised
//! ⌘V. The Transcript stays on the clipboard afterwards: restoring the previous contents
//! made Chrome's web pages drop the paste now and then (it fires `paste` and
//! `beforeinput`, then inserts nothing), even with the restore a second later.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// No Accessibility access: ⌘V can't be synthesised. The Transcript is on the clipboard.
    AccessibilityDenied,
    ClipboardWrite,
    KeyEvent,
    Unsupported,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::AccessibilityDenied => "Accessibility access is not granted",
            Self::ClipboardWrite => "could not write the clipboard",
            Self::KeyEvent => "could not create the ⌘V key event",
            Self::Unsupported => "Insertion is not supported on this platform",
        })
    }
}

impl std::error::Error for Error {}

/// Pastes `text` into the frontmost app, leaving it on the clipboard.
///
/// # Errors
///
/// On any error `text` is left on the clipboard, so the user can paste it by hand.
pub fn insert(text: &str) -> Result<(), Error> {
    imp::insert(text)
}

#[cfg(target_os = "macos")]
mod imp {
    use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
    use objc2_core_graphics::{CGEvent, CGEventFlags, CGEventTapLocation, CGKeyCode};
    use objc2_foundation::NSString;

    use super::Error;
    use crate::permissions::{Permission, Status};

    /// `kVK_ANSI_V`.
    const V: CGKeyCode = 0x09;

    pub fn insert(text: &str) -> Result<(), Error> {
        let pasteboard = NSPasteboard::generalPasteboard();
        pasteboard.clearContents();
        // SAFETY: reading an immutable framework constant.
        let string_type = unsafe { NSPasteboardTypeString };
        if !pasteboard.setString_forType(&NSString::from_str(text), string_type) {
            return Err(Error::ClipboardWrite);
        }

        if Permission::Accessibility.status() != Status::Granted {
            return Err(Error::AccessibilityDenied);
        }
        post_cmd_v()
    }

    fn post_cmd_v() -> Result<(), Error> {
        let down = CGEvent::new_keyboard_event(None, V, true).ok_or(Error::KeyEvent)?;
        let up = CGEvent::new_keyboard_event(None, V, false).ok_or(Error::KeyEvent)?;
        // Only ⌘, even if the user is still holding the Hotkey's modifiers.
        CGEvent::set_flags(Some(&down), CGEventFlags::MaskCommand);
        CGEvent::set_flags(Some(&up), CGEventFlags::MaskCommand);
        CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&down));
        CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&up));
        Ok(())
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::Error;

    pub fn insert(_text: &str) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
}
