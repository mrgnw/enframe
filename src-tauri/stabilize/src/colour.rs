//! Photo colour profiles. Stabilize keeps pixel values and labels the clip with the photos' profile; only
//! a set mixing sRGB and Display P3 photos is converted, sRGB to Display P3, which holds every sRGB colour.

use crate::pixels::Image;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Profile {
	Srgb,
	DisplayP3,
}

/// H.273 transfer characteristics 13: the sRGB curve, which both profiles share.
pub const SRGB_TRANSFER: u16 = 13;
const UNSPECIFIED: u16 = 2;

impl Profile {
	/// H.273 colour primaries: 1 is BT.709 (sRGB's), 12 is SMPTE EG 432 (Display P3).
	pub fn primaries(self) -> u16 {
		match self {
			Profile::Srgb => 1,
			Profile::DisplayP3 => 12,
		}
	}

	/// The same primaries as ffmpeg names them.
	pub fn ffmpeg_primaries(self) -> &'static str {
		match self {
			Profile::Srgb => "bt709",
			Profile::DisplayP3 => "smpte432",
		}
	}
}

/// The profile the clip gets: the photos' shared one, or Display P3 for a mix.
pub fn clip_profile(profiles: &[Profile]) -> Profile {
	assert!(!profiles.is_empty());
	if profiles.iter().all(|&p| p == profiles[0]) { profiles[0] } else { Profile::DisplayP3 }
}

/// From H.273 code points (AVIF nclx). Unspecified primaries and transfer mean sRGB, as browsers read them.
pub fn from_cicp(primaries: u16, transfer: u16) -> Result<Profile, String> {
	let profile = match primaries {
		1 | UNSPECIFIED => Profile::Srgb,
		12 => Profile::DisplayP3,
		_ => return Err(unsupported(&format!("colour primaries {primaries}"))),
	};
	if transfer != SRGB_TRANSFER && transfer != UNSPECIFIED {
		return Err(unsupported(&format!("transfer characteristics {transfer}")));
	}
	Ok(profile)
}

fn unsupported(what: &str) -> String {
	format!("{what}: not sRGB or Display P3; convert the photo to one of them first")
}

/// Colorants (rXYZ, gXYZ, bXYZ) as ICC profiles store them, adapted to D50.
const SRGB_COLORANTS: [[f64; 3]; 3] =
	[[0.4361, 0.2225, 0.0139], [0.3851, 0.7169, 0.0971], [0.1431, 0.0606, 0.7141]];
const P3_COLORANTS: [[f64; 3]; 3] =
	[[0.5151, 0.2412, -0.0011], [0.2919, 0.6922, 0.0419], [0.1572, 0.0666, 0.7841]];
/// Profiles round colorants to 16 fractional bits and differ in the fourth decimal; P3 and sRGB differ by 0.08.
const COLORANT_TOLERANCE: f64 = 0.003;
/// Largest difference from the sRGB curve, in linear light, over the 256 8-bit codes.
const CURVE_TOLERANCE: f64 = 0.002;

/// From an ICC profile, by its colorants and tone curves; the description text is not trusted.
pub fn from_icc(icc: &[u8]) -> Result<Profile, String> {
	if icc.len() < 132 || &icc[36..40] != b"acsp" {
		return Err("the ICC profile is malformed".into());
	}
	if &icc[16..20] != b"RGB " {
		return Err(unsupported(&format!(
			"ICC profile colour space {}",
			String::from_utf8_lossy(&icc[16..20])
		)));
	}
	let colorants = [b"rXYZ", b"gXYZ", b"bXYZ"].map(|sig| tag(icc, sig).and_then(xyz));
	let curves = [b"rTRC", b"gTRC", b"bTRC"].map(|sig| tag(icc, sig).and_then(curve_error));
	let near = |reference: &[[f64; 3]; 3]| {
		colorants.iter().zip(reference).all(|(c, r)| {
			c.as_ref().is_ok_and(|c| c.iter().zip(r).all(|(a, b)| (a - b).abs() <= COLORANT_TOLERANCE))
		})
	};
	let profile = if near(&SRGB_COLORANTS) {
		Profile::Srgb
	} else if near(&P3_COLORANTS) {
		Profile::DisplayP3
	} else {
		return Err(unsupported("ICC profile primaries"));
	};
	if !curves.iter().all(|c| c.as_ref().is_ok_and(|&e| e <= CURVE_TOLERANCE)) {
		return Err(unsupported("ICC profile tone curve"));
	}
	Ok(profile)
}

