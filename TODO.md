# TODO

Open items, roughly in priority order within each section. Check items off as they land.

## Product gaps (vs. the Swift predecessor)

- [ ] **Pause from the menu bar.** Core already has `Event::TogglePause` and `State::Paused` (tested), but nothing in the app sends it. Add a "Pause dictation" menu item, as in the Swift predecessor.
- [ ] **Menu bar icon shows state.** Recording, past the 60 s soft cap (amber), and "no Model" (a `!` badge). The icon is static today.
- [ ] **Choose the Hotkey.** ⌃⌥Space is fixed. Port the Swift predecessor's chord rules (`HotkeyChordValidator.swift`, modifier chords only) and add a recorder to Settings. Esc-to-cancel must follow the chosen modifiers (`platform::hotkey`).
- [ ] **Launch at login.** A switch in Settings (`SMAppService`), instead of adding it by hand under Login Items.
- [ ] **First-run flow.** Today Settings opens when a permission is missing. Decide whether that is enough or a short wizard (permissions, Hotkey, Model) is worth it.

## Reliability

- [ ] **Log file for release builds.** The installed app's logs go nowhere, so there is nothing to attach to a bug report. Write to `~/Library/Application Support/RAGNA-Voice/logs/` with rotation. Never log Transcripts.
- [ ] **Survive a panic on the `dictation` thread.** The transcriber and Insertion catch panics, but a panic in the runner loop itself would leave the Hotkey dead with the app still running. Catch it and restart the runner, or end the Dictation as Failed.
- [ ] **Notifications while Settings is open.** macOS hides notifications for the frontmost app unless a `UNUserNotificationCenterDelegate` allows them (`willPresent`).
- [ ] **Whisper hallucinations.** Filter non-speech tokens such as `[BLANK_AUDIO]` and `(music)` from Transcripts before Insertion.
- [ ] **Overlay on multiple displays.** The pill is placed on the primary display at launch. Follow the display with the focused window.

## Models

- [ ] **First-launch delay.** The first launch of each new build takes about 9 s to load the Model while macOS compiles ggml's Metal shaders (68 ms afterwards). Ship a precompiled `default.metallib`, or show "Preparing…" in the overlay.
- [ ] **Large v3 Turbo.** Add `ggml-large-v3-turbo-q5_0` (574 MB, SHA-256 `394221709cd5…a7e2`) to the catalogue: much faster than Large v3 at similar quality.
- [ ] **Core ML encoder.** The Swift predecessor shipped `ggml-base.en-encoder.mlmodelc` for faster encoding on the Neural Engine. Evaluate whisper-rs's `coreml` feature.
- [ ] **Downloads.** Resume interrupted downloads, and check free disk space before starting a large one (Large v3 is 3.1 GB).
- [ ] **Microphone choice.** Recording always uses the system default input device.

## Release and distribution

- [ ] **DMG.** ADR-0001 specifies a signed, notarized drag-to-Applications DMG. Releases are a notarized `.app` only so far.
- [ ] **Versioning.** Bump the version, keep a `CHANGELOG.md`, tag releases. Everything is still `0.1.0`.
- [ ] **Push and CI.** Create `ragna-ai/ragna-voice`, push, and restore CI (`cargo xtask ci` plus `cargo-deny`). macOS runners bill about 10× in private repos; consider Linux for the platform-independent crates.
- [x] **Licence.** MIT OR Apache-2.0, decided 2026-10-12. RAGNA Voice stays free, with no paid tier.

## Engineering

- [ ] **Layering check.** `crates/core` must not depend on OS, audio or UI crates (`docs/architecture.md`). Enforce it in `cargo xtask ci`.
- [ ] **Tests for the runner and UI.** The state machine is well tested; the runner, Settings and overlay are not. Add runner tests with fake effects, and egui UI tests (`egui_kittest`).
- [ ] **CLI options.** `ragna-voice-cli transcribe` always uses Automatic. Add `--language` and `--model`.
- [ ] **Presses during transcription** are ignored (ADR-0002). Queuing them is the alternative; revisit if users hit it.

## Housekeeping

- [ ] **Retire the Swift predecessor.** Archive the repo once RAGNA Voice covers it, and remove its entries from Accessibility and Microphone in System Settings.
