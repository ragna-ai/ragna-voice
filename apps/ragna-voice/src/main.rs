//! RAGNA Voice menu-bar app: hold a key, speak, release.
//!
//! The root egui window is the waveform overlay: transparent, click-through and empty
//! while idle. It is never hidden, because eframe stops running the UI while its root
//! window is hidden. Settings is a child window, created while open.

#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

mod runner;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use eframe::egui;
use ragna_voice_core::{Event, Notice, language};
use ragna_voice_models::{Model, Store};
use ragna_voice_platform::hotkey::{Hotkey, Press};
use ragna_voice_platform::permissions::{self, Permission, Status};
use ragna_voice_ui_egui::i18n::{self, Locale};
use ragna_voice_ui_egui::settings::{self, PermissionAction};
use ragna_voice_ui_egui::{models, overlay};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use crate::runner::{Live, Runner};

fn main() -> eframe::Result {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info,whisper_rs=warn"),
    )
    .format_timestamp_millis()
    .init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("RAGNA Voice")
            .with_inner_size(overlay::SIZE)
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(false)
            .with_has_shadow(false)
            .with_window_level(egui::WindowLevel::AlwaysOnTop)
            .with_mouse_passthrough(true)
            .with_taskbar(false)
            // Never take focus from the Target App.
            .with_active(false),
        event_loop_builder: Some(Box::new(|builder| {
            // Menu-bar only: no Dock icon, no app menu (LSUIElement equivalent).
            #[cfg(target_os = "macos")]
            {
                use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
                builder.with_activation_policy(ActivationPolicy::Accessory);
            }
            #[cfg(not(target_os = "macos"))]
            let _ = builder;
        })),
        // The overlay positions itself on every launch.
        persist_window: false,
        ..Default::default()
    };
    eframe::run_native(
        "RAGNA Voice",
        options,
        Box::new(|cc| Ok(Box::new(App::new(&cc.egui_ctx, cc.storage)?))),
    )
}

/// What the user chose in Settings, saved by eframe in Application Support.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct Preferences {
    /// Seconds next to the waveform. Off by default.
    show_timer: bool,
    /// The chosen Model's id; `None` means the bundled one.
    model: Option<String>,
    /// The transcription language's Whisper code; `auto` lets Whisper detect it.
    language: String,
    /// The interface language: `system`, `en` or `de`.
    app_language: String,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            show_timer: false,
            model: None,
            language: language::AUTOMATIC.code.to_owned(),
            app_language: i18n::Choice::System.code().to_owned(),
        }
    }
}

/// Developer override for the Model file, ahead of the user's choice.
const MODEL_ENV: &str = "RAGNA_VOICE_MODEL";

/// Downloads go to Application Support; the bundled Model sits in the app's Resources.
fn model_store() -> Store {
    let dir = eframe::storage_dir("RAGNA Voice")
        .unwrap_or_else(std::env::temp_dir)
        .join("Models");
    let resources = std::env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.parent()?.join("Resources")));
    Store::new(dir, resources)
}

/// The Model to load: the developer override, else the user's choice if it is still
/// installed, else the bundled one. The `Model` is `None` for the override.
fn resolve_model(
    store: &Store,
    preferences: &Preferences,
) -> (Option<&'static Model>, Option<PathBuf>) {
    if let Some(path) = std::env::var_os(MODEL_ENV) {
        return (None, Some(PathBuf::from(path)));
    }
    let model = preferences
        .model
        .as_deref()
        .and_then(ragna_voice_models::find)
        .filter(|model| store.is_installed(model))
        .unwrap_or(&ragna_voice_models::BUNDLED);
    (Some(model), store.path(model))
}

/// A Model download in progress, on its own thread.
struct Download {
    model: &'static Model,
    received: Arc<AtomicU64>,
    cancel: Arc<AtomicBool>,
    done: Receiver<Result<(), ragna_voice_models::Error>>,
}

const PREFERENCES_KEY: &str = "preferences";

/// How often the open Settings window re-reads permissions. macOS has no change notification.
const PERMISSION_POLL: Duration = Duration::from_secs(1);

/// How often Settings refreshes a running download's progress bar.
const DOWNLOAD_PROGRESS: Duration = Duration::from_millis(100);

/// Repaint interval while the overlay animates (about 30 fps).
const OVERLAY_FRAME: Duration = Duration::from_millis(33);

/// Distance from the bottom of the screen to the overlay, clear of the Dock.
const OVERLAY_BOTTOM_MARGIN: f32 = 96.0;

