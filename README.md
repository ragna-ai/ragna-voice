<div align="center">

# RAGNA Voice

#### Hold a key, speak, release. The transcript lands in whatever app has focus.

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue?style=flat)](#license)
[![No telemetry](https://img.shields.io/badge/telemetry-none-brightgreen?style=flat)](#privacy)

</div>

RAGNA Voice is a menu-bar dictation app that runs [whisper.cpp](https://github.com/ggml-org/whisper.cpp) locally. No audio ever leaves your machine. Built with [egui](https://github.com/emilk/egui).

## Requirements

- Apple Silicon Mac, macOS 14 or later (v1 target)
- Rust stable (see `rust-toolchain.toml`)

## Get started

```sh
cargo xtask run                 # build, bundle (with the Model) and launch the app
cargo run -p ragna-voice-cli -- transcribe f.wav  # headless pipeline
cargo xtask ci                  # fmt, clippy, tests: run before every commit
```

## Run in development

```sh
cargo xtask run                                # build, sign and launch RAGNA Voice Dev.app
tail -f target/ragna-voice.log                 # follow the dev app's log
RUST_LOG=debug cargo xtask run                 # log every Dictation event and effect
RAGNA_VOICE_DEMO_OVERLAY=1 cargo xtask run     # keep the overlay on screen with a fake Recording
RAGNA_VOICE_SIGN_IDENTITY="Apple Development: …" cargo xtask run  # keep permissions across rebuilds
cargo run -p ragna-voice                       # bare binary; permission grants go to the terminal
RAGNA_VOICE_MODEL=path/to/ggml-base.en.bin \
  cargo run -p ragna-voice-cli -- transcribe f.wav  # headless pipeline, no mic or Hotkey
RAGNA_VOICE_MODEL=path/to/ggml-base.en.bin \
  cargo test -- --ignored                      # tests that need a real Model
cargo xtask models                             # download the bundled Model to target/models
cargo xtask icons                              # render the icons to target/icons
```

`cargo xtask run` quits the installed release first, since both answer the same Hotkey. Reopen it from /Applications afterwards. Dev builds are signed ad-hoc, so each rebuild starts with no permissions. That is on purpose: it keeps the first-run permission flow tested.

## How it's built

A Cargo workspace of focused crates. The core never depends on the OS or the UI.

| Crate | What it does |
|---|---|
| `crates/core` | Dictation domain model and state machine. Pure, no OS deps |
| `crates/audio` | Microphone capture into an in-RAM Recording |
| `crates/transcribe` | whisper.cpp bridge: Recording to Transcript |
| `crates/models` | Model store: bundled Model plus user-initiated downloads |
| `crates/platform` | Hotkey, Esc cancel, Insertion, permissions, notifications |
| `crates/ui-egui` | Settings, first-run wizard, waveform overlay |
| `crates/icon` | The mark (rounded square with an R), drawn in code: menu bar and app icon |
| `apps/ragna-voice` | The menu-bar app |
| `apps/ragna-voice-cli` | Headless frontend over the same crates |
| `xtask` | Repository automation (`cargo xtask <task>`) |

See [`docs/architecture.md`](docs/architecture.md) for layering, [`CONTEXT.md`](CONTEXT.md) for the domain vocabulary and product invariants, and [`docs/adr/`](docs/adr) for decisions.

## Contributing

Read [`AGENTS.md`](AGENTS.md) (binding rules for humans and agents) and [`CLAUDE.md`](CLAUDE.md) (day-to-day working instructions). Run `cargo xtask ci` before every commit.

## Privacy

One network call total in v1: an explicit, user-initiated Model download from HuggingFace. No analytics, no auto-update check, no crash reporting. Recordings live in RAM and are discarded after transcription.

## License

Copyright (C) 2026 Sven Stadhouders.

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.

The RAGNA name and the RAGNA Voice mark (the rounded square with an R, drawn by `crates/icon`) are trademarks of Sven Stadhouders. The code licence grants no rights to them. Forks and modified versions must use their own name and icon. See [TRADEMARKS.md](TRADEMARKS.md).
