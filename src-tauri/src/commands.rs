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
        fn CGDataProviderCreateWithCFData(data: CF) -> CF;
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

    /// Owns one retained CoreFoundation object and releases it on drop.
    struct Cf(CF);

    impl Cf {
        fn new(value: CF, what: &str) -> Result<Cf, String> {
            if value.is_null() {
                Err(format!("{what} failed"))
            } else {
                Ok(Cf(value))
            }
        }
    }

    impl Drop for Cf {
        fn drop(&mut self) {
            assert!(!self.0.is_null());
            unsafe { CFRelease(self.0) }
        }
    }

    unsafe fn color_space(icc: Option<&[u8]>) -> Result<Cf, String> {
        unsafe {
            if let Some(icc) = icc {
                let data = Cf::new(
                    CFDataCreate(kCFAllocatorDefault, icc.as_ptr(), icc.len() as isize),
                    "CFDataCreate (ICC)",
                )?;
                let space = CGColorSpaceCreateWithICCData(data.0);
                if !space.is_null() {
                    return Ok(Cf(space));
                }
            }
            Cf::new(CGColorSpaceCreateDeviceRGB(), "CGColorSpaceCreateDeviceRGB")
        }
    }

    unsafe fn cg_image(image: &RgbImage, icc: Option<&[u8]>) -> Result<Cf, String> {
        unsafe {
            let space = color_space(icc)?;
            let pixels = image.as_raw();
            let data = Cf::new(
                CFDataCreate(kCFAllocatorDefault, pixels.as_ptr(), pixels.len() as isize),
                "CFDataCreate (pixels)",
            )?;
            let provider = Cf::new(
                CGDataProviderCreateWithCFData(data.0),
                "CGDataProviderCreateWithCFData",
            )?;
            Cf::new(
                CGImageCreate(
                    image.width() as usize,
                    image.height() as usize,
                    8,
                    24,
                    image.width() as usize * 3,
                    space.0,
                    0,
                    provider.0,
                    std::ptr::null(),
                    false,
                    0,
                ),
                "CGImageCreate",
            )
        }
    }

    pub fn encode(image: &RgbImage, icc: Option<&[u8]>, quality: f64) -> Result<Vec<u8>, String> {
        unsafe {
            let cg_image = cg_image(image, icc)?;
            let output = Cf::new(
                CFDataCreateMutable(kCFAllocatorDefault, 0),
                "CFDataCreateMutable",
            )?;
            let ty = CString::new("public.avif").unwrap();
            let ty = Cf::new(
                CFStringCreateWithCString(kCFAllocatorDefault, ty.as_ptr(), UTF8),
                "CFStringCreateWithCString",
            )?;
            let destination = Cf::new(
                CGImageDestinationCreateWithData(output.0, ty.0, 1, std::ptr::null()),
                "ImageIO has no AVIF encoder; CGImageDestinationCreateWithData",
            )?;
            let quality = Cf::new(
                CFNumberCreate(
                    kCFAllocatorDefault,
                    FLOAT64,
                    (&quality as *const f64).cast(),
                ),
                "CFNumberCreate",
            )?;
            let keys = [kCGImageDestinationLossyCompressionQuality];
            let values = [quality.0];
            let properties = Cf::new(
                CFDictionaryCreate(
                    kCFAllocatorDefault,
                    keys.as_ptr(),
                    values.as_ptr(),
                    1,
                    (&raw const kCFTypeDictionaryKeyCallBacks).cast(),
                    (&raw const kCFTypeDictionaryValueCallBacks).cast(),
                ),
                "CFDictionaryCreate",
            )?;
            CGImageDestinationAddImage(destination.0, cg_image.0, properties.0);
            if !CGImageDestinationFinalize(destination.0) {
                return Err("ImageIO AVIF encode failed".into());
            }
            Ok(std::slice::from_raw_parts(
                CFDataGetBytePtr(output.0),
                CFDataGetLength(output.0) as usize,
            )
            .to_vec())
        }
    }
}

// Maps the app's JPEG-style dial (0-100) to ImageIO's AVIF quality (0-1). Fitted
// with SSIMULACRA2 against the `image` crate AVIF encoder at speed 9: dial 87
// matches at 0.72, and ImageIO gains ~0.44 points per 0.01 where the `image`
// encoder gains ~1.2 points per dial step.
#[cfg(target_os = "macos")]
fn imageio_avif_quality(dial: u8) -> f64 {
    (0.72 + 2.7 * (f64::from(dial) / 100.0 - 0.87)).clamp(0.0, 1.0)
}

const DETECT_MAX_SIDE: u32 = 1280;
const DETECT_MIN_SCORE: f32 = 0.6;

#[derive(Debug, serde::Serialize)]
pub struct DetectedFace {
    pub x: f32,
    pub y: f32,
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    pub score: f32,
    pub required: bool,
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
    let bytes = fs::read(path).map_err(|e| format!("Image open error: {e}"))?;
    decode_bytes_with_orientation_and_profile(bytes)
}

