use std::io::Write;

use anyhow::{Context, Result};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use image::RgbaImage;
use serde::Serialize;

#[derive(Serialize)]
struct ScreenshotOutput<T: Serialize> {
    width: u32,
    height: u32,
    format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    annotated_image_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    annotated_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tree: Option<T>,
}

fn encode_png(img: &RgbaImage) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut buf);
    image::ImageEncoder::write_image(
        encoder,
        img.as_raw(),
        img.width(),
        img.height(),
        image::ExtendedColorType::Rgba8,
    )
    .context("Failed to encode PNG")?;
    Ok(buf)
}

pub fn emit(
    img: RgbaImage,
    output_path: Option<String>,
    raw: bool,
    tree: Option<impl Serialize>,
    annotated_img: Option<RgbaImage>,
) -> Result<()> {
    let png_bytes = encode_png(&img)?;
    let annotated_png = annotated_img
        .as_ref()
        .map(encode_png)
        .transpose()?;

    if raw {
        let mut stdout = std::io::stdout().lock();
        stdout
            .write_all(&png_bytes)
            .context("Failed to write PNG to stdout")?;
        return Ok(());
    }

    if let Some(path) = output_path {
        std::fs::write(&path, &png_bytes)
            .with_context(|| format!("Failed to write screenshot to {path}"))?;

        // Save annotated image alongside the original
        let annotated_path = if let Some(bytes) = &annotated_png {
            let apath = annotated_file_path(&path);
            std::fs::write(&apath, bytes)
                .with_context(|| format!("Failed to write annotated screenshot to {apath}"))?;
            Some(apath)
        } else {
            None
        };

        let output = ScreenshotOutput {
            width: img.width(),
            height: img.height(),
            format: "png".into(),
            image_base64: None,
            annotated_image_base64: None,
            file: Some(path),
            annotated_file: annotated_path,
            tree,
        };
        println!("{}", serde_json::to_string_pretty(&output)?);
        eprintln!("Screenshot saved.");
    } else {
        let b64 = BASE64.encode(&png_bytes);
        let annotated_b64 = annotated_png.map(|bytes| BASE64.encode(&bytes));
        let output = ScreenshotOutput {
            width: img.width(),
            height: img.height(),
            format: "png".into(),
            image_base64: Some(b64),
            annotated_image_base64: annotated_b64,
            file: None,
            annotated_file: None,
            tree,
        };
        println!("{}", serde_json::to_string(&output)?);
    }

    Ok(())
}

/// Derive annotated file path from the original (e.g. "shot.png" → "shot_annotated.png")
fn annotated_file_path(original: &str) -> String {
    if let Some(dot) = original.rfind('.') {
        format!("{}_annotated{}", &original[..dot], &original[dot..])
    } else {
        format!("{original}_annotated")
    }
}
