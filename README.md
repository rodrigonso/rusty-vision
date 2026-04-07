# rusty-vision

A CLI tool that captures screenshots of windows and screens, outputting them in formats designed for AI agent consumption — base64-encoded JSON, raw PNG bytes, or saved files.

AI coding agents typically operate in a text-only world. **rusty-vision** bridges the gap by giving them a simple command they can shell out to in order to "see" what's on screen — useful for UI testing, visual verification, and any workflow where an agent needs to inspect a graphical interface.

## Features

- **List windows** — enumerate all visible windows with title, PID, position, and size as JSON.
- **Capture by window title** — positional argument with partial, case-insensitive match.
- **Capture by PID** — target a specific process.
- **Full-screen capture** — capture an entire monitor by index.
- **Auto-downscale** — images wider than `--max-width` (default 1920) are downscaled preserving aspect ratio.
- **Flexible output** — base64 JSON to stdout (default), raw PNG to stdout for piping, or save to a file.

## Prerequisites

- [Rust](https://rustup.rs/) (edition 2024 — requires Rust 1.85+)
- Platform-specific dependencies for screen capture (provided by the [`xcap`](https://github.com/aspect-build/xcap) crate):
  - **Windows** — no extra dependencies.
  - **macOS** — screen recording permission must be granted to the terminal.
  - **Linux** — requires X11 libraries (`libxcb`, `libxrandr`, etc.). On Wayland, XWayland must be available.

## Build

```sh
# Debug build
cargo build

# Release build (optimized)
cargo build --release
```

The binary is output to `target/debug/rusty-vision` or `target/release/rusty-vision`.

## Usage

```sh
# List all visible windows as JSON
rusty-vision list

# Capture a window by title (partial match)
rusty-vision capture "Firefox"

# Capture a window by PID
rusty-vision capture --pid 1234

# Capture the full screen (primary monitor)
rusty-vision capture --screen

# Capture a secondary monitor (0-indexed)
rusty-vision capture --screen --monitor 1

# Save to a file instead of printing base64
rusty-vision capture "Code" --output screenshot.png

# Disable auto-downscaling (capture at native resolution)
rusty-vision capture "Code" --max-width 0

# Pipe raw PNG bytes to another tool
rusty-vision capture --screen --raw | feh -
```

### Output format

By default, `capture` prints a JSON object to stdout:

```json
{
  "width": 1920,
  "height": 1080,
  "format": "png",
  "image_base64": "iVBORw0KGgo..."
}
```

When using `--output <file>`, the image is saved and stdout contains metadata only:

```json
{
  "width": 1920,
  "height": 1080,
  "format": "png",
  "file": "screenshot.png"
}
```

Status messages and warnings are printed to **stderr**, so stdout is always clean for machine parsing.

## Debug

```sh
# Run directly with arguments via cargo
cargo run -- list-windows
cargo run -- capture --window "Firefox"

# Enable backtrace on errors
RUST_BACKTRACE=1 cargo run -- capture --full-screen

# Full backtrace for deeply nested errors
RUST_BACKTRACE=full cargo run -- capture --pid 9999

# Run under a debugger (example with lldb on macOS/Linux)
cargo build
lldb -- target/debug/rusty-vision capture --full-screen

# On Windows with Visual Studio debugger
cargo build
devenv /debugexe target\debug\rusty-vision.exe capture --full-screen
```

## Deploy

### Standalone binary

Build a release binary and distribute it — no runtime dependencies needed on Windows/macOS:

```sh
cargo build --release

# The binary is self-contained
cp target/release/rusty-vision /usr/local/bin/   # Linux/macOS
# or copy target\release\rusty-vision.exe wherever needed on Windows
```

### Install from source

```sh
cargo install --path .
```

This installs the binary to `~/.cargo/bin/`, which should be on your `PATH`.

### Cross-compilation

To build for a different target platform:

```sh
rustup target add x86_64-unknown-linux-gnu
cargo build --release --target x86_64-unknown-linux-gnu
```

## License

This project is not yet licensed. Add a `LICENSE` file to specify terms.
