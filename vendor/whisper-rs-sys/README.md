# Vendored whisper-rs-sys

This is `whisper-rs-sys` 0.15.0 from crates.io, patched to build whisper.cpp v1.9.5.

## Why

whisper-rs 0.16 bundles whisper.cpp 1.8.3 through this crate. Upstream whisper-rs has stalled. whisper.cpp 1.9.3 and later fix memory-safety bugs (log-mel on very short audio, malformed model headers, GGUF loading). The root `Cargo.toml` swaps this crate in with `[patch.crates-io]`. `whisper-rs` itself is unchanged.

## Source

- Crate: `whisper-rs-sys` 0.15.0, copied from crates.io (repository: https://codeberg.org/tazz4843/whisper-rs).
- whisper.cpp: git submodule at `whisper.cpp`, pinned to tag `v1.9.5` (commit `d1be6fde11ac6e0407606b4e42fe72d34add8037`). Run `git submodule update --init` after cloning.

## Local changes

- `build.rs`: read the whisper.cpp version from the `WHISPER_VERSION_{MAJOR,MINOR,PATCH}` variables. The old `project("whisper.cpp" VERSION ...)` line is gone since 1.9.3 and made the build panic. A missing version no longer fails the build.
- `src/bindings.rs`: regenerated against the 1.9.5 headers (with the `metal` feature).
- Not shipped: the crate's own `Cargo.lock` and `.cargo_vcs_info.json`.

## When to delete

Delete this directory, the submodule and the `[patch.crates-io]` entry once whisper-rs ships a release that depends on whisper.cpp 1.9.3 or later.
