//! The Settings window: a header, then one card per topic (General, Permissions,
//! Overlay, Model), in the RAGNA look (`theme`) and the chosen language (`i18n`).

use egui::{Align, Color32, CornerRadius, Layout, Margin, RichText, Sense, Stroke, Vec2};
use ragna_voice_platform::permissions::{Permission, Status};

use ragna_voice_core::language::{self as lang, Language};

use crate::i18n::{Choice, Locale, Text};
use crate::models;
use crate::theme::{self, FG, FG_MUTED, FG_SUBTLE, LINE, SURFACE};

/// What Settings shows. Built by the app every frame.
#[derive(Debug)]
pub struct View<'a> {
    /// The interface language in effect.
    pub locale: Locale,
    /// The interface language setting; the picker edits it in place.
    pub app_language: &'a mut Choice,
    pub microphone: Status,
    pub accessibility: Status,
    /// The overlay preference; the toggle edits it in place.
    pub show_timer: &'a mut bool,
    /// The transcription language; the picker edits it in place.
    pub language: &'a mut Language,
    /// Whether the Model in use can transcribe other languages than English.
    pub multilingual: bool,
    /// The names of [`lang::LANGUAGES`] in the interface language, in the same order.
    pub language_names: &'a [String],
    pub models: &'a [models::Row],
    /// A download is running, so the other Download buttons are disabled.
    pub downloading: bool,
    /// The last Model download, delete or load failure.
    pub model_error: Option<&'a str>,
}

/// A button the user clicked. The app performs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Permission(PermissionAction),
    Model(models::Action),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionAction {
    /// Show the system prompt (after clearing a denied or stale entry).
    Grant(Permission),
    OpenSettings(Permission),
}

/// Draws Settings into `ui`, filling it.
pub fn show(ui: &mut egui::Ui, view: &mut View<'_>) -> Option<Action> {
    theme::apply(ui.style_mut());
    let t = view.locale.text();
    let mut action = None;
    egui::Frame::new()
        .fill(theme::CANVAS)
        .inner_margin(Margin::same(20))
        .show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            egui::ScrollArea::vertical().show(ui, |ui| {
                header(ui, t);
                ui.add_space(12.0);
                usage(ui, t);
                ui.add_space(16.0);

                section(ui, t.section_general);
                card(ui, |ui| {
                    app_language_row(ui, t, view.locale, view.app_language);
                });

                section(ui, t.section_permissions);
                card(ui, |ui| {
                    permission(ui, t, Permission::Microphone, view.microphone, &mut action);
                    divider(ui);
                    permission(
                        ui,
                        t,
                        Permission::Accessibility,
                        view.accessibility,
                        &mut action,
                    );
                });

                section(ui, t.section_overlay);
                card(ui, |ui| {
                    row(ui, t.show_timer, t.show_timer_description, |ui| {
                        toggle(ui, &mut *view.show_timer);
                    });
                });

                section(ui, t.section_model);
                card(ui, |ui| {
                    language_row(ui, t, view.language, view.multilingual, view.language_names);
                    divider(ui);
                    if let Some(clicked) = models::show(
                        ui,
                        view.locale,
                        view.models,
                        view.downloading,
                        view.model_error,
                    ) {
                        action = Some(Action::Model(clicked));
                    }
                });
            });
        });
    action
}

/// How to dictate, with the keys drawn as keycaps. Words, not ⌃⌥ glyphs: neither Geist
/// nor egui's fallback fonts have those symbols.
fn usage(ui: &mut egui::Ui, t: &Text) {
    let text = |ui: &mut egui::Ui, s: &str| {
        ui.label(RichText::new(s).font(theme::regular(12.0)).color(FG_MUTED));
    };
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        text(ui, t.hint_hold);
        keycap(ui, t.key_control);
        keycap(ui, t.key_option);
        keycap(ui, t.key_space);
        text(ui, t.hint_to_dictate);
    });
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        text(ui, t.hint_press);
        keycap(ui, t.key_esc);
        text(ui, t.hint_to_discard);
    });
}

