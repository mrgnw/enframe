use base64::Engine;
use image::{DynamicImage, ImageDecoder, ImageEncoder, ImageReader};
use rawler::decoders::{Decoder, RawDecodeParams};
use rawler::rawsource::RawSource;
use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

#[cfg(target_os = "macos")]
mod imageio_avif {
    use image::RgbImage;
    use std::ffi::{CString, c_void};

    type CF = *const c_void;
    const UTF8: u32 = 0x0800_0100;
    const FLOAT64: i32 = 13;

    #[link(name = "ImageIO", kind = "framework")]
    unsafe extern "C" {
        static kCGImageDestinationLossyCompressionQuality: CF;
        fn CGImageDestinationCreateWithData(data: CF, ty: CF, count: usize, options: CF) -> CF;
        fn CGImageDestinationAddImage(destination: CF, image: CF, properties: CF);
        fn CGImageDestinationFinalize(destination: CF) -> bool;
    }
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGColorSpaceCreateWithICCData(data: CF) -> CF;
        fn CGColorSpaceCreateDeviceRGB() -> CF;
        fn CGDataProviderCreateWithData(info: CF, data: *const u8, size: usize, release: CF) -> CF;
        fn CGImageCreate(
            width: usize,
            height: usize,
            bpc: usize,
            bpp: usize,
            bpr: usize,
            color_space: CF,
            bitmap_info: u32,
            provider: CF,
            decode: *const f64,
            interpolate: bool,
            intent: i32,
        ) -> CF;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        static kCFAllocatorDefault: CF;
        static kCFTypeDictionaryKeyCallBacks: c_void;
        static kCFTypeDictionaryValueCallBacks: c_void;
        fn CFDataCreate(allocator: CF, bytes: *const u8, length: isize) -> CF;
        fn CFDataCreateMutable(allocator: CF, capacity: isize) -> CF;
        fn CFDataGetBytePtr(data: CF) -> *const u8;
        fn CFDataGetLength(data: CF) -> isize;
        fn CFDictionaryCreate(
            allocator: CF,
            keys: *const CF,
            values: *const CF,
            count: isize,
            key_callbacks: *const c_void,
            value_callbacks: *const c_void,
        ) -> CF;
        fn CFNumberCreate(allocator: CF, number_type: i32, value: *const c_void) -> CF;
        fn CFRelease(value: CF);
        fn CFStringCreateWithCString(allocator: CF, value: *const i8, encoding: u32) -> CF;
    }

    pub fn encode(image: &RgbImage, icc: Option<&[u8]>, quality: f64) -> Result<Vec<u8>, String> {
        unsafe {
            let color_space = if let Some(icc) = icc {
                let data = CFDataCreate(kCFAllocatorDefault, icc.as_ptr(), icc.len() as isize);
                let color_space = CGColorSpaceCreateWithICCData(data);
                CFRelease(data);
                if color_space.is_null() {
                    CGColorSpaceCreateDeviceRGB()
                } else {
                    color_space
                }
            } else {
                CGColorSpaceCreateDeviceRGB()
            };
            let provider = CGDataProviderCreateWithData(
                std::ptr::null(),
                image.as_raw().as_ptr(),
                image.as_raw().len(),
                std::ptr::null(),
            );
            let cg_image = CGImageCreate(
                image.width() as usize,
                image.height() as usize,
                8,
                24,
                image.width() as usize * 3,
                color_space,
                0,
                provider,
                std::ptr::null(),
                false,
                0,
            );
            if cg_image.is_null() {
                return Err("CGImageCreate failed".into());
            }
            let output = CFDataCreateMutable(kCFAllocatorDefault, 0);
            let ty = CString::new("public.avif").unwrap();
            let ty = CFStringCreateWithCString(kCFAllocatorDefault, ty.as_ptr(), UTF8);
            let destination = CGImageDestinationCreateWithData(output, ty, 1, std::ptr::null());
            if destination.is_null() {
                return Err("ImageIO has no AVIF encoder".into());
            }
            let quality = CFNumberCreate(
                kCFAllocatorDefault,
                FLOAT64,
                (&quality as *const f64).cast(),
            );
            let keys = [kCGImageDestinationLossyCompressionQuality];
            let values = [quality];
            let properties = CFDictionaryCreate(
                kCFAllocatorDefault,
                keys.as_ptr(),
                values.as_ptr(),
                1,
                (&raw const kCFTypeDictionaryKeyCallBacks).cast(),
                (&raw const kCFTypeDictionaryValueCallBacks).cast(),
            );
            CGImageDestinationAddImage(destination, cg_image, properties);
            let result = if CGImageDestinationFinalize(destination) {
                Ok(std::slice::from_raw_parts(
                    CFDataGetBytePtr(output),
                    CFDataGetLength(output) as usize,
                )
                .to_vec())
            } else {
                Err("ImageIO AVIF encode failed".into())
            };
            for value in [
                properties,
                quality,
                destination,
                ty,
                output,
                cg_image,
                provider,
                color_space,
            ] {
                CFRelease(value);
            }
            result
        }
    }
}

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

