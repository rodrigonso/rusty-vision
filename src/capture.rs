use anyhow::{Context, Result, bail};
use image::RgbaImage;
use std::time::{Duration, Instant};
use xcap::{Monitor, Window};

pub fn capture_full_screen(monitor_index: usize) -> Result<RgbaImage> {
    let monitors = Monitor::all().context("Failed to enumerate monitors")?;

    let monitor = monitors
        .into_iter()
        .nth(monitor_index)
        .with_context(|| format!("Monitor index {monitor_index} not found. Use a lower index."))?;

    eprintln!(
        "Capturing monitor: {} ({}x{})",
        monitor.name().unwrap_or_else(|_| "unknown".into()),
        monitor.width().unwrap_or(0),
        monitor.height().unwrap_or(0)
    );

    monitor
        .capture_image()
        .context("Failed to capture monitor screenshot")
}

pub fn capture_by_title(title_query: &str) -> Result<(RgbaImage, u32, u32)> {
    let windows = Window::all().context("Failed to enumerate windows")?;
    let query_lower = title_query.to_lowercase();

    let matches: Vec<Window> = windows
        .into_iter()
        .filter(|w| {
            w.title()
                .map(|t| !t.is_empty() && t.to_lowercase().contains(&query_lower))
                .unwrap_or(false)
        })
        .collect();

    if matches.is_empty() {
        bail!(
            "No window found matching \"{title_query}\".\n\
             Run `rusty-vision list-windows` to see available windows."
        );
    }

    if matches.len() > 1 {
        eprintln!(
            "Warning: {} windows match \"{}\". Capturing the first match:",
            matches.len(),
            title_query
        );
        for w in &matches {
            eprintln!("  - \"{}\" (pid: {})", w.title().unwrap_or_default(), w.pid().unwrap_or(0));
        }
    }

    let window = &matches[0];

    if window.is_minimized().unwrap_or(false) {
        bail!(
            "Window \"{}\" is minimized and cannot be captured.\n\
             Restore the window first, then try again.",
            window.title().unwrap_or_default()
        );
    }

    capture_window(window)
}

pub fn capture_by_pid(pid: u32) -> Result<(RgbaImage, u32, u32)> {
    let windows = Window::all().context("Failed to enumerate windows")?;

    let matches: Vec<Window> = windows
        .into_iter()
        .filter(|w| w.pid().unwrap_or(0) == pid && w.title().map(|t| !t.is_empty()).unwrap_or(false))
        .collect();

    if matches.is_empty() {
        bail!(
            "No window found for PID {pid}.\n\
             Run `rusty-vision list-windows` to see available windows."
        );
    }

    let window = &matches[0];

    if window.is_minimized().unwrap_or(false) {
        bail!(
            "Window \"{}\" (pid: {}) is minimized and cannot be captured.\n\
             Restore the window first, then try again.",
            window.title().unwrap_or_default(),
            pid
        );
    }

    capture_window(window)
}

/// Log and capture a screenshot from a window. Returns (image, pid, window_id).
fn capture_window(window: &Window) -> Result<(RgbaImage, u32, u32)> {
    let pid = window.pid().unwrap_or(0);
    let id = window.id().unwrap_or(0);
    eprintln!(
        "Capturing window: \"{}\" (pid: {}, {}x{})",
        window.title().unwrap_or_default(),
        pid,
        window.width().unwrap_or(0),
        window.height().unwrap_or(0)
    );
    let img = window
        .capture_image()
        .context("Failed to capture window screenshot")?;
    Ok((img, pid, id))
}

const WINDOW_WAIT_TIMEOUT: Duration = Duration::from_secs(10);
const WINDOW_POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Snapshot current visible window IDs (handles).
pub fn snapshot_windows() -> std::collections::HashSet<u32> {
    Window::all()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|w| w.id().ok())
        .collect()
}

/// Wait for a new window that wasn't in `before`, then capture it.
/// Returns (image, pid, window_id) so the caller can close the specific window.
pub fn wait_and_capture_new_window(
    spawned_pid: u32,
    before: &std::collections::HashSet<u32>,
) -> Result<(RgbaImage, u32, u32)> {
    eprintln!("Waiting for new window to appear...");
    let start = Instant::now();

    // Brief delay to let the app initialize and skip transient popups
    std::thread::sleep(Duration::from_secs(1));

    loop {
        let windows = Window::all().unwrap_or_default();

        let candidates: Vec<Window> = windows
            .into_iter()
            .filter(|w| {
                let title = w.title().unwrap_or_default();
                let id = w.id().unwrap_or(0);
                !title.is_empty()
                    && !w.is_minimized().unwrap_or(true)
                    && w.width().unwrap_or(0) > 100
                    && w.height().unwrap_or(0) > 100
                    && (w.pid().unwrap_or(0) == spawned_pid || !before.contains(&id))
            })
            .collect();

        // Prefer a window from the spawned PID, otherwise take the first new one
        let best = candidates
            .iter()
            .find(|w| w.pid().unwrap_or(0) == spawned_pid)
            .or(candidates.first());

        if let Some(window) = best {
            return capture_window(window);
        }

        if start.elapsed() > WINDOW_WAIT_TIMEOUT {
            bail!(
                "Timed out waiting for application window after {}s.",
                WINDOW_WAIT_TIMEOUT.as_secs()
            );
        }

        std::thread::sleep(WINDOW_POLL_INTERVAL);
    }
}
