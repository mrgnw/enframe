//! `.mp4` clips without ffmpeg: H.264 High through the vendored filmcraft encoder, muxed with its writer.
//! Matches what the ffmpeg path wrote: 8-bit 4:2:0, BT.709 matrix, limited range, the photos' primaries
//! with the sRGB transfer in both the VUI and `colr`, and the run ID as the comment.

use crate::colour::{Profile, SRGB_TRANSFER};
use crate::yuv::Yuv;
use filmcraft_h264enc::{ColorConfig, Encoder, EncoderConfig, Packet, PacketFormat, RateControl, YuvFrame};
use filmcraft_isobmff::{
	AvcConfig, Brand, ColorInfo, Mp4Writer, SampleEntry, TrackConfig, WriteSample, WriterOptions,
};
use std::fs::File;
use std::path::Path;

/// H.273 matrix coefficients 1, BT.709.
const MATRIX: u16 = 1;
/// The encoder's CRF that matched x264's CRF 18 (SSIM 0.993) on the train clouds photos (#30).
const CRF: f32 = 15.0;

pub struct Writer {
	encoder: Encoder,
	mux: Mp4Writer<File>,
	entry: Option<SampleEntry>,
	track: usize,
	frames: i64,
}

/// `fps` as a fraction in thousandths, reduced: 4 is 4/1, 29.97 is 2997/100.
fn fraction(fps: f64) -> (u32, u32) {
	let num = (fps * 1000.0).round() as u32;
	assert!(num > 0, "--fps is positive");
	let gcd = |mut a: u32, mut b: u32| {
		while b != 0 {
			(a, b) = (b, a % b);
		}
		a
	};
	let g = gcd(num, 1000);
	(num / g, 1000 / g)
}

impl Writer {
	pub fn new(out: &Path, size: (u32, u32), fps: f64, run: &str, profile: Profile) -> Result<Self, String> {
		let (fps_num, fps_den) = fraction(fps);
		let mut config = EncoderConfig::new(size.0, size.1, fps_num, fps_den);
		config.format = PacketFormat::LengthPrefixed;
		config.rate = RateControl::Crf(CRF);
		config.color = ColorConfig {
			primaries: profile.primaries() as u8,
			transfer: SRGB_TRANSFER as u8,
			matrix: MATRIX as u8,
			full_range: false,
		};
		let encoder = Encoder::new(config).map_err(|e| format!("H.264: {e}"))?;
		let avc = AvcConfig::parse(&encoder.avcc()).map_err(|e| format!("H.264: {e}"))?;
		let mut entry = SampleEntry::avc(avc, size.0 as u16, size.1 as u16);
		entry.video.as_mut().expect("an AVC entry is video").color = Some(ColorInfo::Nclx {
			primaries: profile.primaries(),
			transfer: SRGB_TRANSFER,
			matrix: MATRIX,
			full_range: false,
		});
		// finish_faststart reads the samples back to move them after the header.
		let file =
			File::options().read(true).write(true).truncate(true).open(out).map_err(|e| e.to_string())?;
		let mut options = WriterOptions::new(Brand::Mp4);
		options.metadata = vec![("\u{a9}cmt".into(), format!("stabilize-run={run}"))];
		let mux = Mp4Writer::new(file, options).map_err(|e| e.to_string())?;
		Ok(Writer { encoder, mux, entry: Some(entry), track: 0, frames: 0 })
	}

	pub fn add(&mut self, frame: &Yuv) -> Result<(), String> {
		let [y, u, v] = &frame.planes;
		let frame = YuvFrame { y, u, v, y_stride: frame.width, uv_stride: frame.width / 2 };
		let packets = self.encoder.try_encode(&frame, self.frames).map_err(|e| format!("H.264: {e}"))?;
		self.frames += 1;
		self.write(&packets)
	}

	pub fn finish(mut self) -> Result<(), String> {
		let packets = self.encoder.flush();
		self.write(&packets)?;
		assert!(self.entry.is_none(), "a clip has frames");
		self.mux.finish_faststart().map_err(|e| e.to_string())?;
		Ok(())
	}