fn decode_image_with_orientation_and_profile(
    path: &Path,
) -> Result<(DynamicImage, Option<Vec<u8>>), String> {
    let mut decoder = ImageReader::open(path)
        .map_err(|e| format!("Image open error: {e}"))?
        .into_decoder()
        .map_err(|e| format!("Image decode error: {e}"))?;
    let profile = decoder
        .icc_profile()
        .map_err(|e| format!("Image profile error: {e}"))?;
    let orientation = decoder
        .orientation()
        .map_err(|e| format!("Image orientation error: {e}"))?;
    let mut image =
        DynamicImage::from_decoder(decoder).map_err(|e| format!("Image decode error: {e}"))?;
    image.apply_orientation(orientation);
    Ok((image, profile))
}

/// Decode an image in the orientation WebKit presents to the editor.
pub fn decode_image_with_orientation(path: &Path) -> Result<DynamicImage, String> {
    decode_image_with_orientation_and_profile(path).map(|(image, _)| image)
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

const RAW_EXTENSIONS: [&str; 9] = [
    "arw", "cr2", "cr3", "nef", "dng", "raf", "orf", "rw2", "pef",
];

// rawler accepts some ordinary JPEGs by content — they carry the TIFF/EXIF structure it
// sniffs for — and then spends minutes demosaicing what is not sensor data. Trust the
// extension when there is one; only extension-less files (Lightroom proxies) get probed.
fn may_be_raw(path: &str) -> bool {
    match std::path::Path::new(path).extension() {
        Some(extension) => {
            RAW_EXTENSIONS.contains(&extension.to_ascii_lowercase().to_string_lossy().as_ref())
        }
        None => true,
    }
}

// Browsers cannot decode camera RAW, so serve mode renders a JPEG rather than shipping
// the original bytes to an <img>. Returns None when the file is not RAW at all.
pub fn raw_as_jpeg(path: &str) -> Result<Option<Vec<u8>>, String> {
    if !may_be_raw(path) {
        return Ok(None);
    }
    let file_bytes = fs::read(path).map_err(|e| format!("Read error: {e}"))?;
    let src = RawSource::new_from_shared_vec(Arc::new(file_bytes));
    let Ok(decoder) = rawler::get_decoder(&src) else {
        return Ok(None);
    };
    let image = decode_raw(decoder.as_ref(), &src)?;
    let mut jpeg = Vec::new();
    image
        .to_rgb8()
        .write_to(&mut Cursor::new(&mut jpeg), image::ImageFormat::Jpeg)
        .map_err(|e| format!("JPEG encode error: {e}"))?;
    Ok(Some(jpeg))
}

// Thumbnails accept any embedded preview: a 1616px Sony preview that is too small for
// the editor is more than a 300px grid tile needs, and it skips the demosaic entirely.
// Full decode only when a RAW carries no preview, downscaled so the IPC payload stays
// bounded. Base64 because Tauri serializes Vec<u8> as a JSON number array.
// Async so the seconds-long decode runs off the main thread; a sync command would
// beachball the window for every tile.
#[tauri::command]
pub async fn raw_thumbnail(path: String) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || raw_thumbnail_blocking(&path))
        .await
        .map_err(|e| format!("thumbnail task: {e}"))?
}

