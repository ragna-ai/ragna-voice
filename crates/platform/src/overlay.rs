//! The waveform overlay's window: floats above other apps on every Space, including
//! full-screen ones, and never takes clicks. winit can't express these, so they are set
//! on the underlying `NSWindow`.

use raw_window_handle::HasWindowHandle;

/// Turns `window` into an overlay. Call once, on the main thread, after it exists.
/// Returns `false` if the window could not be configured (it then behaves like a
/// normal always-on-top window).
pub fn configure(window: &impl HasWindowHandle) -> bool {
    imp::configure(window)
}

#[cfg(target_os = "macos")]
mod imp {
    use objc2_app_kit::{NSStatusWindowLevel, NSView, NSWindowCollectionBehavior};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    pub fn configure(window: &impl HasWindowHandle) -> bool {
        let Ok(handle) = window.window_handle() else {
            return false;
        };
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return false;
        };
        // SAFETY: winit's AppKit handle points at the window's content NSView, which is
        // alive while the borrowed handle is, and we are on the main thread.
        let view: &NSView = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
        let Some(window) = view.window() else {
            return false;
        };
        window.setLevel(NSStatusWindowLevel);
        window.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::IgnoresCycle
                | NSWindowCollectionBehavior::FullScreenAuxiliary,
        );
        window.setIgnoresMouseEvents(true);
        window.setHasShadow(false);
        true
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn configure(_window: &impl raw_window_handle::HasWindowHandle) -> bool {
        false
    }
}
