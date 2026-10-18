# RAGNA Voice

A macOS menu-bar app that turns held-key voice into text pasted at the cursor. Hold a hotkey, speak, release — the transcript lands in whatever app has focus.

## Language

**Hotkey**:
The user-configurable key chord that triggers a Dictation while held. Default `⌃⌥Space`. Always a modifier-chord — never a bare non-modifier key.
_Avoid_: shortcut, trigger key, PTT key

**Dictation**:
One full lifecycle from Hotkey-down to Transcript inserted into the Target App. Exactly one Recording, one Transcript, one Insertion per Dictation.
_Avoid_: session, capture, take

**Recording**:
The audio buffer captured between Hotkey-down and Hotkey-up within a single Dictation. Lives in RAM, not on disk.
_Avoid_: clip, audio, sample

**Transcript**:
The text produced by the Model from a Recording. The unit of output.
_Avoid_: result, output, text

**Model**:
A whisper.cpp `.bin` model file (optionally with a Core ML companion). One Model is active at a time. Default bundled: quantised `ggml-base.en`.
_Avoid_: engine, AI, transcriber

**Target App**:
The frontmost application at the moment of Hotkey-up — the one that receives the pasted Transcript.
_Avoid_: focused window, foreground app, host

**Paused**:
A user-toggled global state that disables the Hotkey entirely. While Paused, the Hotkey does not start a Dictation. Off by default; resets on app restart. Purpose: avoid accidental Recordings during sensitive moments (meetings, screen sharing, lending the laptop).
_Avoid_: muted, disabled, off

**Insertion**:
The act of putting a Transcript into the Target App, via clipboard write + synthesised `⌘V`. The Transcript stays on the clipboard afterwards, replacing what was there: restoring the old contents made Chrome drop pastes.
_Avoid_: paste (overloaded with the user's own paste), injection

**Silence Guard**:
A pre-Model VAD check on a Recording. If energy/speech-frame ratio is below threshold, the Dictation is Dropped without invoking the Model — protects against Whisper hallucinations on near-silent audio (e.g. "Thank you.", "Subscribe!").

**Dictation outcome**:
The terminal state of a Dictation. Exactly one of:
- **Inserted** — Transcript pasted into Target App
- **Notified** — Insertion failed (e.g. Accessibility denied, Target App rejects `⌘V`), Transcript left on clipboard with user-facing notification
- **Cancelled** — user pressed `Esc` while holding the Hotkey, or the mic device disappeared mid-Recording
- **Dropped** — Recording shorter than 200 ms, or rejected by Silence Guard, or the Model heard no words (empty Transcript); no notification
- **Capped** — Recording hit the 5-minute hard cap; still transcribed, still Inserted/Notified
- **Failed** — Model inference threw or timed out; user-facing notification reads "Transcription failed", no Transcript produced

## Limits

- **Min Recording duration**: 200 ms — anything shorter is Dropped silently
- **Soft cap**: 60 s — visual warning state, Recording continues
- **Hard cap**: 5 min — Recording auto-stops, Dictation outcome is Capped

## Properties

These are deliberate invariants of the system, not implementation details. Each one is a product commitment.

- **Local-only transcription.** A Recording never leaves the user's Mac. The Model runs in-process via whisper.cpp (statically linked through `whisper-rs`).
- **Recordings live in RAM only.** No audio buffer is ever persisted to disk.
- **No Transcript history.** The app holds a Transcript only between Hotkey-up and Insertion. After that it keeps no copy; the Transcript stays on the system clipboard until the user copies something else. Adding history would be a meaningful product change, not a tweak.
- **No telemetry, no analytics.** The app reports nothing about usage to anyone, ever.
- **One network call total in v1**: explicit Model download from HuggingFace, user-initiated. Nothing else — no auto-update check, no telemetry. Listed verbatim in the Settings privacy statement. Sparkle-style auto-updates are a deliberate v2 concern; v1 ships without them.

## Relationships

- A **Dictation** has exactly one **Recording**, one **Transcript** (unless Cancelled or Dropped), one **Insertion** (unless Notified, Cancelled, or Dropped), and exactly one **Dictation outcome**
- The **Hotkey** opens and closes a **Dictation**; `Esc` while held Cancels it
- The **Model** transforms a **Recording** into a **Transcript**, but only after the **Silence Guard** passes
- The **Target App** is resolved at Hotkey-up time, not Hotkey-down — whichever app is frontmost when the user finishes speaking receives the **Insertion**

## Interface language

The interface (Settings, menu bar, overlay, notifications, the microphone prompt) is available in English and German. The default follows macOS's preferred languages; Settings can pin it. German uses "Sie". The transcription language is a separate setting.

## Non-goals (v1)

Deliberate exclusions. Each of these is a feature someone will reasonably ask for; the answer is "not in v1, revisit later."

- **Streaming transcription during the hold** — Model runs once, batch, on Hotkey-up.
- **Custom vocabulary / Whisper initial prompt** — defer to v2.
- **Voice commands** ("new line", "comma", "delete that") — different product.
- **Persistent Transcript history / journaling** — see Properties; v1 forgets every Transcript.
- **Multi-language auto-detect per Dictation** — pinned-language only; `auto` is a setting, not per-Dictation.
- **iCloud sync** of settings or downloaded Models.
- **Auto-updates** — v1 ships with manual DMG re-download; Sparkle deferred (see ADR-0001).

## Flagged ambiguities

_None yet._