fn decode_bytes_with_orientation_and_profile(
    bytes: Vec<u8>,
) -> Result<(DynamicImage, Option<Vec<u8>>), String> {
    let mut decoder = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("Image format error: {e}"))?
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

fn decode_rgb(path: &str) -> Result<image::RgbImage, String> {
    let bytes = fs::read(path).map_err(|e| format!("Read error: {e}"))?;
    let src = RawSource::new_from_shared_vec(Arc::new(bytes));
    let bytes = src.buf();
    let is_jxl = path.ends_with(".jxl")
        || (bytes.len() >= 2
            && (bytes[0..2] == [0xFF, 0x0A] || bytes.starts_with(b"\x00\x00\x00\x0CJXL ")));

    if is_jxl {
        let decoder = jxl_oxide::JxlImage::builder()
            .read(bytes)
            .map_err(|e| format!("JXL decode error: {e}"))?;
        let render = decoder
            .render_frame(0)
            .map_err(|e| format!("JXL render error: {e}"))?;
        let fb = render.image_all_channels();
        let (w, h, channels) = (
            fb.width() as u32,
            fb.height() as u32,
            fb.channels() as usize,
        );
        let mut rgb = Vec::with_capacity((w * h * 3) as usize);
        for pixel in 0..(w * h) as usize {
            let base = pixel * channels;
            for channel in 0..3 {
                rgb.push((fb.buf()[base + channel].clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
            }
        }
        return image::RgbImage::from_raw(w, h, rgb)
            .ok_or_else(|| "Failed to create image from JXL data".to_string());
    }

    if may_be_raw(path)
        && let Ok(decoder) = rawler::get_decoder(&src)
    {
        let params = RawDecodeParams::default();
        if let Ok(Some(preview)) = decoder.preview_image(&src, &params) {
            return Ok(preview.to_rgb8());
        }
        return decode_raw(decoder.as_ref(), &src).map(|image| image.to_rgb8());
    }

    if is_heif(path) {
        let tmp = sips_convert_to_tiff(path)?;
        let decoded = decode_image_with_orientation(Path::new(&tmp));
        let _ = fs::remove_file(&tmp);
        return decoded.map(|image| image.to_rgb8());
    }

    decode_image_with_orientation(Path::new(path)).map(|image| image.to_rgb8())
}

fn iou(a: &faces::yunet::Face, b: &faces::yunet::Face) -> f32 {
    let width = (a.x1.min(b.x1) - a.x0.max(b.x0)).max(0.0);
    let height = (a.y1.min(b.y1) - a.y0.max(b.y0)).max(0.0);
    let intersection = width * height;
    let union = (a.x1 - a.x0) * (a.y1 - a.y0) + (b.x1 - b.x0) * (b.y1 - b.y0) - intersection;
    if union > 0.0 {
        intersection / union
    } else {
        0.0
    }
}

fn contains(a: &faces::yunet::Face, b: &faces::yunet::Face) -> bool {
    a.x0 <= b.x0 && a.y0 <= b.y0 && a.x1 >= b.x1 && a.y1 >= b.y1
}

pub fn detect_faces_blocking(path: &str) -> Result<Vec<DetectedFace>, String> {
    detect_faces_in(decode_rgb(path)?)
}

/// For a browser that holds the pixels but no path the server can read: the image it
/// already oriented and downscaled, encoded in a format the `image` crate reads.
pub fn detect_faces_in_bytes(bytes: Vec<u8>) -> Result<Vec<DetectedFace>, String> {
    let (image, _) = decode_bytes_with_orientation_and_profile(bytes)?;
    detect_faces_in(image.to_rgb8())
}

fn detect_faces_in(image: image::RgbImage) -> Result<Vec<DetectedFace>, String> {
    let image = if image.width().max(image.height()) > DETECT_MAX_SIDE {
        image::imageops::thumbnail(&image, DETECT_MAX_SIDE, DETECT_MAX_SIDE)
    } else {
        image
    };
    let options = faces::yunet::Options {
        score: DETECT_MIN_SCORE,
        ..Default::default()
    };
    let mut faces = faces::detector()
        .map_err(|e| e.to_string())?
        .detect(
            image.as_raw(),
            image.width() as usize,
            image.height() as usize,
            &options,
        )
        .map_err(|e| e.to_string())?;
    faces.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut deduped = Vec::new();
    for face in faces {
        if deduped
            .iter()
            .any(|kept| iou(kept, &face) > 0.3 || contains(kept, &face) || contains(&face, kept))
        {
            continue;
        }
        deduped.push(face);
    }
    let largest = deduped
        .iter()
        .map(|face| (face.x1 - face.x0) * (face.y1 - face.y0))
        .fold(0.0_f32, f32::max);
    Ok(deduped
        .into_iter()
        .map(|face| DetectedFace {
            x: ((face.landmarks[0].0 + face.landmarks[1].0) / 2.0).clamp(0.0, 1.0),
            y: ((face.landmarks[0].1 + face.landmarks[1].1) / 2.0).clamp(0.0, 1.0),
            x0: face.x0,
            y0: face.y0,
            x1: face.x1,
            y1: face.y1,
            score: face.score,
            required: face.score >= 0.7
                && (face.x1 - face.x0) * (face.y1 - face.y0) >= largest * 0.25,
        })
        .collect())
}

#[tauri::command]
pub async fn detect_faces(path: String) -> Result<Vec<DetectedFace>, String> {
    tauri::async_runtime::spawn_blocking(move || detect_faces_blocking(&path))
        .await
        .map_err(|e| format!("face detection task: {e}"))?
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
        let candidates = [std::path::PathBuf::from(std::env::var("ENFRAME_SAMPLE_DIR").unwrap_or_default())
            .join("sample_00019.ARW")];
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
        let icc = decoder.rendered_icc();
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
            Some(icc),
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
                buf = imageio_avif::encode(&cropped, icc.as_deref(), imageio_avif_quality(quality))
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
    use super::{
        DETECT_MIN_SCORE, crop_and_export, decode_rgb, detect_faces_blocking,
        detect_faces_in_bytes, may_be_raw, preview_is_full_size,
    };
    use std::{path::PathBuf, time::Instant};

    #[cfg(target_os = "macos")]
    #[test]
    fn imageio_avif_quality_is_anchored_at_the_default_and_stays_in_range() {
        use super::imageio_avif_quality;
        assert!((imageio_avif_quality(87) - 0.72).abs() < 1e-9);
        assert_eq!(imageio_avif_quality(100), 1.0);
        assert_eq!(imageio_avif_quality(0), 0.0);
    }

    fn fixture(name: &str) -> String {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
            .to_string_lossy()
            .into_owned()
    }

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
        assert!(may_be_raw("/photos/sample_00019.ARW"));
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

    #[test]
    fn detects_a_face_in_the_portrait_fixture() {
        let faces =
            detect_faces_blocking(&fixture("portrait-upright-ref.jpg")).expect("detect faces");
        assert!(!faces.is_empty(), "portrait should contain a face");
    }

    #[test]
    fn orientation_six_keeps_the_portrait_detectable() {
        let upright =
            detect_faces_blocking(&fixture("portrait-upright-ref.jpg")).expect("detect upright");
        let oriented =
            detect_faces_blocking(&fixture("synthetic-orient6.jpg")).expect("detect oriented");
        assert!(
            !oriented.is_empty(),
            "orientation-6 portrait should contain a face"
        );
        assert!(
            (upright[0].score - oriented[0].score).abs() < 0.05,
            "orientation should preserve the same face"
        );
    }

    #[test]
    fn detects_the_same_face_from_encoded_bytes() {
        let from_path =
            detect_faces_blocking(&fixture("portrait-upright-ref.jpg")).expect("detect by path");
        let bytes = std::fs::read(fixture("portrait-upright-ref.jpg")).expect("read portrait");
        let from_bytes = detect_faces_in_bytes(bytes).expect("detect from bytes");
        assert_eq!(from_bytes.len(), from_path.len());
        assert!((from_bytes[0].x - from_path[0].x).abs() < 0.01);
        assert!((from_bytes[0].y - from_path[0].y).abs() < 0.01);
    }

    #[test]
    fn rejects_bytes_that_are_not_an_image() {
        assert!(detect_faces_in_bytes(b"not an image".to_vec()).is_err());
    }

    #[test]
    fn no_people_landscape_has_no_faces() {
        let landscape = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../images/20230713-DJI_0392-Enhanced-NR.jxl")
            .to_string_lossy()
            .into_owned();
        assert!(
            detect_faces_blocking(&landscape)
                .expect("detect landscape")
                .is_empty()
        );
    }

    #[test]
    fn yunet_median_on_a_2048px_photo_stays_under_the_bound() {
        let source = decode_rgb(&fixture("portrait-upright-ref.jpg")).expect("decode portrait");
        let image =
            image::imageops::resize(&source, 2048, 2048, image::imageops::FilterType::Triangle);
        let options = faces::yunet::Options {
            score: DETECT_MIN_SCORE,
            ..Default::default()
        };
        let detector = faces::detector().expect("load detector");
        let mut timings = Vec::new();
        for _ in 0..9 {
            let started = Instant::now();
            let found = detector
                .detect(
                    image.as_raw(),
                    image.width() as usize,
                    image.height() as usize,
                    &options,
                )
                .expect("detect face");
            assert!(!found.is_empty(), "portrait should contain a face");
            timings.push(started.elapsed());
        }
        timings.sort();
        let median = timings[timings.len() / 2];
        println!("YuNet 2048px median: {median:?}");
        assert!(median.as_millis() < 500, "median {median:?}");
    }
}
