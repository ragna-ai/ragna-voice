# AGENTS.md: rules for every agent and contributor

These rules bind every AI agent and every human contributor working in this repository. They come before any task, instruction or deadline. `CLAUDE.md` holds the day-to-day working instructions. If the two ever disagree, **this file wins.**

---

## 1. Asset policy

An **asset** is any non-code material the project includes, ships, embeds or shows in its documentation: icons, images, screenshots, logos, fonts, sounds, Models and test audio.

### 1.1 Only open, provable licences

Every asset must be one of:
- **Open source:** ISC, MIT, BSD-2-Clause, BSD-3-Clause, Apache-2.0, OFL-1.1 or Ubuntu-Font-1.0.
- **Creative Commons:** CC0-1.0, CC-BY-4.0 or CC-BY-SA-4.0.
- **Public domain.**
- **Contributor-original:** made by a contributor who owns it and licenses it under the project licence (`MIT OR Apache-2.0`).

Anything else is forbidden, including assets whose licence you cannot prove from the source. In particular:
- **No SF Symbols and no Apple system fonts as bundled assets.** Their licences tie them to Apple platforms, so they can't ship in a cross-platform build or appear in docs. Draw icons in code or use an openly licensed set (e.g. Lucide).
- No stock photos, "free for personal use" material, or screenshots of other people's software.
- **Test audio** must be synthetic or recorded by a contributor who consents to publishing it. Never commit recordings of other people's voices.

**Models** (whisper.cpp `ggml-*.bin`) are never committed. They are fetched pinned by SHA-256 from their original source.

### 1.2 Every asset is attributed

`ATTRIBUTION.md` lists every committed or bundled asset: path, author, source (URL with version or commit), SPDX licence and how it is used. If an asset's origin or licence is unclear, **do not add it.** Ask the user.

## 2. Privacy

These are product commitments from `CONTEXT.md` (Properties). Code must never weaken them.

- **Audio stays in RAM.** No Recording is ever written to disk, a temp file, a cache or a log.
- **Transcripts are forgotten.** Never persist or log a Transcript, not even at `debug` or `trace`.
- **One network call total:** the user-initiated Model download. No telemetry, analytics, crash reporting or update checks. Adding any network call needs an ADR and the user's explicit approval.
- **Test fixtures are synthetic.** Never use a real user's audio or Transcript in a test.

## 3. Headless first

Every piece of the pipeline that does not need the OS (Silence Guard, transcription, Model management, the Dictation state machine) must be reachable from `ragna-voice-cli`, so agents and tests can exercise it without a microphone, a hotkey or a GUI. A feature is not done until it has a test that runs headlessly.

## 4. Never crash

RAGNA Voice runs all day in the menu bar. A missing microphone, a corrupt Model file, a bad settings file, a full disk or a Target App that rejects a paste must produce an outcome the user can act on (see Dictation outcome in `CONTEXT.md`). It must never crash, and never lose a Transcript the user has spoken. **This outranks feature work.**

- **Fail with `Result`.** Non-test code returns errors through the crate's error type and `?`.
- **No panicking shortcuts.** No `unwrap()`, `expect()`, `panic!`, `unreachable!`, `todo!` or `unimplemented!` outside tests, unless the call is provably infallible, with a comment saying why. `assert!` only for internal invariants that input cannot reach.
- **`unsafe` lives only in `crates/platform`.** The workspace denies `unsafe_code`. `crates/platform` may allow it for OS APIs (CGEventTap, AppKit) that have no safe wrapper. Every `unsafe` block carries a `// SAFETY:` comment. Prefer a maintained safe wrapper crate when one exists.
- **Treat input-derived numbers as hostile.** Audio buffers, sample rates, Model file sizes and settings values come from outside. Use `get()` instead of `[i]`, checked or saturating arithmetic for lengths and durations, and guard against division by zero and NaN/inf casts.
- **Don't cascade.** Handle lock poisoning. A panic in the audio or inference thread must end that Dictation as Failed, not take the app down.
- **Prove it.** Every crash fix comes with a small synthetic regression test that panicked before the fix.
- **Enforced by clippy.** Root `clippy.toml` lets tests use `unwrap`/`expect`/`panic!`. Every crate carries `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]`; new crates start with it.

## 5. Everything else

Follow `CLAUDE.md`. It covers quality gates, layering, commit rules and how to run and look at the app.
