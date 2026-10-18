//! The RAGNA Voice mark: a rounded square with an R, drawn in code so it needs no font
//! or image asset (`AGENTS.md` §1). Used for the menu bar icon and the app icon.
//!
//! Geometry is in unit coordinates of the square, `y` pointing down.

#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

/// RGBA pixels, row by row.
pub type Rgba = Vec<u8>;

/// RAGNA brand colours (`ragna-website/docs/brand/colors.md`).
const NAVY_950: [u8; 3] = [0x08, 0x15, 0x23];
const TEAL_400: [u8; 3] = [0x50, 0xcf, 0xbd];

/// Samples per pixel along each axis, for anti-aliasing.
const SUPERSAMPLE: u32 = 4;

/// The menu bar icon as a template image: black where opaque, so macOS tints it to match
/// the menu bar. A filled rounded square with the R knocked out, 16 pt on a 22 pt canvas
/// (tray-icon shows images at most 22 pt high). `px` is the canvas size in pixels; use
/// 44 for Retina.
#[must_use]
pub fn menu_bar(px: u32) -> Rgba {
    render(px, 16.0 / 22.0, 0.25, |inside_square, inside_letter| {
        let alpha = if inside_square && !inside_letter {
            1.0
        } else {
            0.0
        };
        ([0, 0, 0], alpha)
    })
}

/// The app icon: a navy rounded square with a teal R, on Apple's icon grid (the square
/// fills 824 of 1024 pixels, centred). `px` is the canvas size in pixels.
#[must_use]
pub fn app_icon(px: u32) -> Rgba {
    render(
        px,
        824.0 / 1024.0,
        0.225,
        |inside_square, inside_letter| match (inside_square, inside_letter) {
            (true, true) => (TEAL_400, 1.0),
            (true, false) => (NAVY_950, 1.0),
            (false, _) => ([0, 0, 0], 0.0),
        },
    )
}

