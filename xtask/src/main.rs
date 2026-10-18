//! Repository automation: `cargo xtask <task>`.

#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::{env, fs};

const USAGE: &str = "usage: cargo xtask <task>

tasks:
  ci       run the quality gates (fmt, clippy, test)
  icons    render the icons to target/icons (PNGs, plus a menu bar preview)
  models   download the bundled Model to target/models (SHA-256 checked, cached)
  bundle   build and sign target/release/RAGNA Voice Dev.app (io.ragna.voice.dev);
           with --release, target/release/RAGNA Voice.app (io.ragna.voice)
  run      bundle, then launch the app; logs go to target/ragna-voice.log

env:
  RAGNA_VOICE_SIGN_IDENTITY  codesign identity for bundle/run (default: - , ad-hoc)
  RAGNA_VOICE_MODEL, RUST_LOG, RAGNA_VOICE_DEMO_OVERLAY  passed through to the app by run";

/// Which app a bundle is. Dev builds get their own identity, so macOS keeps their
/// permissions apart from the installed release and `run` never kills the release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Variant {
    Dev,
    Release,
}

impl Variant {
    fn name(self) -> &'static str {
        match self {
            Self::Dev => "RAGNA Voice Dev",
            Self::Release => "RAGNA Voice",
        }
    }

    fn bundle_id(self) -> &'static str {
        match self {
            Self::Dev => "io.ragna.voice.dev",
            Self::Release => "io.ragna.voice",
        }
    }

    fn executable(self) -> &'static str {
        match self {
            Self::Dev => "ragna-voice-dev",
            Self::Release => "ragna-voice",
        }
    }
}

/// The cargo binary that goes into the bundle.
const BINARY: &str = "ragna-voice";

type Result<T = (), E = String> = std::result::Result<T, E>;

