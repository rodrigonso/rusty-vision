use anyhow::{Context, Result, bail};
use image::RgbaImage;
use xcap::{Monitor, Window};

/// Read pixel data from an HBITMAP into an RGBA buffer.
#[cfg(windows)]
fn read_hbitmap(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    hbitmap: windows::Win32::Graphics::Gdi::HBITMAP,
    width: u32,
    height: u32,
) -> Result<Vec<u8>> {
    use std::mem;
    use windows::Win32::Graphics::Gdi::{
        GetDIBits, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS,
    };

    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            biHeight: -(height as i32), // top-down
            biPlanes: 1,
            biBitCount: 32,
            biSizeImage: width * height * 4,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut buf = vec![0u8; (width * height * 4) as usize];
    unsafe {
        GetDIBits(
            hdc,
            hbitmap,
            0,
            height,
            Some(buf.as_mut_ptr().cast()),
            &mut bmi,
            DIB_RGB_COLORS,
        );
    }

    // BGRA → RGBA
    for pixel in buf.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }

    Ok(buf)
}

/// Capture a window using PrintWindow with PW_RENDERFULLCONTENT.
/// This captures only the target window (no bleed-through from overlapping windows)
/// and correctly renders modern Windows 11 title bars and chrome.
#[cfg(windows)]
fn capture_window_printwindow(hwnd_raw: u32, width: u32, height: u32) -> Result<RgbaImage> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetWindowDC,
        ReleaseDC, SelectObject,
    };
    use windows::Win32::Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS};

    const PW_RENDERFULLCONTENT: u32 = 2;

    unsafe {
        let hwnd = HWND(hwnd_raw as *mut std::ffi::c_void);
        let hdc_window = GetWindowDC(Some(hwnd));
        let hdc_mem = CreateCompatibleDC(Some(hdc_window));
        let hbitmap = CreateCompatibleBitmap(hdc_window, width as i32, height as i32);
        let old_obj = SelectObject(hdc_mem, hbitmap.into());

        let ok = PrintWindow(
            hwnd,
            hdc_mem,
            PRINT_WINDOW_FLAGS(PW_RENDERFULLCONTENT),
        );

        let buf = read_hbitmap(hdc_mem, hbitmap, width, height)?;

        SelectObject(hdc_mem, old_obj);
        let _ = DeleteObject(hbitmap.into());
        let _ = DeleteDC(hdc_mem);
        ReleaseDC(Some(hwnd), hdc_window);

        if !ok.as_bool() {
            bail!("PrintWindow failed for hwnd {hwnd_raw}");
        }

        RgbaImage::from_raw(width, height, buf)
            .context("Failed to create image from PrintWindow capture")
    }
}

/// Fallback: capture a screen region by BitBlt from the desktop DC.
/// Used when PrintWindow fails. Captures whatever is visible on screen,
/// so overlapping windows may bleed through.
#[cfg(windows)]
fn capture_screen_region(x: i32, y: i32, width: u32, height: u32) -> Result<RgbaImage> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetWindowDC,
        ReleaseDC, SelectObject, SRCCOPY,
    };

    unsafe {
        let hwnd_desktop = HWND::default();
        let hdc_screen = GetWindowDC(Some(hwnd_desktop));
        let hdc_mem = CreateCompatibleDC(Some(hdc_screen));
        let hbitmap = CreateCompatibleBitmap(hdc_screen, width as i32, height as i32);
        let old_obj = SelectObject(hdc_mem, hbitmap.into());

        BitBlt(hdc_mem, 0, 0, width as i32, height as i32, Some(hdc_screen), x, y, SRCCOPY)
            .context("BitBlt failed")?;

        let buf = read_hbitmap(hdc_mem, hbitmap, width, height)?;

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
             Run `rusty-vision list` to see available windows."
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
             Run `rusty-vision list` to see available windows."
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

    let cap_x = window.x().unwrap_or(0);
    let cap_y = window.y().unwrap_or(0);
    let cap_w = window.width().unwrap_or(0);
    let cap_h = window.height().unwrap_or(0);

    let geom = WindowGeometry {
        x: cap_x,
        y: cap_y,
        width: cap_w,
        height: cap_h,
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

    // Primary: PrintWindow with PW_RENDERFULLCONTENT captures only the target window
    // (no bleed-through from overlapping windows) and renders modern Win11 chrome.
    // Fallback: BitBlt from screen if PrintWindow fails.
    #[cfg(windows)]
    let img = {
        match capture_window_printwindow(id, geom.width, geom.height) {
            Ok(img) => img,
            Err(e) => {
                eprintln!("PrintWindow failed ({e:#}), falling back to screen capture");
                capture_screen_region(geom.x, geom.y, geom.width, geom.height)?
            }
        }
    };
    #[cfg(not(windows))]
    let img = window
        .capture_image()
        .context("Failed to capture window screenshot")?;
    Ok((img, pid, id, geom))
}