/// Renders the mark on a `px`×`px` canvas. The square takes `scale` of the canvas and
/// has corners of `radius` (relative to its side). `shade` maps (inside square, inside
/// letter) to a colour and opacity; edges are anti-aliased by supersampling.
fn render(px: u32, scale: f64, radius: f64, shade: impl Fn(bool, bool) -> ([u8; 3], f64)) -> Rgba {
    let mut out = Vec::with_capacity(px as usize * px as usize * 4);
    let canvas = f64::from(px);
    let square = canvas * scale;
    let origin = (canvas - square) / 2.0;
    let samples = f64::from(SUPERSAMPLE * SUPERSAMPLE);
    for y in 0..px {
        for x in 0..px {
            let (mut rgb, mut alpha) = ([0.0_f64; 3], 0.0_f64);
            for sy in 0..SUPERSAMPLE {
                for sx in 0..SUPERSAMPLE {
                    let fx = (f64::from(x) + (f64::from(sx) + 0.5) / f64::from(SUPERSAMPLE)
                        - origin)
                        / square;
                    let fy = (f64::from(y) + (f64::from(sy) + 0.5) / f64::from(SUPERSAMPLE)
                        - origin)
                        / square;
                    let (color, a) = shade(in_rounded_square(fx, fy, radius), in_letter_r(fx, fy));
                    for (sum, c) in rgb.iter_mut().zip(color) {
                        *sum += f64::from(c) * a;
                    }
                    alpha += a;
                }
            }
            // Average colour over the covered samples (straight, not premultiplied, alpha).
            for sum in rgb {
                out.push(to_u8(if alpha > 0.0 { sum / alpha } else { 0.0 }));
            }
            out.push(to_u8(alpha / samples * 255.0));
        }
    }
    out
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn to_u8(v: f64) -> u8 {
    v.round().clamp(0.0, 255.0) as u8
}

fn in_rounded_square(x: f64, y: f64, r: f64) -> bool {
    if !(0.0..=1.0).contains(&x) || !(0.0..=1.0).contains(&y) {
        return false;
    }
    // Distance from the inner rectangle the corners are rounded around.
    let dx = (r - x).max(x - (1.0 - r)).max(0.0);
    let dy = (r - y).max(y - (1.0 - r)).max(0.0);
    dx.hypot(dy) <= r
}

// The R: a stem, a bowl (half ring) and a diagonal leg, all with the same stroke.
// A light stroke; the counter (the hole in the bowl) stays open at 16 pt.
const STROKE: f64 = 0.09;
const LEFT: f64 = 0.31;
const TOP: f64 = 0.22;
const BOTTOM: f64 = 0.78;
/// Centre and outer radius of the bowl's round part.
const BOWL_X: f64 = 0.50;
const BOWL_R: f64 = 0.17;
const BOWL_Y: f64 = TOP + BOWL_R;

fn in_letter_r(x: f64, y: f64) -> bool {
    let stem = (LEFT..=LEFT + STROKE).contains(&x) && (TOP..=BOTTOM).contains(&y);
    stem || in_bowl(x, y) || in_leg(x, y)
}

fn in_bowl(x: f64, y: f64) -> bool {
    let inner = BOWL_R - STROKE;
    let outer_shape = if x <= BOWL_X {
        (LEFT..=BOWL_X).contains(&x) && (BOWL_Y - BOWL_R..=BOWL_Y + BOWL_R).contains(&y)
    } else {
        (x - BOWL_X).hypot(y - BOWL_Y) <= BOWL_R
    };
    let counter = if x <= BOWL_X {
        (LEFT + STROKE..=BOWL_X).contains(&x) && (BOWL_Y - inner..=BOWL_Y + inner).contains(&y)
    } else {
        (x - BOWL_X).hypot(y - BOWL_Y) < inner
    };
    outer_shape && !counter
}

/// A thick line from the bowl's bottom down to the baseline, cut flat at the baseline.
fn in_leg(x: f64, y: f64) -> bool {
    let (ax, ay) = (BOWL_X - 0.04, BOWL_Y + BOWL_R - STROKE / 2.0);
    let (bx, by) = (0.68, BOTTOM - STROKE / 2.0);
    let (dx, dy) = (bx - ax, by - ay);
    let t = (((x - ax) * dx + (y - ay) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    let distance = (x - (ax + t * dx)).hypot(y - (ay + t * dy));
    distance <= STROKE / 2.0 && y <= BOTTOM
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha_at(rgba: &[u8], px: u32, x: u32, y: u32) -> u8 {
        rgba[((y * px + x) * 4 + 3) as usize]
    }

    #[test]
    fn sizes_are_px_squared_rgba() {
        assert_eq!(menu_bar(44).len(), 44 * 44 * 4);
        assert_eq!(app_icon(64).len(), 64 * 64 * 4);
    }

    #[test]
    fn menu_bar_is_transparent_at_the_edges_and_cut_out_at_the_stem() {
        let icon = menu_bar(44);
        assert_eq!(alpha_at(&icon, 44, 0, 0), 0, "canvas corner");
        assert_eq!(alpha_at(&icon, 44, 22, 8), 255, "square, above the letter");
        // The stem's middle: x ≈ 0.355 of the 32 px square (+ 6 px margin), y = 0.5.
        assert_eq!(alpha_at(&icon, 44, 6 + 11, 22), 0, "knocked-out stem");
    }

    #[test]
    fn app_icon_is_navy_with_a_teal_letter() {
        let px = 1024;
        let icon = app_icon(px);
        let pixel = |x: u32, y: u32| {
            let i = ((y * px + x) * 4) as usize;
            [icon[i], icon[i + 1], icon[i + 2], icon[i + 3]]
        };
        assert_eq!(
            pixel(512, 150),
            [0x08, 0x15, 0x23, 255],
            "navy above the letter"
        );
        // Stem centre: 100 px margin + 0.355 × 824.
        assert_eq!(pixel(100 + 292, 512), [0x50, 0xcf, 0xbd, 255], "teal stem");
        assert_eq!(pixel(10, 10)[3], 0, "transparent outside Apple's grid");
    }
}
