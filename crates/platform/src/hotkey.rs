//! The global Hotkey, and Esc while it is held.
//!
//! Both are Carbon hotkeys, which need no permission (unlike a `CGEventTap`, which
//! needs Input Monitoring and can freeze input system-wide if Accessibility is revoked
//! while it runs).

use std::cell::Cell;

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

pub use global_hotkey::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// The Hotkey went down.
    Down,
    /// The Hotkey came up.
    Up,
    /// Esc, while [`Hotkey::set_escape`] is on.
    Escape,
}

/// The default chord's modifiers. Esc is registered with them too, because the user is
/// still holding them when they press Esc.
const MODIFIERS: Modifiers = Modifiers::CONTROL.union(Modifiers::ALT);

/// Keeps the Hotkey registered while alive. Main thread only.
pub struct Hotkey {
    manager: GlobalHotKeyManager,
    escape: [HotKey; 2],
    escape_on: Cell<bool>,
}

impl std::fmt::Debug for Hotkey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Hotkey")
            .field("escape_on", &self.escape_on.get())
            .finish_non_exhaustive()
    }
}

impl Hotkey {
    /// Registers the default chord, `⌃⌥Space`. Create it on the main thread, while
    /// the app's event loop runs; `on_press` is called on that thread.
    ///
    /// Only one `Hotkey` may exist: the press handler is process-global.
    ///
    /// # Errors
    ///
    /// The OS refused the registration, e.g. another app already owns the chord.
    pub fn register_default(
        on_press: impl Fn(Press) + Send + Sync + 'static,
    ) -> Result<Self, Error> {
        let manager = GlobalHotKeyManager::new()?;
        let hotkey = HotKey::new(Some(MODIFIERS), Code::Space);
        manager.register(hotkey)?;
        let escape = [
            HotKey::new(Some(MODIFIERS), Code::Escape),
            HotKey::new(None, Code::Escape),
        ];
        let (id, escape_ids) = (hotkey.id(), escape.map(|key| key.id()));
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            match (event.state, event.id) {
                (HotKeyState::Pressed, i) if i == id => on_press(Press::Down),
                (HotKeyState::Released, i) if i == id => on_press(Press::Up),
                (HotKeyState::Pressed, i) if escape_ids.contains(&i) => on_press(Press::Escape),
                _ => {}
            }
        }));
        Ok(Self {
            manager,
            escape,
            escape_on: Cell::new(false),
        })
    }

    /// Turns Esc-to-cancel on or off. While on, Esc is a global hotkey: it reaches us
    /// instead of the frontmost app, so turn it off as soon as the Hotkey is released.
    ///
    /// # Errors
    ///
    /// The OS refused to (un)register Esc. Esc-to-cancel is then unavailable for this
    /// Dictation; nothing else is affected.
    pub fn set_escape(&self, on: bool) -> Result<(), Error> {
        if on == self.escape_on.get() {
            return Ok(());
        }
        if on {
            if let Err(e) = self.manager.register_all(&self.escape) {
                // Never leave half of it registered: that Esc would stay captured for good.
                // Errors here are expected: one of the two may never have registered.
                let _ = self.unregister_escape();
                return Err(e);
            }
            self.escape_on.set(true);
            Ok(())
        } else {
            self.escape_on.set(false);
            self.unregister_escape()
        }
    }

    /// Unregisters each Esc key, even if an earlier one fails. Returns the first error.
    fn unregister_escape(&self) -> Result<(), Error> {
        self.escape
            .iter()
            .map(|key| self.manager.unregister(*key))
            .fold(Ok(()), Result::and)
    }
}
