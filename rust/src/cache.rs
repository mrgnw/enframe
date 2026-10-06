//! Durable, content-addressed SIFT feature cache.
//!
//! Entries are deliberately independent: a BLAKE3 content key includes every result-affecting
//! input, and the entry has its own BLAKE3 checksum. A bad entry is simply a miss.

use crate::sift::{DESCRIPTOR_LEN, Features, KeyPoint};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const MAGIC: &[u8; 8] = b"PFCACHE1";
const FORMAT: u32 = 2;
const SIFT_ID: &str = "opencv-sift-port-2026-10-05";

pub struct CachedFeatures {
	pub small: (usize, usize),
	pub features: Features,
}

pub struct FeatureCache {
	root: PathBuf,
	cap: u64,
}

impl FeatureCache {
	pub fn new(root: PathBuf) -> Result<Self, String> {
		std::fs::create_dir_all(&root).map_err(|e| format!("--cache {}: {e}", root.display()))?;
		Ok(Self { cap: default_cap(&root)?, root })
	}

	pub fn load(&self, key: &str) -> Option<CachedFeatures> {
		let path = self.root.join(format!("{key}.pfc"));
		let bytes = std::fs::read(&path).ok()?;
		let parsed = decode(&bytes)?;
		// mtime is the LRU clock. Failure is harmless: eviction becomes approximate.
		let _ = std::fs::File::open(&path).and_then(|f| f.set_modified(std::time::SystemTime::now()));
		Some(parsed)
	}

	pub fn store(&self, key: &str, small: (usize, usize), features: &Features) {
		let path = self.root.join(format!("{key}.pfc"));
		if self.load(key).is_some() {
			return;
		}
		let bytes = encode(small, features);
		let tmp = self.root.join(format!(".{key}.{}.partial", random_hex()));
		let Ok(mut file) = std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp) else {
			return;
		};
		if file.write_all(&bytes).is_ok() {
			let _ = std::fs::rename(&tmp, &path);
		}
		let _ = std::fs::remove_file(tmp);
		self.evict();
	}

	fn evict(&self) {
		let Ok(entries) = std::fs::read_dir(&self.root) else { return };
		let mut files: Vec<_> = entries
			.flatten()
			.filter_map(|entry| {
				let path = entry.path();
				(path.extension().is_some_and(|x| x == "pfc"))
					.then(|| entry.metadata().ok().map(|m| (m.modified().ok(), m.len(), path)))?
			})
			.collect();
		let mut total: u64 = files.iter().map(|(_, n, _)| *n).sum();
		files.sort_by_key(|(time, _, _)| *time);
		for (_, bytes, path) in files {
			if total <= self.cap {
				break;
			}
			if std::fs::remove_file(path).is_ok() {
				total = total.saturating_sub(bytes);
			}
		}
	}
}

/// The key hashes photo bytes, the decoder and its version, the global scaling decision and every
/// cache-format/SIFT constant.
pub fn key(
	path: &Path,
	decoder: &str,
	k: f64,
	small: (usize, usize),
	track_edge: f64,
) -> Result<String, String> {
	let mut file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
	let mut hash = blake3::Hasher::new();
	hash.update(b"pinframe-feature-key\0");
	hash.update(&FORMAT.to_le_bytes());
	hash.update(SIFT_ID.as_bytes());
	hash.update(&(decoder.len() as u64).to_le_bytes());
	hash.update(decoder.as_bytes());
	hash.update(&track_edge.to_bits().to_le_bytes());
	hash.update(&k.to_bits().to_le_bytes());
	hash.update(&(small.0 as u64).to_le_bytes());
	hash.update(&(small.1 as u64).to_le_bytes());
	let mut buffer = [0; 1 << 20];
	loop {
		let n = file.read(&mut buffer).map_err(|e| format!("{}: {e}", path.display()))?;
		if n == 0 {
			break;
		}
		hash.update(&buffer[..n]);
	}
	Ok(hash.finalize().to_hex().to_string())
}

fn encode(small: (usize, usize), features: &Features) -> Vec<u8> {
	assert_eq!(features.keypoints.len(), features.descriptors.len());
	let n = features.keypoints.len();
	let mut out = Vec::with_capacity(32 + n * (24 + DESCRIPTOR_LEN) + 32);
	out.extend_from_slice(MAGIC);
	out.extend_from_slice(&FORMAT.to_le_bytes());
	out.extend_from_slice(&(small.0 as u32).to_le_bytes());
	out.extend_from_slice(&(small.1 as u32).to_le_bytes());
	out.extend_from_slice(&(n as u32).to_le_bytes());
	for (kp, descriptor) in features.keypoints.iter().zip(&features.descriptors) {
		for value in [kp.x, kp.y, kp.size, kp.angle, kp.response] {
			out.extend_from_slice(&value.to_le_bytes());
		}
		out.extend_from_slice(&kp.octave.to_le_bytes());
		out.extend_from_slice(descriptor);
	}
	let checksum = blake3::hash(&out);
	out.extend_from_slice(checksum.as_bytes());
	out
}