// Public so serve can offer the same thumbnail without a Tauri runtime to spawn on.
pub fn raw_thumbnail_blocking(path: &str) -> Result<Option<String>, String> {
    if !may_be_raw(path) {
        return Ok(None);
    }
    let file_bytes = fs::read(path).map_err(|e| format!("Read error: {e}"))?;
    let src = RawSource::new_from_shared_vec(Arc::new(file_bytes));

    let decoded = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
        || -> Result<Option<image::DynamicImage>, String> {
            let Ok(decoder) = rawler::get_decoder(&src) else {
                // Not RAW after all. Lightroom proxies are extension-less JPEGs the
                // desktop asset protocol serves without a MIME type, so decode them
                // here and hand back a payload the webview will actually render.
                if src.buf().starts_with(&[0xFF, 0xD8]) {
                    return image::load_from_memory(src.buf())
                        .map(Some)
                        .map_err(|e| format!("JPEG decode error: {e}"));
                }
                return Ok(None);
            };
            let params = RawDecodeParams::default();
            if let Ok(Some(preview)) = decoder.preview_image(&src, &params) {
                return Ok(Some(preview));
            }
            decode_raw(decoder.as_ref(), &src).map(Some)
        },
    ));

    let image = match decoded {
        Ok(Ok(Some(image))) => image,
        Ok(Ok(None)) => return Ok(None),
        Ok(Err(e)) => return Err(e),
        // rawler panics outright on absurd dimensions (stitched panoramas); an empty
        // tile beats taking the whole grid down.
        Err(_) => return Ok(None),
    };

    let thumb = image.thumbnail(1024, 1024);
    let mut jpeg = Vec::new();
    thumb
        .to_rgb8()
        .write_to(&mut Cursor::new(&mut jpeg), image::ImageFormat::Jpeg)
        .map_err(|e| format!("JPEG encode error: {e}"))?;
    Ok(Some(base64::engine::general_purpose::STANDARD.encode(jpeg)))
}

#[cfg(test)]
mod raw_thumbnail_tests {
    use super::*;

    #[test]
    fn a_plain_extension_is_not_raw() {
        assert_eq!(raw_thumbnail_blocking("/tmp/x.jpeg"), Ok(None));
    }

