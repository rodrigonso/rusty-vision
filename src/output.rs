use std::io::Write;

use anyhow::{Context, Result};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use image::RgbaImage;
use serde::Serialize;

#[derive(Serialize)]
struct ScreenshotOutput {
    width: u32,
    height: u32,
    format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
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

pub fn emit(img: RgbaImage, output_path: Option<String>, raw: bool) -> Result<()> {
    let png_bytes = encode_png(&img)?;

    if raw {
        // Write raw PNG bytes to stdout
        let mut stdout = std::io::stdout().lock();
        stdout
            .write_all(&png_bytes)
            .context("Failed to write PNG to stdout")?;
        return Ok(());
    }

    if let Some(path) = output_path {
        // Save to file, print metadata JSON
        std::fs::write(&path, &png_bytes)
            .with_context(|| format!("Failed to write screenshot to {path}"))?;

        let output = ScreenshotOutput {
            width: img.width(),
            height: img.height(),
            format: "png".into(),
            image_base64: None,
            file: Some(path),
        };
        println!("{}", serde_json::to_string_pretty(&output)?);
        eprintln!("Screenshot saved.");
    } else {
        // Default: base64-encoded JSON to stdout
        let b64 = BASE64.encode(&png_bytes);
        let output = ScreenshotOutput {
            width: img.width(),
            height: img.height(),
            format: "png".into(),
            image_base64: Some(b64),
            file: None,
        };
        println!("{}", serde_json::to_string(&output)?);
    }

    Ok(())
}
