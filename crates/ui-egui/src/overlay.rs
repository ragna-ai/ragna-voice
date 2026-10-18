//! The waveform overlay: a pill with live bars and a recording timer while recording,
//! and a spinner while transcribing.

use std::time::Duration;

use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Stroke, Vec2, pos2, vec2};
use ragna_voice_core::State;

/// Size of the pill with the timer. The overlay window should be exactly this big.
pub const SIZE: Vec2 = vec2(196.0, 44.0);

const BAR_COUNT: usize = 10;
const BAR_WIDTH: f32 = 4.0;
const BAR_GAP: f32 = 3.0;
const BAR_MAX_HEIGHT: f32 = 22.0;
const BAR_MIN_HEIGHT: f32 = 3.0;
/// Space between the pill's edge and its content.
const PADDING: f32 = 22.0;

/// Width of the pill without the timer: just the bars.
#[allow(clippy::cast_precision_loss)]
const COMPACT_WIDTH: f32 =
    2.0 * PADDING + BAR_COUNT as f32 * BAR_WIDTH + (BAR_COUNT - 1) as f32 * BAR_GAP;

// Opaque: any translucency lets lines behind the overlay show through the pill.
const BACKGROUND: Color32 = Color32::from_rgb(24, 24, 27);
const FOREGROUND: Color32 = Color32::from_rgb(0xf4, 0xf4, 0xf5);
const MUTED: Color32 = Color32::from_rgb(0xa1, 0xa1, 0xaa);
/// Past the soft cap: still recording, but heading for the hard cap.
const WARNING: Color32 = Color32::from_rgb(0xf5, 0x9e, 0x0b);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    pub state: State,
    /// Whether the microphone delivers audio yet.
    pub live: bool,
    /// Time since the first speech; `None` until the user speaks.
    pub elapsed: Option<Duration>,
    /// Microphone level, 0.0 (silence) to 1.0.
    pub level: f32,
    /// Show the seconds next to the bars. Off by default: a running clock can feel
    /// like pressure.
    pub show_timer: bool,
    /// Shown next to the spinner, in the interface language.
    pub transcribing: &'static str,
}

impl View {
    /// Whether there is anything to draw. The window stays empty otherwise.
    #[must_use]
    pub fn is_visible(&self) -> bool {
        matches!(
            self.state,
            State::Recording | State::SoftCapWarning | State::Transcribing
        )
    }
}

/// Seconds with one decimal, e.g. `6.3s`. Truncates, so the display never runs ahead.
#[must_use]
pub fn format_elapsed(elapsed: Duration) -> String {
    let tenths = elapsed.as_millis() / 100;
    format!("{}.{}s", tenths / 10, tenths % 10)
}

/// Draws the overlay into `ui`, centred. Draws nothing when the view is not visible.
pub fn show(ui: &mut egui::Ui, view: &View) {
    if !view.is_visible() {
        return;
    }
    let width = if view.show_timer {
        SIZE.x
    } else {
        COMPACT_WIDTH
    };
    let rect = Rect::from_center_size(ui.max_rect().center(), vec2(width, SIZE.y));
    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(22),
        BACKGROUND,
        Stroke::new(1.0, Color32::from_white_alpha(28)),
        egui::StrokeKind::Inside,
    );

    if view.state == State::Transcribing && !view.show_timer {
        let spinner = Rect::from_center_size(rect.center(), vec2(16.0, 16.0));
        ui.put(spinner, egui::Spinner::new().size(16.0).color(MUTED));
        return;
    }
    if view.state == State::Transcribing {
        let spinner =
            Rect::from_center_size(pos2(rect.left() + 28.0, rect.center().y), vec2(16.0, 16.0));
        ui.put(spinner, egui::Spinner::new().size(16.0).color(MUTED));
        ui.painter().text(
            pos2(rect.left() + 46.0, rect.center().y),
            Align2::LEFT_CENTER,
            view.transcribing,
            FontId::proportional(14.0),
            MUTED,
        );
        return;
    }

    // Grey and flat while the microphone starts, grey bars following the level once it
    // hears the room, and bright with a running timer from the first word.
    let (color, level) = match (view.live, view.elapsed) {
        (false, _) => (MUTED, 0.0),
        (true, None) => (MUTED, view.level),
        (true, Some(_)) if view.state == State::SoftCapWarning => (WARNING, view.level),
        (true, Some(_)) => (FOREGROUND, view.level),
    };
    let time = ui.input(|i| i.time);
    draw_bars(
        painter,
        pos2(rect.left() + PADDING, rect.center().y),
        level,
        time,
        color,
    );
    if !view.show_timer {
        return;
    }
    painter.text(
        pos2(rect.right() - PADDING, rect.center().y),
        Align2::RIGHT_CENTER,
        format_elapsed(view.elapsed.unwrap_or_default()),
        FontId::monospace(15.0),
        color,
    );
}

/// Bars whose heights follow the level, each oscillating on its own phase so the
/// waveform looks alive (the same motion as in the Swift predecessor).
fn draw_bars(painter: &egui::Painter, left_center: Pos2, level: f32, time: f64, color: Color32) {
    let level = level.clamp(0.0, 1.0);
    for i in 0..BAR_COUNT {
        #[allow(clippy::cast_precision_loss)]
        let index = i as f32;
        #[allow(clippy::cast_possible_truncation)]
        let osc = f32::midpoint((time * 9.0 + f64::from(index) * 0.8).sin() as f32, 1.0);
        let height = (BAR_MAX_HEIGHT * level * (0.4 + 0.6 * osc)).max(BAR_MIN_HEIGHT);
        let x = left_center.x + index * (BAR_WIDTH + BAR_GAP);
        let bar = Rect::from_center_size(
            pos2(x + BAR_WIDTH / 2.0, left_center.y),
            vec2(BAR_WIDTH, height),
        );
        painter.rect_filled(bar, CornerRadius::same(2), color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elapsed_is_seconds_with_one_decimal() {
        assert_eq!(format_elapsed(Duration::ZERO), "0.0s");
        assert_eq!(format_elapsed(Duration::from_millis(6_349)), "6.3s");
        assert_eq!(
            format_elapsed(Duration::from_millis(6_399)),
            "6.3s",
            "never rounds up"
        );
        assert_eq!(format_elapsed(Duration::from_secs(75)), "75.0s");
    }

    #[test]
    fn only_recording_and_transcribing_are_visible() {
        let view = |state| View {
            state,
            live: false,
            elapsed: None,
            level: 0.0,
            show_timer: false,
            transcribing: "",
        };
        assert!(view(State::Recording).is_visible());
        assert!(view(State::SoftCapWarning).is_visible());
        assert!(view(State::Transcribing).is_visible());
        for state in [
            State::Idle,
            State::Paused,
            State::ModelLoading,
            State::ModelUnavailable,
        ] {
            assert!(!view(state).is_visible(), "{state:?}");
        }
    }
}
