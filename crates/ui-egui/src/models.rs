//! The Model section of Settings.

use egui::{CornerRadius, RichText};
use ragna_voice_models::Model;

use crate::i18n::Locale;
use crate::{settings, theme};

/// Height of the download progress bar: a thin line, not a button-sized block.
const BAR_HEIGHT: f32 = 6.0;
/// Room for "100%", so the bar keeps its place while the number grows.
const PERCENT_WIDTH: f32 = 34.0;

/// One catalogue entry as Settings shows it.
#[derive(Debug, Clone, Copy)]
pub struct Row {
    pub model: &'static Model,
    pub installed: bool,
    pub active: bool,
    /// Download progress, 0.0 to 1.0, while this Model downloads.
    pub downloading: Option<f32>,
    /// The bundled Model is part of the app: it can't be deleted.
    pub bundled: bool,
}

/// A button the user clicked. The app performs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Use(&'static Model),
    Download(&'static Model),
    CancelDownload,
    Delete(&'static Model),
}

/// Draws the Model list as rows of a Settings card. `busy` disables other downloads
/// while one runs. `error` is the last download or load failure, shown under the list.
pub fn show(
    ui: &mut egui::Ui,
    locale: Locale,
    rows: &[Row],
    busy: bool,
    error: Option<&str>,
) -> Option<Action> {
    let t = locale.text();
    let mut action = None;
    for (i, row) in rows.iter().enumerate() {
        if i > 0 {
            settings::divider(ui);
        }
        let description = megabytes(row.model.size_bytes, locale);
        settings::row(ui, &locale.model_name(row.model), &description, |ui| {
            if let Some(progress) = row.downloading {
                if settings::secondary(ui, t.cancel, true) {
                    action = Some(Action::CancelDownload);
                }
                // Right to left: Cancel, then the percentage, then a thin bar. The
                // percentage gets a fixed slot so the bar doesn't move as it counts up.
                ui.add_space(4.0);
                ui.add_sized(
                    [PERCENT_WIDTH, ui.spacing().interact_size.y],
                    egui::Label::new(
                        RichText::new(format!("{:.0}%", progress * 100.0))
                            .font(theme::regular(11.0))
                            .color(theme::FG_MUTED),
                    ),
                );
                ui.add_space(4.0);
                ui.add(
                    egui::ProgressBar::new(progress)
                        .desired_width(96.0)
                        .desired_height(BAR_HEIGHT)
                        .corner_radius(CornerRadius::same(3))
                        .fill(theme::ACCENT),
                );
            } else if row.active {
                settings::chip(ui, t.in_use, theme::TEAL_400);
            } else if row.installed {
                if settings::secondary(ui, t.use_model, true) {
                    action = Some(Action::Use(row.model));
                }
                if !row.bundled && settings::secondary(ui, t.delete, true) {
                    action = Some(Action::Delete(row.model));
                }
            } else if settings::secondary(ui, t.download, !busy) {
                action = Some(Action::Download(row.model));
            }
        });
    }
    if let Some(error) = error {
        ui.add_space(6.0);
        ui.label(
            RichText::new(error)
                .font(theme::regular(11.0))
                .color(theme::ERROR),
        );
    }
    ui.add_space(6.0);
    ui.label(
        RichText::new(t.models_hint)
            .font(theme::regular(11.0))
            .color(theme::FG_SUBTLE),
    );
    action
}

/// Whole megabytes, e.g. `148 MB`; `3.1 GB` (German: `3,1 GB`) from a gigabyte up.
fn megabytes(bytes: u64, locale: Locale) -> String {
    let mb = bytes / 1_000_000;
    let point = match locale {
        Locale::English => '.',
        Locale::German => ',',
    };
    if mb >= 1_000 {
        format!("{}{point}{} GB", mb / 1_000, (mb % 1_000) / 100)
    } else {
        format!("{mb} MB")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_like_finder() {
        assert_eq!(megabytes(59_721_011, Locale::English), "59 MB");
        assert_eq!(megabytes(1_533_774_781, Locale::English), "1.5 GB");
        assert_eq!(megabytes(3_095_033_483, Locale::English), "3.0 GB");
        assert_eq!(megabytes(1_533_774_781, Locale::German), "1,5 GB");
    }
}