fn main() -> ExitCode {
    let result = match env::args().nth(1).as_deref() {
        Some("ci") => ci(),
        Some("icons") => icons().map(|_| ()),
        Some("models") => models().map(|_| ()),
        Some("bundle") => match env::args().nth(2).as_deref() {
            None => bundle(Variant::Dev, &sign_identity()).map(|_| ()),
            Some("--release") => bundle(Variant::Release, &sign_identity()).map(|_| ()),
            Some(_) => Err(USAGE.to_string()),
        },
        Some("run") => run(),
        _ => Err(USAGE.to_string()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

/// Runs `program` with `args`, echoing the command, and fails on a non-zero exit.
fn sh(program: &str, args: &[&str]) -> Result {
    eprintln!("$ {program} {}", args.join(" "));
    let status = Command::new(program)
        .args(args)
        .current_dir(root())
        .status()
        .map_err(|e| format!("{program}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} failed: {status}"))
    }
}

fn ci() -> Result {
    let cargo = env!("CARGO");
    sh(cargo, &["fmt", "--all", "--check"])?;
    sh(
        cargo,
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    sh(cargo, &["test", "--workspace"])
}

/// Builds the release binary and wraps it in a signed `.app`, so macOS attributes
/// permission grants to RAGNA Voice instead of the terminal.
/// Ad-hoc by default, so every dev build exercises the permission flow (`CLAUDE.md`).
fn sign_identity() -> String {
    env::var("RAGNA_VOICE_SIGN_IDENTITY").unwrap_or_else(|_| "-".into())
}

fn bundle(variant: Variant, identity: &str) -> Result<PathBuf> {
    sh(env!("CARGO"), &["build", "--release", "-p", "ragna-voice"])?;
    let root = root();
    let app = root
        .join("target/release")
        .join(format!("{}.app", variant.name()));
    let contents = app.join("Contents");
    let io = |e: std::io::Error| e.to_string();
    if app.exists() {
        fs::remove_dir_all(&app).map_err(io)?;
    }
    fs::create_dir_all(contents.join("MacOS")).map_err(io)?;
    fs::copy(
        root.join("target/release").join(BINARY),
        contents.join("MacOS").join(variant.executable()),
    )
    .map_err(io)?;
    let iconset = icons()?.join("AppIcon.iconset");
    fs::create_dir_all(contents.join("Resources")).map_err(io)?;
    let icns = contents.join("Resources/AppIcon.icns");
    sh(
        "iconutil",
        &[
            "--convert",
            "icns",
            "--output",
            icns.to_str().ok_or("non-UTF-8 target path")?,
            iconset.to_str().ok_or("non-UTF-8 target path")?,
        ],
    )?;
    let model = models()?;
    let file_name = model
        .file_name()
        .ok_or("bundled Model path has no file name")?;
    fs::copy(&model, contents.join("Resources").join(file_name)).map_err(io)?;
    // Localised Info.plist strings (the microphone prompt), one folder per language.
    for language in ["en", "de"] {
        let folder = format!("{language}.lproj");
        let target = contents.join("Resources").join(&folder);
        fs::create_dir_all(&target).map_err(io)?;
        fs::copy(
            root.join("packaging/macos")
                .join(&folder)
                .join("InfoPlist.strings"),
            target.join("InfoPlist.strings"),
        )
        .map_err(io)?;
    }
    let plist = fs::read_to_string(root.join("packaging/macos/Info.plist")).map_err(io)?;
    fs::write(
        contents.join("Info.plist"),
        plist
            .replace("{{VERSION}}", env!("CARGO_PKG_VERSION"))
            .replace("{{BUNDLE_ID}}", variant.bundle_id())
            .replace("{{NAME}}", variant.name())
            .replace("{{EXECUTABLE}}", variant.executable()),
    )
    .map_err(io)?;

    let app_path = app.to_str().ok_or("non-UTF-8 target path")?;
    let mut args = vec![
        "--force",
        "--options",
        "runtime",
        "--entitlements",
        "packaging/macos/entitlements.plist",
        "--sign",
        identity,
    ];
    // Notarization requires a secure timestamp; ad-hoc signatures can't have one.
    if identity != "-" {
        args.push("--timestamp");
    }
    args.push(app_path);
    sh("codesign", &args)?;
    // The designated requirement is what macOS ties permission grants to.
    sh("codesign", &["--display", "--requirements", "-", app_path])?;
    Ok(app)
}

/// Renders the icons into `target/icons`: the app icon as an `.iconset` (for
/// `iconutil`), the menu bar icon, and a preview of it on light and dark menu bars.
fn icons() -> Result<PathBuf> {
    let dir = root().join("target/icons");
    let iconset = dir.join("AppIcon.iconset");
    fs::create_dir_all(&iconset).map_err(|e| e.to_string())?;
    for points in [16, 32, 128, 256, 512] {
        write_png(
            &iconset.join(format!("icon_{points}x{points}.png")),
            (points, points),
            &ragna_voice_icon::app_icon(points),
        )?;
        let px = points * 2;
        write_png(
            &iconset.join(format!("icon_{points}x{points}@2x.png")),
            (px, px),
            &ragna_voice_icon::app_icon(px),
        )?;
    }
    write_png(
        &dir.join("menu-bar.png"),
        (44, 44),
        &ragna_voice_icon::menu_bar(44),
    )?;
    write_png(
        &dir.join("menu-bar-preview.png"),
        (PREVIEW_WIDTH, PREVIEW_HEIGHT),
        &menu_bar_preview(),
    )?;
    eprintln!("icons: {}", dir.display());
    Ok(dir)
}

/// The menu bar preview: two 44 px tiles side by side, scaled up 4×.
const PREVIEW_SCALE: u32 = 4;
const PREVIEW_WIDTH: u32 = 2 * 44 * PREVIEW_SCALE;
const PREVIEW_HEIGHT: u32 = 44 * PREVIEW_SCALE;

/// The menu bar icon tinted as macOS shows it: black on a light bar, white on a dark one.
fn menu_bar_preview() -> Vec<u8> {
    let icon = ragna_voice_icon::menu_bar(44);
    let mut out = Vec::with_capacity((PREVIEW_WIDTH * PREVIEW_HEIGHT * 4) as usize);
    for y in 0..PREVIEW_HEIGHT {
        for x in 0..PREVIEW_WIDTH {
            let dark = x >= PREVIEW_WIDTH / 2;
            let (ix, iy) = ((x % (PREVIEW_WIDTH / 2)) / PREVIEW_SCALE, y / PREVIEW_SCALE);
            let alpha = icon
                .get(((iy * 44 + ix) * 4 + 3) as usize)
                .copied()
                .unwrap_or(0);
            let (bar, ink) = if dark {
                (0x2a_u16, 0xff_u16)
            } else {
                (0xec, 0x00)
            };
            let a = u16::from(alpha);
            let v = u8::try_from((ink * a + bar * (255 - a)) / 255).unwrap_or(0);
            out.extend_from_slice(&[v, v, v, 255]);
        }
    }
    out
}

fn write_png(path: &Path, (width, height): (u32, u32), rgba: &[u8]) -> Result {
    let file = fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(rgba).map_err(|e| e.to_string())
}

/// The bundled Model, downloaded once into `target/models` and verified by checksum.
fn models() -> Result<PathBuf> {
    use std::sync::atomic::AtomicBool;

    let model = ragna_voice_models::BUNDLED;
    let store = ragna_voice_models::Store::new(root().join("target/models"), None);
    if let Some(path) = store.path(&model) {
        return Ok(path);
    }
    eprintln!(
        "downloading {} ({} MB)",
        model.url(),
        model.size_bytes / 1_000_000
    );
    let mut shown = 0;
    let path = store
        .download(
            &model,
            |received, total| {
                let percent = received * 100 / total.max(1);
                if percent >= shown + 10 {
                    shown = percent;
                    eprintln!("  {percent}%");
                }
            },
            &AtomicBool::new(false),
        )
        .map_err(|e| format!("bundled Model: {e}"))?;
    eprintln!("verified {}", path.display());
    Ok(path)
}

fn run() -> Result {
    let app = bundle(Variant::Dev, &sign_identity())?;
    // One instance at a time; ignore "no process found".
    let _ = Command::new("pkill")
        .args(["-x", Variant::Dev.executable()])
        .status();
    // Both apps would register the same Hotkey and paste every Dictation twice, so the
    // installed release quits (cleanly, as if from its menu) while the dev build runs.
    let release_running = Command::new("pgrep")
        .args(["-x", Variant::Release.executable()])
        .stdout(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    if release_running {
        eprintln!(
            "quitting the installed {} while the dev build runs",
            Variant::Release.name()
        );
        let script = format!(
            "tell application id \"{}\" to quit",
            Variant::Release.bundle_id()
        );
        let _ = Command::new("osascript").args(["-e", &script]).status();
    }
    let log = root().join("target/ragna-voice.log");
    let log = log.to_str().ok_or("non-UTF-8 target path")?;
    let app = app.to_str().ok_or("non-UTF-8 target path")?;
    let mut args = vec!["-n", "--stdout", log, "--stderr", log];
    let passthrough: Vec<String> = ["RAGNA_VOICE_MODEL", "RUST_LOG", "RAGNA_VOICE_DEMO_OVERLAY"]
        .iter()
        .filter_map(|key| env::var(key).ok().map(|value| format!("{key}={value}")))
        .collect();
    for pair in &passthrough {
        args.extend(["--env", pair]);
    }
    args.push(app);
    sh("open", &args)?;
    eprintln!("logs: tail -f {log}");
    Ok(())
}