/// A key name drawn like a key on a keyboard.
fn keycap(ui: &mut egui::Ui, key: &str) {
    egui::Frame::new()
        .fill(theme::RAISED)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(CornerRadius::same(4))
        .inner_margin(Margin::symmetric(5, 1))
        .show(ui, |ui| {
            ui.label(RichText::new(key).font(theme::medium(11.0)).color(FG));
        });
}

fn header(ui: &mut egui::Ui, t: &Text) {
    ui.horizontal(|ui| {
        ui.add(egui::Image::new(&logo(ui.ctx())).fit_to_exact_size(Vec2::splat(32.0)));
        ui.add_space(4.0);
        ui.vertical(|ui| {
            ui.add_space(1.0);
            ui.label(
                RichText::new("RAGNA Voice")
                    .font(theme::semibold(17.0))
                    .color(FG),
            );
            ui.label(
                RichText::new(format!("{} {}", t.version, env!("CARGO_PKG_VERSION")))
                    .font(theme::regular(11.0))
                    .color(FG_SUBTLE),
            );
        });
    });
}

/// The app icon as a texture, created once per context.
fn logo(ctx: &egui::Context) -> egui::TextureHandle {
    const PX: u32 = 96;
    let id = egui::Id::new("ragna-voice-logo");
    if let Some(texture) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return texture;
    }
    let image = egui::ColorImage::from_rgba_unmultiplied(
        [PX as usize, PX as usize],
        &ragna_voice_icon::app_icon(PX),
    );
    let texture = ctx.load_texture("logo", image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, texture.clone()));
    texture
}

fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(4.0);
    ui.label(
        RichText::new(title)
            .font(theme::medium(12.0))
            .color(FG_MUTED),
    );
    ui.add_space(2.0);
}

fn card(ui: &mut egui::Ui, contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(CornerRadius::same(theme::CARD_RADIUS))
        .inner_margin(Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            contents(ui);
        });
    ui.add_space(16.0);
}

pub(crate) fn divider(ui: &mut egui::Ui) {
    ui.add_space(2.0);
    let rect = ui.available_rect_before_wrap();
    ui.painter()
        .hline(rect.x_range(), rect.top(), Stroke::new(1.0, LINE));
    ui.add_space(6.0);
}

/// Room kept free on the right of a row for its controls.
const TRAILING_WIDTH: f32 = 160.0;

/// A title and description on the left, `trailing` controls on the right.
pub(crate) fn row(
    ui: &mut egui::Ui,
    title: &str,
    description: &str,
    trailing: impl FnOnce(&mut egui::Ui),
) {
    row_with(ui, title, FG, description, |_| {}, trailing);
}

/// Like [`row`], with `below` drawn under the description (e.g. a link).
fn row_with(
    ui: &mut egui::Ui,
    title: &str,
    title_color: Color32,
    description: &str,
    below: impl FnOnce(&mut egui::Ui),
    trailing: impl FnOnce(&mut egui::Ui),
) {
    ui.horizontal(|ui| {
        let text_width = (ui.available_width() - TRAILING_WIDTH).max(120.0);
        ui.vertical(|ui| {
            ui.set_width(text_width);
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(
                RichText::new(title)
                    .font(theme::medium(13.0))
                    .color(title_color),
            );
            ui.add(
                egui::Label::new(
                    RichText::new(description)
                        .font(theme::regular(11.0))
                        .color(FG_MUTED),
                )
                .wrap(),
            );
            below(ui);
        });
        ui.with_layout(Layout::right_to_left(Align::Center), trailing);
    });
}

/// A status label, tinted by `color`, with exactly a button's shape (height, corners,
/// padding, text size), so it lines up with buttons in the same column.
pub(crate) fn chip(ui: &mut egui::Ui, text: &str, color: Color32) {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), theme::medium(12.0), color);
    let padding = ui.spacing().button_padding.x;
    let size = egui::vec2(
        galley.size().x + 2.0 * padding,
        ui.spacing().interact_size.y,
    );
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(theme::RADIUS),
        color.gamma_multiply(0.14),
    );
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, color);
}

