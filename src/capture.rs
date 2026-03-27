use anyhow::{Context, Result, bail};
use image::RgbaImage;
use std::time::{Duration, Instant};
use xcap::{Monitor, Window};

/// Capture a screen region by BitBlt from the desktop DC.
/// This captures everything visible at the given screen coordinates,
/// including window title bars and chrome that PrintWindow may miss.
#[cfg(windows)]
fn capture_screen_region(x: i32, y: i32, width: u32, height: u32) -> Result<RgbaImage> {
    use std::mem;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits,
        GetWindowDC, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS,
        SRCCOPY,
    };

    unsafe {
        let hwnd_desktop = HWND::default();
        let hdc_screen = GetWindowDC(Some(hwnd_desktop));
        let hdc_mem = CreateCompatibleDC(Some(hdc_screen));
        let hbitmap = CreateCompatibleBitmap(hdc_screen, width as i32, height as i32);
        let old_obj = SelectObject(hdc_mem, hbitmap.into());

        BitBlt(hdc_mem, 0, 0, width as i32, height as i32, Some(hdc_screen), x, y, SRCCOPY)
            .context("BitBlt failed")?;

        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biSizeImage: width * height * 4,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut buf = vec![0u8; (width * height * 4) as usize];
        GetDIBits(
            hdc_mem,
            hbitmap,
            0,
            height,
            Some(buf.as_mut_ptr().cast()),
            &mut bmi,
            DIB_RGB_COLORS,
        );

        // BGRA → RGBA
        for pixel in buf.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }

        SelectObject(hdc_mem, old_obj);
        let _ = DeleteObject(hbitmap.into());
        let _ = DeleteDC(hdc_mem);
        ReleaseDC(Some(hwnd_desktop), hdc_screen);

        RgbaImage::from_raw(width, height, buf)
            .context("Failed to create image from screen capture")
    }
}

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

pub fn capture_by_title(title_query: &str) -> Result<(RgbaImage, u32, u32, WindowGeometry)> {
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

pub fn capture_by_pid(pid: u32) -> Result<(RgbaImage, u32, u32, WindowGeometry)> {
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

/// The geometry of the captured window as reported by xcap.
pub struct WindowGeometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub dpi_scale: f64,
}

/// Log and capture a screenshot from a window. Returns (image, pid, window_id, geometry).
fn capture_window(window: &Window) -> Result<(RgbaImage, u32, u32, WindowGeometry)> {
    let pid = window.pid().unwrap_or(0);
    let id = window.id().unwrap_or(0);

    #[cfg(windows)]
    let dpi_scale = {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::HiDpi::GetDpiForWindow;
        let hwnd = HWND(id as *mut std::ffi::c_void);
        let dpi = unsafe { GetDpiForWindow(hwnd) };
        if dpi > 0 { dpi as f64 / 96.0 } else { 1.0 }
    };
    #[cfg(not(windows))]
    let dpi_scale = 1.0;

    let geom = WindowGeometry {
        x: window.x().unwrap_or(0),
        y: window.y().unwrap_or(0),
        width: window.width().unwrap_or(0),
        height: window.height().unwrap_or(0),
        dpi_scale,
    };
    eprintln!(
        "Capturing window: \"{}\" (pid: {}, {}x{}, dpi: {:.0}%)",
        window.title().unwrap_or_default(),
        pid,
        geom.width,
        geom.height,
        dpi_scale * 100.0
    );
    // Use our own screen-region capture to include the full window with title bar.
    // xcap's PrintWindow-based capture often crops the title bar on Windows 11.
    #[cfg(windows)]
    let img = capture_screen_region(geom.x, geom.y, geom.width, geom.height)?;
    #[cfg(not(windows))]
    let img = window
        .capture_image()
        .context("Failed to capture window screenshot")?;
    Ok((img, pid, id, geom))
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
) -> Result<(RgbaImage, u32, u32, WindowGeometry)> {
    eprintln!("Waiting for new window to appear...");
    let start = Instant::now();

    // Brief delay to let the app initialize and skip transient popups
    std::thread::sleep(Duration::from_secs(2));

    loop {
        let windows = Window::all().unwrap_or_default();

        let candidates: Vec<Window> = windows
            .into_iter()
            .filter(|w| {
                let title = w.title().unwrap_or_default();
                let id = w.id().unwrap_or(0);
                !title.is_empty()
                    && !title.contains("PopupHost")
                    && !w.is_minimized().unwrap_or(true)
                    && w.width().unwrap_or(0) > 200
                    && w.height().unwrap_or(0) > 200
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