fn decode(bytes: &[u8]) -> Option<CachedFeatures> {
	if bytes.len() < 56 || &bytes[..8] != MAGIC {
		return None;
	}
	let payload = bytes.get(..bytes.len().checked_sub(32)?)?;
	if blake3::hash(payload).as_bytes() != bytes.get(bytes.len() - 32..)? {
		return None;
	}
	let mut at = 8;
	let take_u32 = |at: &mut usize| {
		let x: [u8; 4] = payload.get(*at..*at + 4)?.try_into().ok()?;
		*at += 4;
		Some(u32::from_le_bytes(x))
	};
	if take_u32(&mut at)? != FORMAT {
		return None;
	}
	let small = (take_u32(&mut at)? as usize, take_u32(&mut at)? as usize);
	let n = take_u32(&mut at)? as usize;
	let record = 24usize.checked_add(DESCRIPTOR_LEN)?;
	if n.checked_mul(record)?.checked_add(at)? != payload.len() {
		return None;
	}
	let mut features = Features::default();
	features.keypoints.reserve(n);
	features.descriptors.reserve(n);
	for _ in 0..n {
		let f32_at = |at: &mut usize| {
			let x: [u8; 4] = payload.get(*at..*at + 4)?.try_into().ok()?;
			*at += 4;
			Some(f32::from_le_bytes(x))
		};
		let x = f32_at(&mut at)?;
		let y = f32_at(&mut at)?;
		let size = f32_at(&mut at)?;
		let angle = f32_at(&mut at)?;
		let response = f32_at(&mut at)?;
		let octave = take_u32(&mut at)? as i32;
		let descriptor: [u8; DESCRIPTOR_LEN] = payload.get(at..at + DESCRIPTOR_LEN)?.try_into().ok()?;
		at += DESCRIPTOR_LEN;
		features.keypoints.push(KeyPoint { x, y, size, angle, response, octave });
		features.descriptors.push(descriptor);
	}
	Some(CachedFeatures { small, features })
}

fn default_cap(path: &Path) -> Result<u64, String> {
	use std::ffi::CString;
	use std::os::unix::ffi::OsStrExt;
	let name =
		CString::new(path.as_os_str().as_bytes()).map_err(|_| "--cache path contains a NUL".to_string())?;
	let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
	if unsafe { libc::statvfs(name.as_ptr(), &mut stat) } != 0 {
		return Err(format!("--cache {}: cannot inspect filesystem", path.display()));
	}
	let free = (stat.f_bavail as u128 * stat.f_frsize as u128) as u64;
	// Five percent of currently free disk, bounded to a useful 512 MiB--8 GiB range.
	Ok((free / 20).clamp(512 << 20, 8 << 30))
}