    // Needs a real camera file; skipped wherever none exists.
    #[test]
    fn an_arw_yields_a_jpeg_data_payload() {
        let candidates = [std::path::PathBuf::from(std::env::var("HOME").unwrap())
            .join("Pictures/samples/IMG_0001.ARW")];
        let Some(arw) = candidates.iter().find(|p| p.exists()) else {
            eprintln!("no ARW fixture on this machine, skipping");
            return;
        };
        let encoded = raw_thumbnail_blocking(&arw.to_string_lossy())
            .expect("decode should succeed")
            .expect("an ARW always carries an embedded preview");
        let jpeg = base64::engine::general_purpose::STANDARD
            .decode(&encoded)
            .expect("payload is base64");
        assert_eq!(&jpeg[0..2], &[0xFF, 0xD8], "payload is a JPEG");
        assert!(jpeg.len() < 1_500_000, "thumbnail payload stays bounded");
    }
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

// The flat argument list is the invoke contract: export.ts passes these by name and
// serve's dispatcher forwards them positionally, so grouping them into a struct would
// change both callers for no behavioral gain.
#[expect(clippy::too_many_arguments)]
#[tauri::command]
pub fn crop_and_export(
    source_path: String,
    output_path: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    quality: u8,
    speed: u8,
    format: String,
) -> Result<(), String> {
    if !(1..=10).contains(&speed) {
        return Err(format!("AVIF speed must be 1–10, got {speed}"));
    }
    let file_bytes = fs::read(&source_path).map_err(|e| format!("Read error: {e}"))?;
    let src = RawSource::new_from_shared_vec(Arc::new(file_bytes));
    let file_bytes = src.buf();

    let is_jxl = source_path.ends_with(".jxl")
        || (file_bytes.len() >= 2
            && (file_bytes[0..2] == [0xFF, 0x0A]
                || file_bytes.starts_with(b"\x00\x00\x00\x0CJXL ")));

    let (img, icc) = if is_jxl {
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
        (
            image::RgbImage::from_raw(w, h, rgb_buf)
                .ok_or_else(|| "Failed to create image from JXL data".to_string())?,
            None,
        )
    } else if let Ok(decoder) = rawler::get_decoder(&src) {
        (decode_raw(decoder.as_ref(), &src)?.to_rgb8(), None)
    } else if is_heif(&source_path) {
        let tmp = sips_convert_to_tiff(&source_path)?;
        let decoded = decode_image_with_orientation_and_profile(Path::new(&tmp));
        let _ = fs::remove_file(&tmp);
        let (image, icc) = decoded?;
        (image.to_rgb8(), icc)
    } else {
        let (image, icc) = decode_image_with_orientation_and_profile(Path::new(&source_path))?;
        (image.to_rgb8(), icc)
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

    #[cfg(not(target_os = "macos"))]
    let _ = &icc;

    let mut buf: Vec<u8> = Vec::new();
    match format.as_str() {
        "png" => {
            let encoder = image::codecs::png::PngEncoder::new(&mut buf);
            encoder
                .write_image(&cropped, crop_w, crop_h, image::ExtendedColorType::Rgb8)
                .map_err(|e| format!("PNG encode error: {e}"))?;
        }
        "avif" => {
            #[cfg(target_os = "macos")]
            {
                buf = imageio_avif::encode(
                    &cropped,
                    icc.as_deref(),
                    (f64::from(quality) / 100.0 - 0.10).clamp(0.0, 1.0),
                )
                .map_err(|e| format!("ImageIO AVIF encode error: {e}"))?;
            }
            #[cfg(not(target_os = "macos"))]
            {
                let encoder = image::codecs::avif::AvifEncoder::new_with_speed_quality(
                    &mut buf, speed, quality,
                );
                encoder
                    .write_image(&cropped, crop_w, crop_h, image::ExtendedColorType::Rgb8)
                    .map_err(|e| format!("AVIF encode error: {e}"))?;
            }
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
    use super::{crop_and_export, may_be_raw, preview_is_full_size};
    use std::path::PathBuf;

    #[test]
    fn avif_speed_outside_one_to_ten_is_refused_before_any_work() {
        for speed in [0, 11] {
            let result = crop_and_export(
                "/missing.jpg".into(),
                "/missing.avif".into(),
                0.0,
                0.0,
                1.0,
                1.0,
                87,
                speed,
                "avif".into(),
            );
            assert_eq!(result, Err(format!("AVIF speed must be 1–10, got {speed}")));
        }
    }

    #[test]
    fn ordinary_images_never_reach_the_raw_decoder() {
        assert!(!may_be_raw("/photos/a.jpeg"));
        assert!(!may_be_raw("/photos/a.JPG"));
        assert!(!may_be_raw("/photos/a.png"));
        assert!(!may_be_raw("/photos/2022 pt cabo da roca.jpeg"));
    }

    #[test]
    fn raw_extensions_and_bare_names_are_probed() {
        assert!(may_be_raw("/photos/IMG_0001.ARW"));
        assert!(may_be_raw("/photos/shot.cr3"));
        assert!(may_be_raw("/photos/shot.dng"));
        assert!(may_be_raw("/lightroom/proxy-no-extension"));
    }

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

    #[test]
    fn crops_use_exif_orientation_without_rotating_jxl_twice() {
        let cases = [
            ("orientation-1.jpg", (600, 200), [255u8, 0, 0]),
            ("orientation-3.jpg", (600, 200), [255u8, 255, 0]),
            ("orientation-6.jpg", (400, 300), [0u8, 255, 0]),
            ("orientation-8.jpg", (400, 300), [0u8, 0, 255]),
            ("orientation-6.jxl", (400, 300), [0u8, 255, 0]),
        ];

        for (source, dimensions, expected) in cases {
            let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("testdata")
                .join(source);
            let output = std::env::temp_dir().join(format!(
                "enframe-orientation-{}-{}.png",
                source.file_stem().unwrap().to_string_lossy(),
                std::process::id()
            ));
            crop_and_export(
                source.to_string_lossy().into_owned(),
                output.to_string_lossy().into_owned(),
                0.0,
                0.0,
                0.5,
                0.25,
                87,
                9,
                "png".into(),
            )
            .unwrap();

            let crop = image::open(&output).unwrap().to_rgb8();
            let _ = std::fs::remove_file(output);
            assert_eq!(crop.dimensions(), dimensions, "{source:?}");
            let pixel = crop.get_pixel(5, 5).0;
            for (actual, wanted) in pixel.into_iter().zip(expected) {
                assert!(
                    (i16::from(actual) - i16::from(wanted)).abs() < 20,
                    "{source:?}: {pixel:?}"
                );
            }
        }
    }
}