#[derive(Debug, Clone, Copy)]
enum Command {
    ShowSettings,
    /// Turn Esc-to-cancel on or off (main thread only).
    Escape(bool),
    Quit,
}

/// Lets other threads (menu, runner, permission callbacks) command the UI and wake it.
#[derive(Clone)]
struct UiHandle {
    tx: Sender<Command>,
    ctx: egui::Context,
    /// The interface language, for notices posted from the runner thread.
    locale: Arc<Mutex<Locale>>,
}

impl runner::Ui for UiHandle {
    fn changed(&self) {
        self.ctx.request_repaint();
    }

    fn attention(&self) {
        self.send(Command::ShowSettings);
    }

    fn escape(&self, on: bool) {
        self.send(Command::Escape(on));
    }

    fn notice(&self, notice: Notice) {
        let locale = *self.locale.lock().unwrap_or_else(PoisonError::into_inner);
        let (title, body) = locale.notice(notice);
        ragna_voice_platform::notify::post(title, body);
    }
}

/// The interface language for a setting: macOS's preferred languages for "System".
fn resolve_locale(choice: i18n::Choice) -> Locale {
    match choice {
        i18n::Choice::System => {
            Locale::from_preferred(&ragna_voice_platform::locale::preferred_languages())
        }
        i18n::Choice::Fixed(locale) => locale,
    }
}

/// The transcription languages' names in `locale`, in [`language::LANGUAGES`] order.
fn language_names(locale: Locale) -> Vec<String> {
    language::LANGUAGES
        .iter()
        .map(|l| {
            ragna_voice_platform::locale::language_name(l.code, locale.code())
                .unwrap_or_else(|| l.name.to_owned())
        })
        .collect()
}

impl UiHandle {
    fn send(&self, command: Command) {
        if self.tx.send(command).is_ok() {
            self.ctx.request_repaint();
        }
    }
}

