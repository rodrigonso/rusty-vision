# Copilot Instructions for rusty-vision

## Overview

rusty-vision is a Rust CLI tool that captures screenshots of windows and screens, outputting them in formats suitable for AI agent consumption (base64 JSON, raw PNG, or file). It uses the `xcap` crate for cross-platform screen/window capture.

## Build & Run

```sh
cargo build
cargo run -- list-windows
cargo run -- capture --full-screen
cargo run -- capture --window "Firefox"
cargo run -- capture --pid 1234 --output screenshot.png
```

There are no tests yet. When adding tests, use `cargo test` and `cargo test <test_name>` to run a single test.

## Architecture

The CLI uses **clap derive** for argument parsing with two subcommands: `list-windows` and `capture`. The code is split into three modules by responsibility:

- **`capture`** — Screen/window capture logic via `xcap`. Three entry points: `capture_full_screen`, `capture_by_title`, `capture_by_pid`. All return `RgbaImage`.
- **`output`** — Encodes the captured `RgbaImage` to PNG and handles the three output modes: JSON with base64 (default), file save, or raw bytes to stdout.
- **`list`** — Enumerates visible windows and prints JSON metadata to stdout.

Data flows linearly: **CLI parsing → capture → output**. The capture module finds and screenshots the target; the output module encodes and emits the result.

## Conventions

- **Stdout is for machine-readable output only** — always structured JSON or raw PNG bytes. Never print human-readable messages to stdout.
- **Stderr is for diagnostics** — status messages, warnings, and progress info go to `eprintln!`.
- **Error handling** — use `anyhow::Result` with `.context()` / `.with_context()` for all fallible operations. Use `bail!` for early-return errors with user-facing messages.
- **Rust edition 2024** — this project uses the latest Rust edition. Be aware of edition-specific changes (e.g., `gen` is a reserved keyword, lifetime capture rules in opaque types).
- **Window filtering** — when matching windows by title, use case-insensitive partial matching. Skip windows with empty titles (system/invisible windows).
