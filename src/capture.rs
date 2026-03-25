use anyhow::{Context, Result, bail};
use image::RgbaImage;
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

pub fn capture_by_title(title_query: &str) -> Result<RgbaImage> {
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

    eprintln!(
        "Capturing window: \"{}\" (pid: {}, {}x{})",
        window.title().unwrap_or_default(),
        window.pid().unwrap_or(0),
        window.width().unwrap_or(0),
        window.height().unwrap_or(0)
    );

    window
        .capture_image()
        .context("Failed to capture window screenshot")
}

pub fn capture_by_pid(pid: u32) -> Result<RgbaImage> {
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

    eprintln!(
        "Capturing window: \"{}\" (pid: {}, {}x{})",
        window.title().unwrap_or_default(),
        window.pid().unwrap_or(0),
        window.width().unwrap_or(0),
        window.height().unwrap_or(0)
    );

    window
        .capture_image()
        .context("Failed to capture window screenshot")
}