fn random_hex() -> String {
	let mut bytes = [0u8; 8];
	unsafe { libc::arc4random_buf(bytes.as_mut_ptr() as *mut _, bytes.len()) };
	bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::sync::Arc;

	fn sample() -> CachedFeatures {
		CachedFeatures {
			small: (9, 7),
			features: Features {
				keypoints: vec![KeyPoint { x: 1.0, y: 2.0, size: 3.0, angle: 4.0, response: 5.0, octave: 6 }],
				descriptors: vec![[7; DESCRIPTOR_LEN]],
			},
		}
	}

	#[test]
	fn entries_reject_corruption() {
		let sample = sample();
		let mut bytes = encode(sample.small, &sample.features);
		assert!(decode(&bytes).is_some());
		bytes[20] ^= 1;
		assert!(decode(&bytes).is_none());
	}

	fn temp(name: &str) -> PathBuf {
		let path = std::env::temp_dir().join(format!("pinframe-cache-{name}-{}", random_hex()));
		std::fs::create_dir_all(&path).unwrap();
		path
	}

	fn run_writer(exe: PathBuf, root: PathBuf) -> std::process::ExitStatus {
		std::process::Command::new(exe)
			.args(["--exact", "cache::tests::process_writer"])
			.env("PINFRAME_CACHE_WRITER", root)
			.status()
			.unwrap()
	}

	#[test]
	fn hit_miss_and_corrupt_entry_recomputes() {
		let root = temp("hit");
		let cache = FeatureCache { root: root.clone(), cap: u64::MAX };
		assert!(cache.load("a").is_none());
		let sample = sample();
		cache.store("a", sample.small, &sample.features);
		assert_eq!(cache.load("a").unwrap().features.descriptors[0][0], 7);
		let path = root.join("a.pfc");
		let mut bad = std::fs::read(&path).unwrap();
		bad[16] ^= 1;
		std::fs::write(&path, bad).unwrap();
		assert!(cache.load("a").is_none());
		cache.store("a", sample.small, &sample.features);
		assert!(cache.load("a").is_some());
		let _ = std::fs::remove_dir_all(root);
	}

	#[test]
	fn key_changes_with_the_global_downscale() {
		let root = temp("key");
		let photo = root.join("photo.avif");
		std::fs::write(&photo, b"pixels").unwrap();
		let decoder = "libavif 1.4.2";
		assert_ne!(
			key(&photo, decoder, 1.0, (100, 80), 1024.0).unwrap(),
			key(&photo, decoder, 0.5, (50, 40), 1024.0).unwrap()
		);
		let _ = std::fs::remove_dir_all(root);
	}

	#[test]
	fn key_changes_with_the_decoder_and_its_version() {
		let root = temp("decoder-key");
		let photo = root.join("photo.avif");
		std::fs::write(&photo, b"pixels").unwrap();
		let keys: Vec<String> = [
			"libavif 1.4.2; dav1d [dec]:1.5.4, aom [enc]:3.15.0",
			"libavif 1.4.2; dav1d [dec]:1.5.5, aom [enc]:3.15.0",
			"libavif 1.4.3; dav1d [dec]:1.5.4, aom [enc]:3.15.0",
			"ImageIO macOS 25A354",
			"ImageIO macOS 25B78",
		]
		.iter()
		.map(|decoder| key(&photo, decoder, 1.0, (100, 80), 1024.0).unwrap())
		.collect();
		let unique: std::collections::HashSet<_> = keys.iter().collect();
		assert_eq!(unique.len(), keys.len());
		let _ = std::fs::remove_dir_all(root);
	}

	#[test]
	fn warm_cache_hits_within_one_build_and_os() {
		let root = temp("warm");
		let cache = FeatureCache { root: root.clone(), cap: u64::MAX };
		let sample = sample();
		for (photo, decoder) in [
			("../samples/train clouds/IMG_4667.avif", "dav1d [dec]:"),
			("testdata/orientation-6.heic", "ImageIO macOS "),
			("testdata/orientation-6.jxl", "ImageIO macOS "),
			("testdata/orientation-6.png", "png 0."),
		] {
			let photo = Path::new(photo);
			let id = crate::decode::decoder_id(photo).unwrap();
			assert!(id.contains(decoder), "{id}");
			assert!(std::ptr::eq(id, crate::decode::decoder_id(photo).unwrap()), "read once per run");
			let cold = key(photo, id, 1.0, (100, 80), 1024.0).unwrap();
			assert!(cache.load(&cold).is_none());
			cache.store(&cold, sample.small, &sample.features);
			let warm = key(photo, crate::decode::decoder_id(photo).unwrap(), 1.0, (100, 80), 1024.0).unwrap();
			assert!(cache.load(&warm).is_some(), "{}", photo.display());
		}
		let _ = std::fs::remove_dir_all(root);
	}

	#[test]
	fn simultaneous_cold_writers_leave_a_valid_entry() {
		let root = temp("race");
		let cache = Arc::new(FeatureCache { root: root.clone(), cap: u64::MAX });
		let a = {
			let cache = cache.clone();
			std::thread::spawn(move || {
				let sample = sample();
				cache.store("same", sample.small, &sample.features)
			})
		};
		let b = {
			let cache = cache.clone();
			std::thread::spawn(move || {
				let sample = sample();
				cache.store("same", sample.small, &sample.features)
			})
		};
		a.join().unwrap();
		b.join().unwrap();
		assert!(cache.load("same").is_some());
		assert_eq!(
			std::fs::read_dir(&root)
				.unwrap()
				.flatten()
				.filter(|e| e.path().extension().is_some_and(|x| x == "pfc"))
				.count(),
			1
		);
		let _ = std::fs::remove_dir_all(root);
	}

	#[test]
	fn process_writer() {
		let Some(root) = std::env::var_os("PINFRAME_CACHE_WRITER") else { return };
		let cache = FeatureCache { root: PathBuf::from(root), cap: u64::MAX };
		let sample = sample();
		cache.store("same", sample.small, &sample.features);
	}

	#[test]
	fn concurrent_processes_leave_one_valid_entry() {
		let root = temp("process-race");
		let exe = std::env::current_exe().unwrap();
		let first = std::thread::spawn({
			let exe = exe.clone();
			let root = root.clone();
			move || run_writer(exe, root)
		});
		let second = std::thread::spawn({
			let exe = exe.clone();
			let root = root.clone();
			move || run_writer(exe, root)
		});
		assert!(first.join().unwrap().success());
		assert!(second.join().unwrap().success());
		let cache = FeatureCache { root: root.clone(), cap: u64::MAX };
		assert!(cache.load("same").is_some());
		assert_eq!(
			std::fs::read_dir(&root)
				.unwrap()
				.flatten()
				.filter(|e| e.path().extension().is_some_and(|x| x == "pfc"))
				.count(),
			1
		);
		let _ = std::fs::remove_dir_all(root);
	}

	#[test]
	fn eviction_holds_the_cap() {
		let root = temp("evict");
		let sample = sample();
		let bytes = encode(sample.small, &sample.features).len() as u64;
		let cache = FeatureCache { root: root.clone(), cap: bytes };
		cache.store("old", sample.small, &sample.features);
		cache.store("new", sample.small, &sample.features);
		let total: u64 =
			std::fs::read_dir(&root).unwrap().flatten().map(|e| e.metadata().unwrap().len()).sum();
		assert!(total <= bytes);
		assert!(cache.load("new").is_some());
		let _ = std::fs::remove_dir_all(root);
	}
}
