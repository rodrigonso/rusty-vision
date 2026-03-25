use anyhow::{Context, Result};
use serde::Serialize;
use xcap::Window;

#[derive(Serialize)]
struct WindowInfo {
    title: String,
    pid: u32,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    minimized: bool,
}

pub fn list_windows() -> Result<()> {
    let windows = Window::all().context("Failed to enumerate windows")?;

    let infos: Vec<WindowInfo> = windows
        .into_iter()
        .filter(|w| {
            // Filter out windows with empty titles (invisible/system windows)
            w.title().map(|t| !t.is_empty()).unwrap_or(false)
        })
        .map(|w| {
            WindowInfo {
                title: w.title().unwrap_or_default(),
                pid: w.pid().unwrap_or(0),
                x: w.x().unwrap_or(0),
                y: w.y().unwrap_or(0),
                width: w.width().unwrap_or(0),
                height: w.height().unwrap_or(0),
                minimized: w.is_minimized().unwrap_or(false),
            }
        })
        .collect();

    let json = serde_json::to_string_pretty(&infos)?;
    println!("{json}");
    Ok(())
}
