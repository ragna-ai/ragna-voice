# CLAUDE.md

Rules that bind every change are in `AGENTS.md`. If this file and `AGENTS.md` disagree, `AGENTS.md` wins.

Behavioral guidelines to reduce common LLM coding mistakes. Merge with project-specific instructions as needed.

**Tradeoff:** These guidelines bias toward caution over speed. For trivial tasks, use judgment.

## 1. Think Before Coding

**Don't assume. Don't hide confusion. Surface tradeoffs.**

Before implementing:

- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them - don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

## 2. Simplicity First

**Minimum code that solves the problem. Nothing speculative.**

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

## 3. Surgical Changes

**Touch only what you must. Clean up only your own mess.**

When editing existing code:

- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it - don't delete it.

When your changes create orphans:

- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: Every changed line should trace directly to the user's request.

## 4. Goal-Driven Execution

**Define success criteria. Loop until verified.**

Transform tasks into verifiable goals:

- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:

```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.

---

**These guidelines are working if:** fewer unnecessary changes in diffs, fewer rewrites due to overcomplication, and clarifying questions come before implementation rather than after mistakes.

---
## RAGNA Voice

A menu-bar dictation app in Rust and egui: hold a Hotkey, speak, release, and the Transcript is pasted into the Target App. whisper.cpp runs locally. v1 targets macOS on Apple Silicon.

### Start every session here

1. Read `AGENTS.md` (binding rules: assets, privacy, never crash).
2. Read `CONTEXT.md` (domain vocabulary, invariants, non-goals) and the relevant ADRs in `docs/adr/`. Use the vocabulary in code and docs: *Dictation*, not session; *Recording*, not clip.
3. Read `docs/architecture.md` and the crate you're touching.

### Non-negotiables

- **Layering.** `crates/core` depends on no other workspace crate, no OS API and no egui. Only `crates/ui-egui` and `apps/ragna-voice` depend on egui, eframe, winit or tray-icon. OS APIs live only in `crates/platform`. See `docs/architecture.md`.
- **Privacy and never-crash rules** are in `AGENTS.md` §2 and §4.
- **Brand.** UI follows the RAGNA brand (`ragna-website/docs/brand/`: navy first, teal only as a small interactive accent, no gradients, Geist type). Use the tokens and components in `crates/ui-egui/src/theme.rs` and `settings.rs`, not raw colours. The overlay pill keeps its own neutral look.
- **Rust only** in the product and the build (`xtask`). No shell scripts for anything `xtask` can do.
- **Quality gates** before every commit: `cargo xtask ci` (fmt, clippy with `-D warnings`, tests). `deny.toml` configures `cargo deny check` (dependency licences, advisories, sources).
- **Commits:** one logical change per commit, only from green states. Conventional prefixes (`feat:`, `fix:`, `chore:`, `docs:`). If a change alters behaviour described in `CONTEXT.md`, update it in the same commit.

### Running and looking at the app

- `cargo run -p ragna-voice` starts the menu-bar app. It has no Dock icon and no window at launch. The microphone icon in the menu bar opens Settings.
- `cargo xtask run` builds, signs and launches `RAGNA Voice Dev.app` (`io.ragna.voice.dev`, logs: `target/ragna-voice.log`). It has its own identity, so it never touches the installed release's permissions. Both would answer the same Hotkey, so `run` quits the installed release first; reopen it from /Applications afterwards. Use it, not `cargo run`, for anything involving permissions: a bare binary's grants go to the terminal.
- `cargo xtask bundle --release` builds `RAGNA Voice.app` (`io.ragna.voice`), signed with `RAGNA_VOICE_SIGN_IDENTITY` (ad-hoc by default). Notarized releases are built outside this repo.
- **Dev builds are signed ad-hoc on purpose.** macOS ties grants to the signature, and ad-hoc signatures change on every rebuild, so each build starts with no permissions. That keeps the first-run permission flow tested. Don't switch to a stable identity to "fix" it; set `RAGNA_VOICE_SIGN_IDENTITY` only when you need grants to persist.
- `cargo run -p ragna-voice-cli` runs the headless pipeline. Prefer it for checking engine changes.
- For UI work, **look at the result.** `screencapture -x -R 0,0,3000,40 /tmp/menubar.png` captures the menu bar. With Accessibility granted to the terminal, `osascript` and System Events can open the tray menu (`menu bar 2` of process `ragna-voice`) and list windows.
- `RAGNA_VOICE_DEMO_OVERLAY=1 cargo xtask run` keeps the overlay on screen with a fake Recording, so it can be screenshotted without holding the Hotkey. It appears about 96 pt above the bottom of the screen, centred.
- Parallel agents: use a separate `CARGO_TARGET_DIR` per agent and separate git worktrees.

### Testing

Failing tests are an important signal for code quality and should be fixed as soon as possible, even if they are not directly related to the issue at hand.
