use ab_glyph::{FontRef, PxScale};
use image::{Rgba, RgbaImage};
use imageproc::drawing::{draw_filled_rect_mut, draw_hollow_rect_mut, draw_text_mut};
use imageproc::rect::Rect;

use crate::capture::WindowGeometry;
use crate::tree::TreeNode;

const FONT_BYTES: &[u8] = include_bytes!(r"C:\Windows\Fonts\consolab.ttf");

const COLORS: &[Rgba<u8>] = &[
    Rgba([255, 0, 0, 220]),
    Rgba([0, 180, 0, 220]),
    Rgba([0, 100, 255, 220]),
    Rgba([255, 165, 0, 220]),
    Rgba([180, 0, 180, 220]),
    Rgba([0, 180, 180, 220]),
    Rgba([255, 80, 80, 220]),
    Rgba([80, 200, 80, 220]),
];

const LABEL_BG: Rgba<u8> = Rgba([0, 0, 0, 200]);
const LABEL_FG: Rgba<u8> = Rgba([255, 255, 255, 255]);

/// Draw bounding boxes and numbered labels on a copy of the screenshot.
/// Uses the UIA window rect and xcap geometry to compute the correct coordinate
/// mapping, accounting for DPI scaling and title bar cropping.
pub fn annotate(
    img: &RgbaImage,
    tree: &TreeNode,
    geom: &WindowGeometry,
) -> RgbaImage {
    let mut annotated = img.clone();
    let font = FontRef::try_from_slice(FONT_BYTES).expect("Failed to load embedded font");
    let nodes = crate::tree::collect_annotatable_nodes(tree);

    // Use the actual system DPI scale for UIA logical → physical conversion
    let dpi = geom.dpi_scale;
    // Image pixels per physical pixel (should be ~1.0 since we capture at physical resolution)
    let img_scale_x = img.width() as f64 / geom.width.max(1) as f64;
    let img_scale_y = img.height() as f64 / geom.height.max(1) as f64;

    // Font size scales with image DPI
    let font_size = (18.0 * dpi * img_scale_x).max(14.0) as f32;
    let label_scale = PxScale { x: font_size, y: font_size };
    let char_w = (font_size * 0.62) as u32;
    let label_h = font_size as u32 + 4;

    for (id, rect, _role, _name) in &nodes {
        let color = COLORS[*id as usize % COLORS.len()];

        // UIA logical → physical screen → image pixels
        let phys_x = rect.x as f64 * dpi;
        let phys_y = rect.y as f64 * dpi;
        let px = ((phys_x - geom.x as f64) * img_scale_x) as i32;
        let py = ((phys_y - geom.y as f64) * img_scale_y) as i32;
        let pw = (rect.width as f64 * dpi * img_scale_x) as u32;
        let ph = (rect.height as f64 * dpi * img_scale_y) as u32;

        if pw == 0 || ph == 0 {
            continue;
        }

        // Clamp to image bounds
        let x = px.max(0) as u32;
        let y = py.max(0) as u32;
        let w = pw.min(annotated.width().saturating_sub(x));
        let h = ph.min(annotated.height().saturating_sub(y));
        if w == 0 || h == 0 {
            continue;
        }

        let draw_rect = Rect::at(x as i32, y as i32).of_size(w, h);
        draw_hollow_rect_mut(&mut annotated, draw_rect, color);
        // Draw a second outline for visibility
        if w > 2 && h > 2 {
            let inner = Rect::at(x as i32 + 1, y as i32 + 1).of_size(w - 2, h - 2);
            draw_hollow_rect_mut(&mut annotated, inner, color);
        }

        // Draw label background + number at top-left of bounding box
        let label = id.to_string();
        let label_w = label.len() as u32 * char_w + 6;
        let lx = x.min(annotated.width().saturating_sub(label_w)) as i32;
        let ly = (y as i32).saturating_sub(label_h as i32).max(0);

        let bg_rect = Rect::at(lx, ly).of_size(label_w, label_h);
        draw_filled_rect_mut(&mut annotated, bg_rect, LABEL_BG);
        draw_text_mut(
            &mut annotated,
            LABEL_FG,
            lx + 3,
            ly + 2,
            label_scale,
            &font,
            &label,
        );
    }

    annotated
}
