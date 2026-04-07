mod capture;
mod list;
mod output;
#[cfg(windows)]
mod annotate;
#[cfg(windows)]
mod tree;

use anyhow::Result;
use clap::{Parser, Subcommand};
use image::RgbaImage;

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
    List,

    /// Capture a screenshot of a window or the full screen
    Capture {
        /// Window title to capture (partial, case-insensitive match)
        window: Option<String>,

        /// Capture the full screen instead of a specific window
        #[arg(long, conflicts_with = "window")]
        screen: bool,

        /// Capture a window by process ID
        #[arg(long, conflicts_with_all = ["screen", "window"])]
        pid: Option<u32>,

        /// Which monitor to capture (0-indexed, default: 0). Only used with --screen
        #[arg(long, default_value = "0")]
        monitor: usize,

        /// Save screenshot to a file instead of outputting base64 to stdout
        #[arg(long, short)]
        output: Option<String>,

        /// Output raw PNG bytes to stdout (for piping)
        #[arg(long, conflicts_with = "output")]
        raw: bool,

        /// Maximum image width in pixels (downscaled preserving aspect ratio, 0 to disable)
        #[arg(long, default_value = "1920")]
        max_width: u32,

        /// Include the UI element tree in the output (Windows only)
        #[arg(long, conflicts_with = "screen")]
        tree: bool,

        /// Maximum depth for UI tree traversal (default: unlimited)
        #[arg(long, requires = "tree")]
        depth: Option<usize>,
    },
}

/// Downscale an image if it exceeds the max width, preserving aspect ratio.
fn downscale(img: RgbaImage, max_width: Option<u32>) -> RgbaImage {
    let Some(max_w) = max_width else {
        return img;
    };
    if img.width() <= max_w {
        return img;
    }
    let scale = max_w as f64 / img.width() as f64;
    let new_h = (img.height() as f64 * scale).round() as u32;
    eprintln!("Downscaling {}x{} → {}x{}", img.width(), img.height(), max_w, new_h);
    image::imageops::resize(&img, max_w, new_h, image::imageops::FilterType::Lanczos3)
}

fn main() -> Result<()> {
    // Ensure all Windows APIs (GetWindowRect, UIA, etc.) return physical pixel
    // coordinates, matching xcap's DWM-based coordinate system.
    #[cfg(windows)]
    {
        use windows::Win32::UI::HiDpi::{
            SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        };
        unsafe {
            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
    }

    let cli = Cli::parse();

    match cli.command {
        Commands::List => list::list_windows(),
        Commands::Capture {
            window,
            screen,
            pid,
            monitor,
            output,
            raw,
            max_width,
            tree,
            depth,
        } => {
            let mw = if max_width > 0 { Some(max_width) } else { None };

            if screen {
                let img = capture::capture_full_screen(monitor)?;
                let img = downscale(img, mw);
                output::emit(img, output, raw, None::<serde_json::Value>, None)
            } else if let Some(title) = window {
                let (img, captured_pid, _, geom) = capture::capture_by_title(&title)?;
                let (tree_data, annotated) =
                    maybe_inspect_tree(tree, &img, captured_pid, depth, &geom)?;
                let img = downscale(img, mw);
                let annotated = annotated.map(|a| downscale(a, mw));
                output::emit(img, output, raw, tree_data, annotated)
            } else if let Some(pid) = pid {
                let (img, window_pid, _, geom) = capture::capture_by_pid(pid)?;
                let (tree_data, annotated) =
                    maybe_inspect_tree(tree, &img, window_pid, depth, &geom)?;
                let img = downscale(img, mw);
                let annotated = annotated.map(|a| downscale(a, mw));
                output::emit(img, output, raw, tree_data, annotated)
            } else {
                anyhow::bail!(
                    "Specify a window title, --pid <id>, or --screen.\n\
                     Run `rusty-vision list` to see available windows."
                );
            }
        }
    }
}

#[cfg(windows)]
fn maybe_inspect_tree(
    enabled: bool,
    img: &image::RgbaImage,
    pid: u32,
    max_depth: Option<usize>,
    geom: &capture::WindowGeometry,
) -> Result<(Option<tree::TreeNode>, Option<image::RgbaImage>)> {
    if !enabled {
        return Ok((None, None));
    }
    eprintln!("Inspecting UI tree for pid {pid}...");
    let mut node = tree::inspect_tree(pid, max_depth)?;
    tree::assign_ids(&mut node);
    let annotated = annotate::annotate(img, &node, geom);
    Ok((Some(node), Some(annotated)))
}

#[cfg(not(windows))]
fn maybe_inspect_tree(
    enabled: bool,
    _img: &image::RgbaImage,
    _pid: u32,
    _max_depth: Option<usize>,
    _geom: &capture::WindowGeometry,
) -> Result<(Option<serde_json::Value>, Option<image::RgbaImage>)> {
    if enabled {
        anyhow::bail!("--tree is only supported on Windows");
    }
    Ok((None, None))
}