	/// Packets in the frame timescale; the track's timescale is fps_num, so each frame lasts fps_den.
	fn write(&mut self, packets: &[Packet]) -> Result<(), String> {
		let (num, den) = (self.encoder.config().fps_num, self.encoder.config().fps_den);
		for packet in packets {
			if let Some(entry) = self.entry.take() {
				// B-frames start decoding before frame 1's time; the edit list shows frame 1 at 0.
				let mut track = TrackConfig::new(entry, num);
				track.media_start = Some(-packet.dts * den as i64);
				self.track = self.mux.add_track(track).map_err(|e| e.to_string())?;
			}
			let sample = WriteSample {
				data: &packet.data,
				duration: den,
				composition_offset: ((packet.pts - packet.dts) * den as i64) as i32,
				is_sync: packet.keyframe,
			};
			self.mux.write_sample(self.track, sample).map_err(|e| e.to_string())?;
		}
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn fps_fractions() {
		assert_eq!(fraction(4.0), (4, 1));
		assert_eq!(fraction(0.5), (1, 2));
		assert_eq!(fraction(29.97), (2997, 100));
		assert_eq!(fraction(1.0 / 3.0), (333, 1000));
	}

	fn gradient(t: usize) -> crate::pixels::Image {
		let (w, h) = (64, 48);
		let pixels =
			(0..w * h).flat_map(|i| [((i % w) * 4 + t * 8) as u8, ((i / w) * 5) as u8, 128]).collect();
		crate::pixels::Image { width: w, height: h, channels: 3, pixels }
	}

	/// The clip as ffprobe reads it, decoding every frame. Skipped without ffprobe on PATH.
	#[test]
	fn ffprobe_reads_stream_tags_and_every_frame() {
		if !crate::on_path("ffprobe") {
			eprintln!("skipped: no ffprobe on PATH");
			return;
		}
		for (frames, profile, primaries) in [(8, Profile::DisplayP3, "smpte432"), (1, Profile::Srgb, "bt709")]
		{
			let path =
				std::env::temp_dir().join(format!("stabilize-mp4-{frames}-{}.mp4", std::process::id()));
			File::create(&path).unwrap();
			let mut writer = Writer::new(&path, (64, 48), 4.0, "test-run", profile).unwrap();
			for t in 0..frames {
				writer
					.add(&crate::yuv::to_yuv420(&gradient(t), crate::yuv::BT709, crate::yuv::LIMITED))
					.unwrap();
			}
			writer.finish().unwrap();
			let probe = std::process::Command::new("ffprobe")
				.args(["-v", "error", "-count_frames", "-select_streams", "v", "-of", "default=nw=1"])
				.args([
					"-show_entries",
					"stream=codec_name,profile,pix_fmt,width,height,r_frame_rate,nb_read_frames",
				])
				.args([
					"-show_entries",
					"stream=color_primaries,color_transfer,color_space,color_range,start_time",
				])
				.args(["-show_entries", "format_tags=comment"])
				.arg(&path)
				.output()
				.unwrap();
			std::fs::remove_file(&path).unwrap();
			assert!(
				probe.status.success() && probe.stderr.is_empty(),
				"{}",
				String::from_utf8_lossy(&probe.stderr)
			);
			let stdout = String::from_utf8(probe.stdout).unwrap();
			let expected = [
				"codec_name=h264".into(),
				"profile=High".into(),
				"width=64".into(),
				"height=48".into(),
				"pix_fmt=yuv420p".into(),
				"color_range=tv".into(),
				"color_space=bt709".into(),
				"color_transfer=iec61966-2-1".into(),
				format!("color_primaries={primaries}"),
				"r_frame_rate=4/1".into(),
				"start_time=0.000000".into(),
				format!("nb_read_frames={frames}"),
				"TAG:comment=stabilize-run=test-run".into(),
			];
			assert_eq!(stdout.lines().collect::<Vec<_>>(), expected, "{frames} frames");
		}
	}
}