/// The one teal, filled button style: for the action that moves things forward.
pub(crate) fn primary(ui: &mut egui::Ui, text: &str, enabled: bool) -> bool {
    let button = egui::Button::new(RichText::new(text).color(theme::ON_ACCENT))
        .fill(theme::ACCENT)
        .corner_radius(CornerRadius::same(theme::RADIUS));
    ui.add_enabled(enabled, button).clicked()
}

/// Everything else: navy, so teal stays rare (brand: about 5% teal).
pub(crate) fn secondary(ui: &mut egui::Ui, text: &str, enabled: bool) -> bool {
    ui.add_enabled(enabled, egui::Button::new(RichText::new(text).color(FG)))
        .clicked()
}

/// An iOS-style switch.
fn toggle(ui: &mut egui::Ui, on: &mut bool) -> egui::Response {
    let size = egui::vec2(30.0, 17.0);
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, *on, ""));
    let t = ui.ctx().animate_bool_responsive(response.id, *on);
    let track = if *on { theme::ACCENT } else { theme::NAVY_700 };
    let radius = rect.height() / 2.0;
    ui.painter().rect_filled(rect, radius, track);
    let x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), t);
    ui.painter()
        .circle_filled(egui::pos2(x, rect.center().y), radius - 3.0, theme::NAVY_50);
    response
}

fn app_language_row(ui: &mut egui::Ui, t: &Text, locale: Locale, choice: &mut Choice) {
    row(ui, t.app_language, t.app_language_description, |ui| {
        egui::ComboBox::from_id_salt("app-language")
            .selected_text(choice.label(locale))
            .width(150.0)
            .show_ui(ui, |ui| {
                for option in Choice::ALL {
                    ui.selectable_value(choice, option, option.label(locale));
                }
            });
    });
}

fn language_row(
    ui: &mut egui::Ui,
    t: &Text,
    language: &mut Language,
    multilingual: bool,
    names: &[String],
) {
    let description = if multilingual {
        t.language_description
    } else {
        t.language_english_only
    };
    // AUTOMATIC isn't a real language, so macOS can't name it.
    let name = |option: &Language| -> String {
        if *option == lang::AUTOMATIC {
            return t.automatic.to_owned();
        }
        lang::LANGUAGES
            .iter()
            .position(|l| l == option)
            .and_then(|i| names.get(i))
            .cloned()
            .unwrap_or_else(|| option.name.to_owned())
    };
    row(ui, t.language, description, |ui| {
        let shown = if multilingual {
            *language
        } else {
            lang::ENGLISH
        };
        ui.add_enabled_ui(multilingual, |ui| {
            egui::ComboBox::from_id_salt("language")
                .selected_text(name(&shown))
                .width(150.0)
                .height(320.0)
                .show_ui(ui, |ui| {
                    for option in lang::LANGUAGES {
                        ui.selectable_value(language, *option, name(option));
                    }
                });
        });
    });
}

fn permission(
    ui: &mut egui::Ui,
    t: &Text,
    permission: Permission,
    status: Status,
    action: &mut Option<Action>,
) {
    let (title, description) = match permission {
        Permission::Microphone => (t.microphone, t.microphone_description),
        Permission::Accessibility => (t.accessibility, t.accessibility_description),
    };
    let granted = status == Status::Granted;
    let (mut open_settings, mut grant) = (false, false);
    row_with(
        ui,
        title,
        if granted { FG } else { theme::ERROR },
        description,
        |ui| {
            open_settings = !granted
                && ui
                    .link(RichText::new(t.open_system_settings).font(theme::regular(11.0)))
                    .clicked();
        },
        |ui| {
            if granted {
                chip(ui, t.granted, theme::SUCCESS);
            } else {
                grant = primary(ui, t.grant, true);
                ui.add(
                    egui::Image::new(egui::include_image!("../assets/icons/triangle-alert.svg"))
                        .tint(theme::ERROR)
                        .fit_to_exact_size(Vec2::splat(16.0)),
                )
                .on_hover_text(t.not_granted);
            }
        },
    );
    if grant {
        *action = Some(Action::Permission(PermissionAction::Grant(permission)));
    } else if open_settings {
        *action = Some(Action::Permission(PermissionAction::OpenSettings(
            permission,
        )));
    }
}
