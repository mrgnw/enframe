//! `imageio.rs` off macOS: ImageIO is not there, so `decode::format` refuses HEIC and JPEG XL before any
//! of these is reached. They keep the same signatures so `decode.rs` dispatches the same way everywhere.

use crate::colour::Profile;
use crate::pixels::Image;
use std::path::Path;

pub struct ExifDateTime {
	pub date: String,
	pub subsec: String,
}

const UNREACHABLE: &str = "decode::format refuses HEIC and JPEG XL off macOS";

pub fn decoder_version() -> String {
	unreachable!("{UNREACHABLE}")
}

pub fn oriented_size(_path: &Path) -> Result<(u32, u32), String> {
	unreachable!("{UNREACHABLE}")
}

pub fn exif_date_time_and_focal_35mm(_path: &Path) -> Result<(Option<ExifDateTime>, Option<f64>), String> {
	unreachable!("{UNREACHABLE}")
}

pub fn profile(_path: &Path) -> Result<Profile, String> {
	unreachable!("{UNREACHABLE}")
}

pub fn decode(_path: &Path) -> Result<Image, String> {
	unreachable!("{UNREACHABLE}")
}
