#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod lightroom;

use base64::Engine;
use image::ImageEncoder;
use std::fs;

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
) -> Result<(), String> {
	let file_bytes = fs::read(&source_path).map_err(|e| format!("Read error: {e}"))?;

	let is_jxl = source_path.ends_with(".jxl")
		|| (file_bytes.len() >= 2 && (file_bytes[0..2] == [0xFF, 0x0A] || file_bytes.starts_with(b"\x00\x00\x00\x0CJXL ")));

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
		let dyn_img = image::open(&source_path).map_err(|e| format!("Image open error: {e}"))?;
		dyn_img.to_rgb8()
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

	let mut jpeg_buf: Vec<u8> = Vec::new();
	let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_buf, quality);
	encoder
		.write_image(&cropped, crop_w, crop_h, image::ExtendedColorType::Rgb8)
		.map_err(|e| format!("JPEG encode error: {e}"))?;

	fs::write(&output_path, &jpeg_buf).map_err(|e| format!("Write error: {e}"))?;
	Ok(())
}

fn main() {
	tauri::Builder::default()
		.plugin(
			tauri_plugin_sql::Builder::new()
				.add_migrations(
					"sqlite:enframe.db",
					vec![tauri_plugin_sql::Migration {
						version: 1,
						description: "create tables",
						sql: include_str!("../../database.sql"),
						kind: tauri_plugin_sql::MigrationKind::Up,
					}],
				)
				.build(),
		)
		.plugin(tauri_plugin_fs::init())
		.plugin(tauri_plugin_dialog::init())
		.invoke_handler(tauri::generate_handler![
			lightroom::get_lightroom_library_path,
			lightroom::list_lightroom_photos,
			save_image,
			crop_and_export
		])
		.run(tauri::generate_context!())
		.expect("error while running tauri application")
}
