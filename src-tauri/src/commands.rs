use base64::Engine;
use image::ImageEncoder;
use rawler::decoders::{Decoder, RawDecodeParams};
use rawler::rawsource::RawSource;
use std::fs;
use std::process::Command;
use std::sync::Arc;

// sips survives for HEIC/HEIF only: rawler decodes camera RAW formats and the
// `image` crate has no HEIF codec, so macOS is the only decoder available.
fn is_heif(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.ends_with(".heic") || lower.ends_with(".heif")
}

fn sips_convert_to_tiff(source: &str) -> Result<String, String> {
    let tmp = std::env::temp_dir().join(format!("enframe_sips_{}.tiff", std::process::id()));
    let tmp_str = tmp.to_string_lossy().to_string();
    let output = Command::new("sips")
        .args(["-s", "format", "tiff", source, "--out", &tmp_str])
        .output()
        .map_err(|e| format!("sips launch: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "sips failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(tmp_str)
}

// ponytail: embedded previews carry the in-camera render, so prefer them, but
// Sony ARW previews cap at 1616x1080 and fall through to a full demosaic.
fn preview_is_full_size(preview_width: u32, raw_width: usize) -> bool {
    preview_width as usize * 10 >= raw_width * 9
}

fn decode_raw(decoder: &dyn Decoder, src: &RawSource) -> Result<image::DynamicImage, String> {
    let params = RawDecodeParams::default();
    let dims = decoder
        .raw_image(src, &params, true)
        .map_err(|e| format!("RAW header error: {e}"))?;

    if let Ok(Some(preview)) = decoder.preview_image(src, &params)
        && preview_is_full_size(preview.width(), dims.width)
    {
        return Ok(preview);
    }

    let raw = decoder
        .raw_image(src, &params, false)
        .map_err(|e| format!("RAW decode error: {e}"))?;
    rawler::imgop::develop::RawDevelop::default()
        .develop_intermediate(&raw)
        .map_err(|e| format!("RAW develop error: {e}"))?
        .to_dynamic_image()
        .ok_or_else(|| "RAW develop produced no image".to_string())
}

#[tauri::command]
pub fn get_screenshot_mode() -> Option<i64> {
    std::env::var("SCREENSHOT_PHOTO_ID")
        .ok()
        .and_then(|v| v.parse().ok())
}

#[tauri::command]
pub fn save_image(path: String, data: String) -> Result<(), String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&data)
        .map_err(|e| e.to_string())?;
    fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn crop_and_export(
    source_path: String,
    output_path: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    quality: u8,
    format: String,
) -> Result<(), String> {
    let file_bytes = fs::read(&source_path).map_err(|e| format!("Read error: {e}"))?;
    let src = RawSource::new_from_shared_vec(Arc::new(file_bytes));
    let file_bytes = src.buf();

    let is_jxl = source_path.ends_with(".jxl")
        || (file_bytes.len() >= 2
            && (file_bytes[0..2] == [0xFF, 0x0A]
                || file_bytes.starts_with(b"\x00\x00\x00\x0CJXL ")));

    let img = if is_jxl {
        let decoder = jxl_oxide::JxlImage::builder()
            .read(file_bytes)
            .map_err(|e| format!("JXL decode error: {e}"))?;
        let render = decoder
            .render_frame(0)
            .map_err(|e| format!("JXL render error: {e}"))?;
        let fb = render.image_all_channels();
        let w = fb.width() as u32;
        let h = fb.height() as u32;
        let samples = fb.buf();
        let channels = fb.channels() as usize;

        let mut rgb_buf = Vec::with_capacity((w * h * 3) as usize);
        for pixel in 0..(w * h) as usize {
            let base = pixel * channels;
            let r = (samples[base].clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            let g = (samples[base + 1].clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            let b = (samples[base + 2].clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            rgb_buf.push(r);
            rgb_buf.push(g);
            rgb_buf.push(b);
        }
        image::RgbImage::from_raw(w, h, rgb_buf)
            .ok_or_else(|| "Failed to create image from JXL data".to_string())?
    } else if let Ok(decoder) = rawler::get_decoder(&src) {
        decode_raw(decoder.as_ref(), &src)?.to_rgb8()
    } else if is_heif(&source_path) {
        let tmp = sips_convert_to_tiff(&source_path)?;
        let decoded = image::open(&tmp).map_err(|e| format!("Image open error: {e}"));
        let _ = fs::remove_file(&tmp);
        decoded?.to_rgb8()
    } else {
        image::open(&source_path)
            .map_err(|e| format!("Image open error: {e}"))?
            .to_rgb8()
    };

    let img_w = img.width();
    let img_h = img.height();

    let crop_x = (x * img_w as f64).round() as u32;
    let crop_y = (y * img_h as f64).round() as u32;
    let crop_w = (width * img_w as f64).round() as u32;
    let crop_h = (height * img_h as f64).round() as u32;

    let crop_x = crop_x.min(img_w.saturating_sub(1));
    let crop_y = crop_y.min(img_h.saturating_sub(1));
    let crop_w = crop_w.min(img_w - crop_x);
    let crop_h = crop_h.min(img_h - crop_y);

    let cropped = image::imageops::crop_imm(&img, crop_x, crop_y, crop_w, crop_h).to_image();

    let mut buf: Vec<u8> = Vec::new();
    match format.as_str() {
        "png" => {
            let encoder = image::codecs::png::PngEncoder::new(&mut buf);
            encoder
                .write_image(&cropped, crop_w, crop_h, image::ExtendedColorType::Rgb8)
                .map_err(|e| format!("PNG encode error: {e}"))?;
        }
        "avif" => {
            let encoder =
                image::codecs::avif::AvifEncoder::new_with_speed_quality(&mut buf, 6, quality);
            encoder
                .write_image(&cropped, crop_w, crop_h, image::ExtendedColorType::Rgb8)
                .map_err(|e| format!("AVIF encode error: {e}"))?;
        }
        _ => {
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality);
            encoder
                .write_image(&cropped, crop_w, crop_h, image::ExtendedColorType::Rgb8)
                .map_err(|e| format!("JPEG encode error: {e}"))?;
        }
    }

    fs::write(&output_path, &buf).map_err(|e| format!("Write error: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::preview_is_full_size;

    #[test]
    fn sony_arw_preview_falls_through_to_develop() {
        assert!(!preview_is_full_size(1616, 6024));
    }

    #[test]
    fn full_size_preview_is_used() {
        assert!(preview_is_full_size(6024, 6024));
        assert!(preview_is_full_size(5422, 6024));
        assert!(!preview_is_full_size(5421, 6024));
    }
}
