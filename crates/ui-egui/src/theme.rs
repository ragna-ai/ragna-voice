//! The RAGNA look for Settings: brand colours (`ragna-website/docs/brand/colors.md`) and
//! the Geist type system (`typography.md`). Dark is the default; teal is the only
//! interactive accent and never fills large areas.

use std::sync::Arc;

use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle};

pub const NAVY_950: Color32 = Color32::from_rgb(0x08, 0x15, 0x23);
pub const NAVY_900: Color32 = Color32::from_rgb(0x0c, 0x1b, 0x2e);
pub const NAVY_800: Color32 = Color32::from_rgb(0x10, 0x24, 0x3c);
pub const NAVY_700: Color32 = Color32::from_rgb(0x24, 0x3c, 0x58);
pub const NAVY_600: Color32 = Color32::from_rgb(0x3d, 0x57, 0x76);
pub const NAVY_400: Color32 = Color32::from_rgb(0x7a, 0x95, 0xb4);
pub const NAVY_300: Color32 = Color32::from_rgb(0x9f, 0xb8, 0xd4);
pub const NAVY_50: Color32 = Color32::from_rgb(0xf4, 0xf7, 0xfb);
pub const TEAL_700: Color32 = Color32::from_rgb(0x02, 0x73, 0x67);
pub const TEAL_500: Color32 = Color32::from_rgb(0x19, 0xb8, 0xa6);
pub const TEAL_400: Color32 = Color32::from_rgb(0x50, 0xcf, 0xbd);
pub const TEAL_300: Color32 = Color32::from_rgb(0x8a, 0xe6, 0xda);
pub const SUCCESS: Color32 = Color32::from_rgb(0x7c, 0xcd, 0x8e);
pub const WARNING: Color32 = Color32::from_rgb(0xe4, 0xac, 0x59);
pub const ERROR: Color32 = Color32::from_rgb(0xfb, 0x98, 0x90);

/// Semantic tokens (dark mode).
pub const CANVAS: Color32 = NAVY_950;
pub const SURFACE: Color32 = NAVY_900;
pub const RAISED: Color32 = NAVY_800;
pub const LINE: Color32 = NAVY_700;
pub const FG: Color32 = NAVY_50;
pub const FG_MUTED: Color32 = NAVY_300;
pub const FG_SUBTLE: Color32 = NAVY_400;
pub const ACCENT: Color32 = TEAL_500;
pub const ON_ACCENT: Color32 = NAVY_950;

pub const RADIUS: u8 = 8;
pub const CARD_RADIUS: u8 = 12;

const REGULAR: &str = "geist";
const MEDIUM: &str = "geist-medium";
const SEMIBOLD: &str = "geist-semibold";
const MONO: &str = "geist-mono";

/// Sets up what Settings needs: the Geist fonts and the SVG loader for icons. Call once
/// at startup.
pub fn install(ctx: &egui::Context) {
    install_fonts(ctx);
    egui_extras::install_image_loaders(ctx);
}

/// Registers the Geist fonts under their own family names, leaving egui's defaults (and
/// so the overlay) untouched.
fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let files: [(&str, &'static [u8]); 4] = [
        (REGULAR, include_bytes!("../assets/fonts/Geist-Regular.ttf")),
        (MEDIUM, include_bytes!("../assets/fonts/Geist-Medium.ttf")),
        (
            SEMIBOLD,
            include_bytes!("../assets/fonts/Geist-SemiBold.ttf"),
        ),
        (
            MONO,
            include_bytes!("../assets/fonts/GeistMono-Regular.ttf"),
        ),
    ];
    // egui's own fonts stay as fallbacks for glyphs Geist lacks. They don't cover
    // everything either: no ⌃ or ⌥, so key names are spelled out (see `settings::usage`).
    let fallbacks = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    for (name, bytes) in files {
        fonts.font_data.insert(
            name.to_owned(),
            Arc::new(egui::FontData::from_static(bytes)),
        );
        let mut family = vec![name.to_owned()];
        family.extend(fallbacks.iter().cloned());
        fonts.families.insert(FontFamily::Name(name.into()), family);
    }
    ctx.set_fonts(fonts);
}

pub fn regular(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(REGULAR.into()))
}

pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(MEDIUM.into()))
}

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
}

/// Applies the RAGNA dark theme to `style`.
pub fn apply(style: &mut egui::Style) {
    style.text_styles = [
        (TextStyle::Heading, semibold(17.0)),
        (TextStyle::Body, regular(13.0)),
        (TextStyle::Button, medium(12.0)),
        (TextStyle::Small, regular(11.0)),
        (
            TextStyle::Monospace,
            FontId::new(12.0, FontFamily::Name(MONO.into())),
        ),
    ]
    .into();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 4.0);
    style.spacing.interact_size.y = 24.0;

    let v = &mut style.visuals;
    v.dark_mode = true;
    v.override_text_color = None;
    v.panel_fill = CANVAS;
    v.window_fill = SURFACE;
    v.window_stroke = Stroke::new(1.0, LINE);
    v.extreme_bg_color = NAVY_950;
    v.faint_bg_color = SURFACE;
    v.hyperlink_color = ACCENT;
    v.error_fg_color = ERROR;
    v.warn_fg_color = WARNING;
    v.selection.bg_fill = TEAL_700;
    v.selection.stroke = Stroke::new(1.0, TEAL_300);

    let radius = CornerRadius::same(RADIUS);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = SURFACE;
    w.noninteractive.weak_bg_fill = SURFACE;
    w.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
    w.noninteractive.fg_stroke = Stroke::new(1.0, FG);
    w.noninteractive.corner_radius = radius;
    for (state, fill, stroke) in [
        (&mut w.inactive, RAISED, LINE),
        (&mut w.hovered, NAVY_700, NAVY_600),
        (&mut w.active, NAVY_600, TEAL_400),
        (&mut w.open, NAVY_700, NAVY_600),
    ] {
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.bg_stroke = Stroke::new(1.0, stroke);
        state.fg_stroke = Stroke::new(1.0, FG);
        state.corner_radius = radius;
        state.expansion = 0.0;
    }
}
