mod lightroom;

use base64::Engine;
use image::ImageEncoder;
use std::fs;
use std::process::Command;

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

fn needs_sips(path: &str, file_bytes: &[u8]) -> bool {
    let lower = path.to_lowercase();
    if lower.ends_with(".dng")
        || lower.ends_with(".arw")
        || lower.ends_with(".cr2")
        || lower.ends_with(".cr3")
        || lower.ends_with(".nef")
        || lower.ends_with(".heic")
        || lower.ends_with(".heif")
    {
        return true;
    }
    // DNG/TIFF magic with lossy JPEG compression (LR CC proxies have no extension)
    if file_bytes.len() >= 4 {
        let is_tiff = &file_bytes[0..4] == b"II\x2a\x00" || &file_bytes[0..4] == b"MM\x00\x2a";
        if is_tiff && image::open(path).is_err() {
            return true;
        }
    }
    false
}

#[tauri::command]
fn save_image(path: String, data: String) -> Result<(), String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&data)
        .map_err(|e| e.to_string())?;
    fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn crop_and_export(
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

    let is_jxl = source_path.ends_with(".jxl")
        || (file_bytes.len() >= 2
            && (file_bytes[0..2] == [0xFF, 0x0A]
                || file_bytes.starts_with(b"\x00\x00\x00\x0CJXL ")));

    let sips_tmp: Option<String> = if !is_jxl && needs_sips(&source_path, &file_bytes) {
        Some(sips_convert_to_tiff(&source_path)?)
    } else {
        None
    };

    let img = if is_jxl {
        let decoder = jxl_oxide::JxlImage::builder()
            .read(&file_bytes[..])
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
    } else {
        let open_path = sips_tmp.as_deref().unwrap_or(&source_path);
        let dyn_img = image::open(open_path).map_err(|e| format!("Image open error: {e}"))?;
        dyn_img.to_rgb8()
    };

    // Clean up temp file
    if let Some(ref tmp) = sips_tmp {
        let _ = fs::remove_file(tmp);
    }

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_sql::Builder::new()
                .add_migrations(
                    "sqlite:enframe.db",
                    vec![
                        tauri_plugin_sql::Migration {
                            version: 1,
                            description: "create tables",
                            sql: include_str!("../../database.sql"),
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                        tauri_plugin_sql::Migration {
                            version: 2,
                            description: "add rotation to frames",
                            sql: "ALTER TABLE frames ADD COLUMN rotation REAL NOT NULL DEFAULT 0;",
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                    ],
                )
                .build(),
        )
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            lightroom::get_lightroom_library_path,
            lightroom::list_lr_albums,
            lightroom::list_lr_album_assets,
            lightroom::search_lr_assets,
            lightroom::list_lr_recent,
            lightroom::get_lr_asset_paths,
            save_image,
            crop_and_export
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application")
}
