mod capture;
mod list;
mod output;

use anyhow::Result;
use clap::{Parser, Subcommand};

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

        /// Which monitor to capture (0-indexed, default: 0). Only used with --full-screen
        #[arg(long, default_value = "0")]
        monitor: usize,

        /// Save screenshot to a file instead of outputting base64 to stdout
        #[arg(long, short)]
        output: Option<String>,

        /// Output raw PNG bytes to stdout (for piping)
        #[arg(long, conflicts_with = "output")]
        raw: bool,
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
            monitor,
            output,
            raw,
        } => {
            if full_screen {
                let img = capture::capture_full_screen(monitor)?;
                output::emit(img, output, raw)
            } else if let Some(title) = window {
                let img = capture::capture_by_title(&title)?;
                output::emit(img, output, raw)
            } else if let Some(pid) = pid {
                let img = capture::capture_by_pid(pid)?;
                output::emit(img, output, raw)
            } else {
                anyhow::bail!(
                    "Specify --full-screen, --window <title>, or --pid <id>.\n\
                     Run `rusty-vision list-windows` to see available windows."
                );
            }
        }
    }
}
