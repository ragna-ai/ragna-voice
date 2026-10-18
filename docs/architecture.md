# RAGNA Voice architecture

> **Last reviewed:** 2026-10-11 · **Last updated:** 2026-10-11 · **Change:** major (first version, written for the workspace skeleton)

How RAGNA Voice is built today. Domain vocabulary is in [`CONTEXT.md`](../CONTEXT.md), decisions are in [`adr/`](adr).

## Shape

One Cargo workspace (`Cargo.toml`, edition 2024, `MIT OR Apache-2.0`): six library crates in `crates/`, two binaries in `apps/`, build tooling in `xtask/`. Rust only.

| Binary | What it is |
|---|---|
| `apps/ragna-voice` | The menu-bar app: eframe/egui on wgpu, tray icon via `tray-icon`, no Dock icon |
| `apps/ragna-voice-cli` | Headless frontend over the same crates: `transcribe <file.wav>` runs one Dictation with the file as the microphone |

## Layers

A crate may depend only on crates in lower layers.

| Layer | Crate | Role |
|---|---|---|
| L0 domain | `core` | Dictation state machine, Dictation outcomes, limits, Silence Guard, Hotkey chord rules. Pure: no OS, audio, Model or UI dependencies |
| L0 assets | `icon` | The mark, drawn in code (no font or image files). Used by the app (menu bar) and `xtask` (app icon) |
| L1 services | `audio`, `transcribe`, `models` | Microphone capture into an in-RAM Recording; whisper.cpp bridge from Recording to Transcript; Model store and the one user-initiated download |
| L2 platform | `platform` | Hotkey, Esc cancel, Insertion, permissions, notifications. The only crate allowed `unsafe` (`AGENTS.md` §4) |
| L3 frontends | `ui-egui`, `apps/*` | egui views (Settings, first-run wizard, waveform overlay); the binaries that wire everything together |

Nothing below L3 depends on egui, eframe, winit or tray-icon.

## App shell

- `eframe` owns the main-thread event loop. On macOS the winit activation policy is `Accessory`, the equivalent of `LSUIElement`.
- The Settings window starts hidden. Closing it hides it again. Only Quit ends the app.
- Tray menu events arrive on another thread. They are forwarded over a channel, and `ctx.request_repaint()` wakes eframe, which calls `App::logic` even while the window is hidden.

## Dictation state machine

`crates/core::Dictation` is sans-IO ([ADR-0002](adr/ADR-0002.md)). The app feeds it `Event`s with the current time and executes the `Effect`s it returns; `poll_timeout` says when to call `handle_timeout` for the soft and hard caps. Core never reads the clock, spawns a thread or touches the OS.

The runner (`apps/ragna-voice/src/runner.rs`) is its only caller: one `dictation` thread that receives events over a channel, waits on `poll_timeout`, and executes effects. Transcription runs on a separate `transcriber` thread that loads the Model and reports back with events. Insertion (`platform::insert`) pastes with ⌘V and leaves the Transcript on the clipboard (restoring it made Chrome drop pastes). Esc-to-cancel is a Carbon hotkey (⌃⌥Esc and Esc) registered only while the Hotkey is held, so it needs no permission; the main thread (un)registers it on the runner's request. A finished Dictation's notice (paste failed, hard cap, transcription failed) is posted through Notification Center (`platform::notify`); permission is asked the first time one is posted.

## Safety

- `unsafe_code` is denied workspace-wide; `crates/platform` is the only exception.
- Every crate denies `unwrap`/`expect`/`panic!` outside tests (`AGENTS.md` §4).

## Build and quality gates

`cargo xtask ci` runs fmt, clippy (`-D warnings`, `pedantic`) and tests. `deny.toml` configures `cargo deny check` (dependency licences, advisories, sources). There is no hosted CI yet.

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-11 | major | Created for the workspace skeleton |
| 2026-10-11 | minor | Added the sans-IO Dictation state machine (ADR-0002) |
| 2026-10-11 | minor | Added `crates/icon` and generated app icon |
