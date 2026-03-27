mod capture;
mod list;
mod output;
#[cfg(windows)]
mod tree;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::process::{Child, Command, Stdio};

#[derive(Parser)]
#[command(name = "rusty-vision")]
#[command(about = "Capture screenshots of windows/screens for AI agent consumption")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List all capturable windows with their metadata
    ListWindows,

    /// Capture a screenshot of a window or the full screen
    Capture {
        /// Capture the full screen instead of a specific window
        #[arg(long)]
        full_screen: bool,

        /// Capture a window by title (partial, case-insensitive match)
        #[arg(long, conflicts_with = "full_screen")]
        window: Option<String>,

        /// Capture a window by process ID
        #[arg(long, conflicts_with_all = ["full_screen", "window"])]
        pid: Option<u32>,

        /// Launch an application, capture its window, then close it
        #[arg(long, conflicts_with_all = ["full_screen", "window", "pid"])]
        launch: Option<String>,

        /// Which monitor to capture (0-indexed, default: 0). Only used with --full-screen
        #[arg(long, default_value = "0")]
        monitor: usize,

        /// Save screenshot to a file instead of outputting base64 to stdout
        #[arg(long, short)]
        output: Option<String>,

        /// Output raw PNG bytes to stdout (for piping)
        #[arg(long, conflicts_with = "output")]
        raw: bool,

        /// Include the UI element tree in the output (Windows only)
        #[arg(long, conflicts_with = "full_screen")]
        tree: bool,

        /// Maximum depth for UI tree traversal (default: unlimited). Only used with --tree
        #[arg(long, requires = "tree")]
        tree_depth: Option<usize>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::ListWindows => list::list_windows(),
        Commands::Capture {
            full_screen,
            window,
            pid,
            launch,
            monitor,
            output,
            raw,
            tree,
            tree_depth,
        } => {
            if full_screen {
                let img = capture::capture_full_screen(monitor)?;
                output::emit(img, output, raw, None::<serde_json::Value>)
            } else if let Some(exe_path) = launch {
                let before = capture::snapshot_windows();
                let mut child = launch_app(&exe_path)?;
                let child_pid = child.id();
                let mut window_handle: Option<u32> = None;
                let result = (|| {
                    let (img, window_pid, window_id) =
                        capture::wait_and_capture_new_window(child_pid, &before)?;
                    window_handle = Some(window_id);
                    let tree_data = maybe_inspect_tree(tree, window_pid, tree_depth)?;
                    output::emit(img, output, raw, tree_data)
                })();
                // Close the specific window we opened
                if let Some(hwnd) = window_handle {
                    close_window(hwnd);
                }
                // Kill the spawned process if still alive
                if child.try_wait().ok().flatten().is_none() {
                    let _ = child.kill();
                    let _ = child.wait();
                }
                result
            } else if let Some(title) = window {
                let (img, captured_pid, _) = capture::capture_by_title(&title)?;
                let tree_data = maybe_inspect_tree(tree, captured_pid, tree_depth)?;
                output::emit(img, output, raw, tree_data)
            } else if let Some(pid) = pid {
                let (img, window_pid, _) = capture::capture_by_pid(pid)?;
                let tree_data = maybe_inspect_tree(tree, window_pid, tree_depth)?;
                output::emit(img, output, raw, tree_data)
            } else {
                anyhow::bail!(
                    "Specify --full-screen, --window <title>, --pid <id>, or --launch <exe>.\n\
                     Run `rusty-vision list-windows` to see available windows."
                );
            }
        }
    }
}

#[cfg(windows)]
fn maybe_inspect_tree(
    enabled: bool,
    pid: u32,
    max_depth: Option<usize>,
) -> Result<Option<tree::TreeNode>> {
    if !enabled {
        return Ok(None);
    }
    eprintln!("Inspecting UI tree for pid {pid}...");
    let node = tree::inspect_tree(pid, max_depth)?;
    Ok(Some(node))
}

#[cfg(not(windows))]
fn maybe_inspect_tree(
    enabled: bool,
    _pid: u32,
    _max_depth: Option<usize>,
) -> Result<Option<serde_json::Value>> {
    if enabled {
        anyhow::bail!("--tree is only supported on Windows");
    }
    Ok(None)
}

fn launch_app(exe_path: &str) -> Result<Child> {
    eprintln!("Launching {exe_path}...");
    Command::new(exe_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("Failed to launch \"{exe_path}\""))
}

/// Send WM_CLOSE to a specific window handle to close just that window.
#[cfg(windows)]
fn close_window(hwnd: u32) {
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};

    eprintln!("Closing launched window (hwnd: {hwnd})...");
    let hwnd = HWND(hwnd as *mut std::ffi::c_void);
    unsafe {
        let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
    }
}

#[cfg(not(windows))]
fn close_window(_hwnd: u32) {
    // No-op on non-Windows; the child process kill handles cleanup
}

