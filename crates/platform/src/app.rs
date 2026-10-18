//! The app process as a whole.

/// The running app's bundle identifier (`io.ragna.voice`, or `io.ragna.voice.dev` for
/// dev builds); `None` when running as a bare binary.
#[must_use]
pub fn bundle_id() -> Option<String> {
    imp::bundle_id()
}

/// Brings the app and its key window to the front. A menu-bar-only (`LSUIElement`)
/// app isn't activated by focusing one of its windows, so a window it opens can end up
/// behind the frontmost app. Call on the main thread; elsewhere it does nothing.
pub fn activate() {
    imp::activate();
}

#[cfg(target_os = "macos")]
mod imp {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;
    use objc2_foundation::NSBundle;

    pub fn bundle_id() -> Option<String> {
        NSBundle::mainBundle()
            .bundleIdentifier()
            .map(|id| id.to_string())
    }

    pub fn activate() {
        if let Some(mtm) = MainThreadMarker::new() {
            NSApplication::sharedApplication(mtm).activate();
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn bundle_id() -> Option<String> {
        None
    }

    pub fn activate() {}
}