fn be32(data: &[u8], at: usize) -> Option<u32> {
	Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn s15f16(data: &[u8], at: usize) -> Option<f64> {
	Some(be32(data, at)? as i32 as f64 / 65536.0)
}

/// A tag's data from the tag table.
fn tag<'a>(icc: &'a [u8], signature: &[u8; 4]) -> Result<&'a [u8], String> {
	let count = be32(icc, 128).ok_or("the ICC profile is malformed")?.min(1024) as usize;
	for i in 0..count {
		let entry = 132 + 12 * i;
		let Some(sig) = icc.get(entry..entry + 4) else { break };
		if sig == signature {
			let (offset, size) = (be32(icc, entry + 4), be32(icc, entry + 8));
			let data = offset.zip(size).and_then(|(o, s)| icc.get(o as usize..o as usize + s as usize));
			return data.ok_or_else(|| "the ICC profile is malformed".into());
		}
	}
	Err(format!("the ICC profile has no {} tag", String::from_utf8_lossy(signature)))
}

fn xyz(data: &[u8]) -> Result<[f64; 3], String> {
	let value = |i: usize| s15f16(data, 8 + 4 * i);
	match (data.get(..4), value(0), value(1), value(2)) {
		(Some(b"XYZ "), Some(x), Some(y), Some(z)) => Ok([x, y, z]),
		_ => Err("the ICC profile has a malformed colorant".into()),
	}
}