/// Shows the system microphone prompt, then brings Settings back to the front: after a
/// system prompt an `LSUIElement` app's window is easily left behind.
fn request_microphone(ui: &UiHandle) {
    let ui = ui.clone();
    permissions::request_microphone(move |granted| {
        log::info!(
            "microphone access {}",
            if granted { "granted" } else { "denied" }
        );
        ui.send(Command::ShowSettings);
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsWindow {
    Closed,
    /// Open, and should come to the front on its next frame.
    Opening,
    Open,
}

struct App {
    // Dropping the tray icon removes it from the menu bar.
    _tray: TrayIcon,
    // `None` if the OS refused the chord; the app still runs from the menu bar.
    hotkey: Option<Hotkey>,
    commands: Receiver<Command>,
    ui: UiHandle,
    live: Arc<Live>,
    /// Configured and positioned; that needs the monitor size, known after a frame.
    overlay_ready: bool,
    settings: SettingsWindow,
    /// Developer aid: with `RAGNA_VOICE_DEMO_OVERLAY=1` the overlay shows a fake
    /// Recording that started at this time, to look at it without holding the Hotkey.
    demo_overlay: Option<Instant>,
    preferences: Preferences,
    /// Changed since last saved.
    preferences_dirty: bool,
    microphone: Status,
    accessibility: Status,
    polled_at: Instant,
    runner: Runner,
    store: Store,
    /// The Model in use; `None` when the developer override is set.
    active_model: Option<&'static Model>,
    download: Option<Download>,
    /// The last download, delete or load failure, shown in Settings.
    model_error: Option<String>,
    /// The interface language in effect.
    locale: Locale,
    /// Transcription language names in `locale`, from macOS.
    language_names: Vec<String>,
    menu_settings: MenuItem,
    menu_quit: MenuItem,
}

impl App {
    fn new(
        ctx: &egui::Context,
        storage: Option<&dyn eframe::Storage>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        ragna_voice_ui_egui::install(ctx);
        let preferences: Preferences = storage
            .and_then(|storage| eframe::get_value(storage, PREFERENCES_KEY))
            .unwrap_or_default();
        let settings = MenuItem::new("", true, None);
        let quit = MenuItem::new("", true, None);
        let menu = Menu::new();
        menu.append_items(&[&settings, &PredefinedMenuItem::separator(), &quit])?;

        let builder = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("RAGNA Voice");
        #[cfg(target_os = "macos")]
        let builder = builder.with_icon_templated(tray_icon_image()?);
        #[cfg(not(target_os = "macos"))]
        let builder = builder.with_icon(tray_icon_image()?);
        let tray = builder.build()?;

        // Menu events and permission callbacks arrive on other threads. They command the
        // UI through this channel and wake egui.
        let (tx, commands) = mpsc::channel();
        let locale = resolve_locale(i18n::Choice::from_code(&preferences.app_language));
        let ui = UiHandle {
            tx,
            ctx: ctx.clone(),
            locale: Arc::new(Mutex::new(locale)),
        };

        let live = Arc::new(Live::default());
        let runner = Runner::spawn(Arc::clone(&live), ui.clone())?;
        let store = model_store();
        let (active_model, model_path) = resolve_model(&store, &preferences);
        if model_path.is_none() {
            log::error!("no Model installed: open Settings to download one");
        }
        runner.load_model(model_path);
        runner.set_language(language::find(&preferences.language));
        let hotkey_runner = runner.clone();
        let hotkey = Hotkey::register_default(move |press| {
            let event = match press {
                Press::Down => Event::HotkeyDown,
                Press::Up => Event::HotkeyUp,
                Press::Escape => Event::EscPressed,
            };
            hotkey_runner.send(event, Instant::now());
        })
        .inspect_err(|e| log::error!("could not register the Hotkey: {e}"))
        .ok();

        let (settings_id, quit_id) = (settings.id().clone(), quit.id().clone());
        let menu_ui = ui.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let command = if event.id == settings_id {
                Command::ShowSettings
            } else if event.id == quit_id {
                Command::Quit
            } else {
                return;
            };
            menu_ui.send(command);
        }));

        let microphone = Permission::Microphone.status();
        let accessibility = Permission::Accessibility.status();
        log::info!("permissions: microphone {microphone:?}, accessibility {accessibility:?}");
        // Ask now, not during the first Dictation: a Recording made while the prompt is
        // open captures silence and is Dropped.
        if microphone == Status::NotAsked {
            request_microphone(&ui);
        }
        if microphone != Status::Granted || accessibility != Status::Granted {
            ui.send(Command::ShowSettings);
        }

        Ok(Self {
            _tray: tray,
            hotkey,
            commands,
            ui,
            live,
            overlay_ready: false,
            settings: SettingsWindow::Closed,
            demo_overlay: std::env::var_os("RAGNA_VOICE_DEMO_OVERLAY").map(|_| Instant::now()),
            preferences,
            preferences_dirty: false,
            microphone,
            accessibility,
            polled_at: Instant::now(),
            runner,
            store,
            active_model,
            download: None,
            model_error: None,
            locale,
            language_names: Vec::new(),
            menu_settings: settings,
            menu_quit: quit,
        }
        .with_locale(locale))
    }

    fn model_rows(&self) -> Vec<models::Row> {
        ragna_voice_models::CATALOG
            .iter()
            .map(|model| models::Row {
                model,
                installed: self.store.is_installed(model),
                active: self.active_model == Some(model),
                downloading: self
                    .download
                    .as_ref()
                    .filter(|download| download.model == model)
                    .map(|download| {
                        #[allow(clippy::cast_precision_loss)]
                        let fraction = download.received.load(Ordering::Relaxed) as f32
                            / model.size_bytes.max(1) as f32;
                        fraction
                    }),
                bundled: *model == ragna_voice_models::BUNDLED,
            })
            .collect()
    }

    fn perform_model(&mut self, action: models::Action, ctx: &egui::Context) {
        log::info!("models: {action:?}");
        self.model_error = None;
        match action {
            models::Action::Use(model) => {
                self.active_model = Some(model);
                self.preferences.model = Some(model.id.to_owned());
                self.preferences_dirty = true;
                self.runner.load_model(self.store.path(model));
            }
            models::Action::Download(model) => self.start_download(model, ctx),
            models::Action::CancelDownload => {
                if let Some(download) = &self.download {
                    download.cancel.store(true, Ordering::Relaxed);
                }
            }
            models::Action::Delete(model) => {
                if let Err(e) = self.store.delete(model) {
                    log::error!("could not delete {}: {e}", model.id);
                    self.model_error = Some(self.locale.model_error(&e).to_owned());
                }
            }
        }
    }

    /// The user asked for this download: it is the app's one network call.
    fn start_download(&mut self, model: &'static Model, ctx: &egui::Context) {
        let (received, cancel) = (
            Arc::new(AtomicU64::new(0)),
            Arc::new(AtomicBool::new(false)),
        );
        let (tx, done) = mpsc::channel();
        let (store, ctx) = (self.store.clone(), ctx.clone());
        let (progress, stop) = (Arc::clone(&received), Arc::clone(&cancel));
        let spawned = std::thread::Builder::new()
            .name("download".into())
            .spawn(move || {
                let result = store
                    .download(
                        model,
                        |bytes, _| progress.store(bytes, Ordering::Relaxed),
                        &stop,
                    )
                    .map(drop);
                let _ = tx.send(result);
                ctx.request_repaint();
            });
        match spawned {
            Ok(_) => {
                self.download = Some(Download {
                    model,
                    received,
                    cancel,
                    done,
                });
            }
            Err(e) => {
                log::error!("could not start the download: {e}");
                self.model_error = Some(self.locale.text().error_network.to_owned());
            }
        }
    }

    fn poll_download(&mut self) {
        let Some(download) = &self.download else {
            return;
        };
        let result = match download.done.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => Err(ragna_voice_models::Error::Cancelled),
        };
        let model = download.model;
        self.download = None;
        match result {
            Ok(()) => log::info!("downloaded {}", model.id),
            Err(ragna_voice_models::Error::Cancelled) => {
                log::info!("download of {} cancelled", model.id);
            }
            Err(e) => {
                log::error!("download of {} failed: {e}", model.id);
                self.model_error = Some(format!(
                    "{}: {}",
                    self.locale.model_name(model),
                    self.locale.model_error(&e)
                ));
            }
        }
    }

    fn with_locale(mut self, locale: Locale) -> Self {
        self.set_locale(locale);
        self
    }

    /// Switches the interface language everywhere: menu bar, Settings, overlay and
    /// notifications.
    fn set_locale(&mut self, locale: Locale) {
        self.locale = locale;
        *self
            .ui
            .locale
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = locale;
        let t = locale.text();
        self.menu_settings.set_text(t.menu_settings);
        self.menu_quit.set_text(t.menu_quit);
        self.language_names = language_names(locale);
        self.model_error = None;
    }

    fn poll_permissions(&mut self) {
        self.microphone = Permission::Microphone.status();
        self.accessibility = Permission::Accessibility.status();
        self.polled_at = Instant::now();
    }

    fn perform(&self, action: PermissionAction) {
        log::info!("permissions: {action:?}");
        match action {
            PermissionAction::Grant(permission) => {
                // A denied or stale entry (one that belonged to an earlier signature)
                // suppresses the prompt. Clearing our own entry first makes it appear.
                // The running bundle's own id: a dev build must never reset the
                // installed release's permissions. Unbundled, there is nothing to reset.
                if permission.status() == Status::Denied {
                    match ragna_voice_platform::app::bundle_id() {
                        Some(id) => {
                            if let Err(e) = permission.reset(&id) {
                                log::error!("could not reset {permission:?}: {e}");
                            }
                        }
                        None => log::warn!("not bundled: cannot reset {permission:?}"),
                    }
                }
                match permission {
                    Permission::Microphone => request_microphone(&self.ui),
                    Permission::Accessibility => {
                        let _ = permissions::request_accessibility();
                    }
                }
            }
            PermissionAction::OpenSettings(permission) => {
                if let Err(e) = permission.open_settings() {
                    log::error!("could not open System Settings: {e}");
                }
            }
        }
    }

    /// Makes the root window an overlay (see `platform::overlay`) and moves it to the
    /// bottom centre of the screen.
    fn set_up_overlay(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        let Some(monitor) = ctx.input(|i| i.viewport().monitor_size) else {
            return;
        };
        let configured = frame
            .winit_window()
            .is_some_and(|window| ragna_voice_platform::overlay::configure(window.as_ref()));
        if !configured {
            log::warn!("could not configure the overlay window");
        }
        let x = (monitor.x - overlay::SIZE.x) / 2.0;
        let y = monitor.y - overlay::SIZE.y - OVERLAY_BOTTOM_MARGIN;
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(x, y)));
        self.overlay_ready = true;
    }

    fn show_settings(&mut self, ctx: &egui::Context) {
        let id = egui::ViewportId::from_hash_of("settings");
        let builder = egui::ViewportBuilder::default()
            .with_title(self.locale.text().settings_title)
            .with_inner_size([480.0, 580.0]);
        ctx.show_viewport_immediate(id, builder, |ui, _class| {
            if self.settings == SettingsWindow::Opening {
                ragna_voice_platform::app::activate();
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Focus);
                self.settings = SettingsWindow::Open;
            }
            if ui.ctx().input(|i| i.viewport().close_requested()) {
                self.settings = SettingsWindow::Closed;
                return;
            }
            if self.polled_at.elapsed() >= PERMISSION_POLL {
                self.poll_permissions();
            }
            self.poll_download();
            ui.ctx().request_repaint_after(if self.download.is_some() {
                DOWNLOAD_PROGRESS
            } else {
                PERMISSION_POLL
            });
            let before = self.preferences.clone();
            let rows = self.model_rows();
            let model_error = self.model_error.clone();
            let mut language = language::find(&self.preferences.language);
            let multilingual = self.active_model.is_none_or(|model| model.multilingual);
            let mut app_language = i18n::Choice::from_code(&self.preferences.app_language);
            let language_names = std::mem::take(&mut self.language_names);
            let mut view = settings::View {
                locale: self.locale,
                app_language: &mut app_language,
                microphone: self.microphone,
                accessibility: self.accessibility,
                show_timer: &mut self.preferences.show_timer,
                language: &mut language,
                multilingual,
                language_names: &language_names,
                models: &rows,
                downloading: self.download.is_some(),
                model_error: model_error.as_deref(),
            };
            let action = settings::show(ui, &mut view);
            self.language_names = language_names;
            if app_language.code() != self.preferences.app_language {
                app_language
                    .code()
                    .clone_into(&mut self.preferences.app_language);
                self.set_locale(resolve_locale(app_language));
            }
            if language.code != self.preferences.language {
                language.code.clone_into(&mut self.preferences.language);
                self.runner.set_language(language);
            }
            if self.preferences != before {
                self.preferences_dirty = true;
            }
            match action {
                Some(settings::Action::Permission(action)) => {
                    self.perform(action);
                    self.poll_permissions();
                }
                Some(settings::Action::Model(action)) => self.perform_model(action, ui.ctx()),
                None => {}
            }
        });
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(command) = self.commands.try_recv() {
            match command {
                Command::ShowSettings => self.settings = SettingsWindow::Opening,
                Command::Escape(on) => {
                    if let Some(Err(e)) = self.hotkey.as_ref().map(|hotkey| hotkey.set_escape(on)) {
                        log::warn!(
                            "could not turn Esc-to-cancel {}: {e}",
                            if on { "on" } else { "off" }
                        );
                    }
                }
                Command::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if !self.overlay_ready {
            self.set_up_overlay(&ctx, frame);
        }

        let snapshot = self.live.snapshot();
        let view = if let Some(since) = self.demo_overlay {
            overlay::View {
                state: ragna_voice_core::State::Recording,
                live: true,
                elapsed: Some(since.elapsed()),
                level: 0.6,
                show_timer: self.preferences.show_timer,
                transcribing: self.locale.text().transcribing,
            }
        } else {
            overlay::View {
                state: snapshot.state,
                live: snapshot.capturing_since.is_some(),
                elapsed: snapshot.speaking_since.map(|since| since.elapsed()),
                level: self.live.level(),
                show_timer: self.preferences.show_timer,
                transcribing: self.locale.text().transcribing,
            }
        };
        overlay::show(ui, &view);
        if view.is_visible() {
            ctx.request_repaint_after(OVERLAY_FRAME);
        }

        if self.settings != SettingsWindow::Closed {
            self.show_settings(&ctx);
        }
        // Save at once rather than at eframe's next autosave, so a crash or kill
        // can't lose the change.
        if std::mem::take(&mut self.preferences_dirty)
            && let Some(storage) = frame.storage_mut()
        {
            eframe::set_value(storage, PREFERENCES_KEY, &self.preferences);
            storage.flush();
        }
    }

    fn on_exit(&mut self) {
        self.runner.shutdown(Duration::from_secs(5));
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, PREFERENCES_KEY, &self.preferences);
    }

    /// Only our own preferences are worth keeping, not egui's widget state.
    fn persist_egui_memory(&self) -> bool {
        false
    }

    /// Transparent, so the overlay window is invisible while idle.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }
}

/// The rounded square with a knocked-out R (`ragna-voice-icon`). On macOS it is drawn
/// as a template image, so the system tints it to match the menu bar.
fn tray_icon_image() -> Result<Icon, tray_icon::BadIcon> {
    /// 22 pt at 2×, the tallest image tray-icon shows unscaled.
    const PX: u32 = 44;
    Icon::from_rgba(ragna_voice_icon::menu_bar(PX), PX, PX)
}