pub fn srgb_to_linear(v: f64) -> f64 {
	if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

fn linear_to_srgb(l: f64) -> f64 {
	if l <= 0.0031308 { 12.92 * l } else { 1.055 * l.powf(1.0 / 2.4) - 0.055 }
}

/// How far an ICC tone curve (`curv` table or `para` function) lies from the sRGB curve.
fn curve_error(data: &[u8]) -> Result<f64, String> {
	let malformed = || "the ICC profile has a malformed tone curve".to_string();
	let eval: Box<dyn Fn(f64) -> f64> = match data.get(..4) {
		Some(b"curv") => {
			let n = be32(data, 8).ok_or_else(malformed)? as usize;
			// Zero entries is the identity and one is a pure gamma: neither is the sRGB curve.
			if n < 2 || data.len() < 12 + 2 * n {
				return Ok(f64::INFINITY);
			}
			let entry = |i: usize| u16::from_be_bytes([data[12 + 2 * i], data[13 + 2 * i]]) as f64 / 65535.0;
			Box::new(move |x| {
				let at = x * (n - 1) as f64;
				let i = (at.floor() as usize).min(n - 2);
				entry(i) + (entry(i + 1) - entry(i)) * (at - i as f64)
			})
		}
		Some(b"para") => {
			let kind = u16::from_be_bytes(data.get(8..10).ok_or_else(malformed)?.try_into().unwrap());
			let count = [1, 3, 4, 5, 7].get(kind as usize).ok_or_else(malformed)?;
			let p: Vec<f64> =
				(0..*count).map(|i| s15f16(data, 12 + 4 * i)).collect::<Option<_>>().ok_or_else(malformed)?;
			let g = p[0];
			let power = move |x: f64, a: f64, b: f64| (a * x + b).max(0.0).powf(g);
			Box::new(move |x| match kind {
				0 => x.powf(p[0]),
				1 => {
					if x >= -p[2] / p[1] {
						power(x, p[1], p[2])
					} else {
						0.0
					}
				}
				2 => {
					if x >= -p[2] / p[1] {
						power(x, p[1], p[2]) + p[3]
					} else {
						p[3]
					}
				}
				3 => {
					if x >= p[4] {
						power(x, p[1], p[2])
					} else {
						p[3] * x
					}
				}
				_ => {
					if x >= p[4] {
						power(x, p[1], p[2]) + p[5]
					} else {
						p[3] * x + p[6]
					}
				}
			})
		}
		_ => return Err(malformed()),
	};
	Ok((0..=255).map(|i| i as f64 / 255.0).map(|x| (eval(x) - srgb_to_linear(x)).abs()).fold(0.0, f64::max))
}

/// Display P3 as an ICC profile, for labelling the `--frames` PNGs: Apple's own on macOS, and elsewhere a
/// minimal v4 matrix/TRC profile with the same colorants and the sRGB parametric curve.
pub fn display_p3_icc() -> Vec<u8> {
	#[cfg(target_os = "macos")]
	{
		crate::imageio::display_p3_icc()
	}
	#[cfg(not(target_os = "macos"))]
	{
		minimal_p3_icc()
	}
}

/// ICC v4 header and tag table (ICC.1:2010), then the tags: wtpt, rXYZ, gXYZ, bXYZ, rTRC, gTRC, bTRC,
/// desc, cprt. The curve is `para` type 3 with sRGB's parameters, so `from_icc` reads it back as Display P3.
#[cfg_attr(target_os = "macos", allow(dead_code))]
fn minimal_p3_icc() -> Vec<u8> {
	fn s15f16(v: f64) -> [u8; 4] {
		((v * 65536.0).round() as i32).to_be_bytes()
	}
	fn xyz(v: [f64; 3]) -> Vec<u8> {
		let mut tag = b"XYZ \0\0\0\0".to_vec();
		tag.extend(v.iter().flat_map(|&c| s15f16(c)));
		tag
	}
	fn mluc(text: &str) -> Vec<u8> {
		let utf16: Vec<u8> = text.encode_utf16().flat_map(u16::to_be_bytes).collect();
		let mut tag = b"mluc\0\0\0\0".to_vec();
		tag.extend(1u32.to_be_bytes()); // one record
		tag.extend(12u32.to_be_bytes()); // record size
		tag.extend(b"enUS");
		tag.extend((utf16.len() as u32).to_be_bytes());
		tag.extend(28u32.to_be_bytes()); // the string follows the record
		tag.extend(utf16);
		tag
	}
	// sRGB's curve: y = (a x + b)^g above d, c x below.
	let mut curve = b"para\0\0\0\0".to_vec();
	curve.extend(3u16.to_be_bytes());
	curve.extend([0, 0]);
	curve.extend([2.4, 1.0 / 1.055, 0.055 / 1.055, 1.0 / 12.92, 0.04045].iter().flat_map(|&p| s15f16(p)));
	let tags: [(&[u8; 4], Vec<u8>); 9] = [
		(b"wtpt", xyz([0.9642, 1.0, 0.8249])),
		(b"rXYZ", xyz(P3_COLORANTS[0])),
		(b"gXYZ", xyz(P3_COLORANTS[1])),
		(b"bXYZ", xyz(P3_COLORANTS[2])),
		(b"rTRC", curve.clone()),
		(b"gTRC", curve.clone()),
		(b"bTRC", curve),
		(b"desc", mluc("Display P3")),
		(b"cprt", mluc("Public domain")),
	];
	let mut icc = vec![0u8; 128];
	icc[8..12].copy_from_slice(&0x0430_0000u32.to_be_bytes()); // version 4.3
	icc[12..16].copy_from_slice(b"mntr");
	icc[16..20].copy_from_slice(b"RGB ");
	icc[20..24].copy_from_slice(b"XYZ ");
	icc[36..40].copy_from_slice(b"acsp");
	icc[68..80].copy_from_slice(&xyz([0.9642, 1.0, 0.8249])[8..]); // the PCS illuminant, D50
	icc.extend((tags.len() as u32).to_be_bytes());
	let mut offset = 128 + 4 + 12 * tags.len();
	let mut data = Vec::new();
	for (signature, tag) in &tags {
		icc.extend(*signature);
		icc.extend((offset as u32).to_be_bytes());
		icc.extend((tag.len() as u32).to_be_bytes());
		data.extend(tag);
		let padded = tag.len().next_multiple_of(4);
		data.resize(data.len() + padded - tag.len(), 0);
		offset += padded;
	}
	icc.extend(data);
	let size = icc.len() as u32;
	icc[..4].copy_from_slice(&size.to_be_bytes());
	icc
}

/// Linear sRGB to linear Display P3: both have the D65 white, so no adaptation. Rows sum to 1, so greys stay.
const SRGB_TO_P3: [[f64; 3]; 3] = [
	[0.822_461_97, 0.177_538_03, 0.0],
	[0.033_194_20, 0.966_805_80, 0.0],
	[0.017_082_63, 0.072_397_44, 0.910_519_93],
];

/// Re-encodes sRGB pixels as Display P3 values of the same colours, rounded to 8 bits.
pub fn srgb_to_p3(image: &mut Image) {
	assert!(image.channels == 3 || image.channels == 4, "colour conversion needs RGB(A)");
	let linear: Vec<f64> = (0..=255).map(|v| srgb_to_linear(v as f64 / 255.0)).collect();
	for px in image.pixels.chunks_exact_mut(image.channels) {
		let rgb = [linear[px[0] as usize], linear[px[1] as usize], linear[px[2] as usize]];
		for (out, row) in px.iter_mut().zip(SRGB_TO_P3) {
			let l = row[0] * rgb[0] + row[1] * rgb[1] + row[2] * rgb[2];
			*out = (linear_to_srgb(l.clamp(0.0, 1.0)) * 255.0).round() as u8;
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn srgb_to_p3_known_values() {
		// Expected values computed in float64 from the primaries: sRGB red is (233.96, 51.07, 35.33) in P3.
		let mut image = Image { width: 7, height: 1, channels: 3, pixels: vec![] };
		let cases = [
			([255, 0, 0], [234, 51, 35]),
			([0, 255, 0], [117, 251, 76]),
			([0, 0, 255], [0, 0, 245]),
			([128, 128, 128], [128, 128, 128]),
			([200, 100, 50], [187, 105, 62]),
			([255, 255, 255], [255, 255, 255]),
			([0, 0, 0], [0, 0, 0]),
		];
		image.pixels = cases.iter().flat_map(|c| c.0).collect();
		srgb_to_p3(&mut image);
		assert_eq!(image.pixels, cases.iter().flat_map(|c| c.1).collect::<Vec<u8>>());
	}

	#[test]
	fn clip_profile_is_shared_or_p3() {
		use Profile::*;
		assert_eq!(clip_profile(&[Srgb, Srgb]), Srgb);
		assert_eq!(clip_profile(&[DisplayP3, DisplayP3]), DisplayP3);
		assert_eq!(clip_profile(&[DisplayP3, Srgb]), DisplayP3);
	}

	#[test]
	fn cicp_codes() {
		assert_eq!(from_cicp(12, 13), Ok(Profile::DisplayP3));
		assert_eq!(from_cicp(1, 13), Ok(Profile::Srgb));
		assert_eq!(from_cicp(2, 2), Ok(Profile::Srgb));
		assert!(from_cicp(9, 13).is_err(), "BT.2020");
		assert!(from_cicp(12, 16).is_err(), "PQ");
		assert!(from_cicp(1, 1).is_err(), "BT.709 transfer");
	}

	#[test]
	fn display_p3_icc_is_recognised_as_display_p3() {
		assert_eq!(from_icc(&display_p3_icc()), Ok(Profile::DisplayP3));
		assert_eq!(from_icc(&minimal_p3_icc()), Ok(Profile::DisplayP3));
	}

	/// macOS's own profiles: sRGB and Display P3 are recognised, everything else is refused.
	#[cfg(target_os = "macos")]
	#[test]
	fn system_icc_profiles() {
		let dir = std::path::Path::new("/System/Library/ColorSync/Profiles");
		let read = |name: &str| from_icc(&std::fs::read(dir.join(name)).unwrap());
		assert_eq!(read("sRGB Profile.icc"), Ok(Profile::Srgb));
		assert_eq!(read("Display P3.icc"), Ok(Profile::DisplayP3));
		for name in [
			"AdobeRGB1998.icc",
			"ITU-709.icc",
			"ITU-2020.icc",
			"DCI(P3) RGB.icc",
			"ROMM RGB.icc",
			"Generic Gray Gamma 2.2 Profile.icc",
			"Generic CMYK Profile.icc",
		] {
			assert!(read(name).is_err(), "{name}");
		}
		assert!(from_icc(b"short").is_err());
	}
}
