use std::path::{Path, PathBuf};
use std::process::Command;

// The same versions Homebrew ships, so the static build encodes and decodes exactly like the dynamic one.
//
// Each `url` is the primary source. `fallback`, when present, is an official release mirror run by
// the same project, used only if the primary download fails or its checksum does not match. Every
// checksum here was computed by downloading the file and running `shasum -a 256` on it.
const AOM: Source = Source {
	name: "libaom-3.15.0",
	// Google's dedicated release bucket for AOM; already the project's stable release mirror.
	url: "https://storage.googleapis.com/aom-releases/libaom-3.15.0.tar.gz",
	sha256: "ea08c38ecc078bc85bb1b691020e52b06250f1a81fe7ca5b624629225081af96",
	fallback: None,
};
const DAV1D: Source = Source {
	// code.videolan.org is a GitLab archive endpoint and can serve a bot-check page instead of the
	// tarball (observed 2026-10-06).
	name: "dav1d-1.5.4",
	url: "https://code.videolan.org/videolan/dav1d/-/archive/1.5.4/dav1d-1.5.4.tar.bz2",
	sha256: "2abfb0c89212e6e4733a54e0ae509ec00a5b845a6360946f918806e14aedb011",
	// VideoLAN's own release server, independent of the GitLab frontend.
	fallback: Some(Mirror {
		url: "https://downloads.videolan.org/pub/videolan/dav1d/1.5.4/dav1d-1.5.4.tar.xz",
		sha256: "686616b7c69eb88d44459391ab25cac13b6647a3b288835c5784e71c1514a5c5",
	}),
};
const LIBAVIF: Source = Source {
	// GitHub's tag-archive endpoint. libavif's GitHub Releases only carry prebuilt binaries, not a
	// source tarball, so there is no separate official mirror to fall back to.
	name: "libavif-1.4.2",
	url: "https://github.com/AOMediaCodec/libavif/archive/refs/tags/v1.4.2.tar.gz",
	sha256: "2b645287340ba5a631d268b551dc2d72bd73ac33335962dd36dcdb6d8366921d",
	fallback: None,
};

struct Source {
	name: &'static str,
	url: &'static str,
	sha256: &'static str,
	fallback: Option<Mirror>,
}

struct Mirror {
	url: &'static str,
	sha256: &'static str,
}

fn main() {
	println!("cargo:rerun-if-changed=build.rs");
	println!("cargo:rerun-if-env-changed=MACOSX_DEPLOYMENT_TARGET");
	locked_versions(&["png", "zune-jpeg"]);
	println!("cargo:rustc-link-lib=framework=ImageIO");
	println!("cargo:rustc-link-lib=framework=CoreGraphics");
	println!("cargo:rustc-link-lib=framework=CoreFoundation");
	if std::env::var_os("MACOSX_DEPLOYMENT_TARGET").is_none() {
		// rustc's default for aarch64-apple-darwin. Without it, cc and meson target the running macOS.
		// SAFETY: the build script is single-threaded here.
		unsafe { std::env::set_var("MACOSX_DEPLOYMENT_TARGET", "11.0") };
	}
	let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
	let started = std::time::Instant::now();
	let aom = build_aom(&out, &fetch(&out, &AOM));
	let dav1d = build_dav1d(&out, &fetch(&out, &DAV1D));
	let avif = build_libavif(&out, &fetch(&out, &LIBAVIF), &aom, &dav1d);
	eprintln!("pinframe: built libaom, dav1d and libavif in {:.1}s", started.elapsed().as_secs_f64());

	for (prefix, lib) in [(&avif, "avif"), (&aom, "aom"), (&dav1d, "dav1d")] {
		println!("cargo:rustc-link-search=native={}", prefix.join("lib").display());
		println!("cargo:rustc-link-lib=static={lib}");
	}

	let bindings = bindgen::Builder::default()
		.header(avif.join("include/avif/avif.h").to_string_lossy())
		.allowlist_item("avif.*")
		.allowlist_item("AVIF_.*")
		.prepend_enum_name(false)
		.derive_default(true)
		.generate()
		.expect("bindgen");
	bindings.write_to_file(out.join("avif_sys.rs")).unwrap();
}

/// Exposes each crate's locked version as `PINFRAME_VERSION_<NAME>` for the feature cache key.
fn locked_versions(crates: &[&str]) {
	println!("cargo:rerun-if-changed=Cargo.lock");
	let lock = std::fs::read_to_string("Cargo.lock").expect("Cargo.lock");
	for name in crates {
		let entry = format!("name = \"{name}\"\nversion = \"");
		let start = lock.find(&entry).unwrap_or_else(|| panic!("{name} is not in Cargo.lock")) + entry.len();
		let version = &lock[start..start + lock[start..].find('"').unwrap()];
		println!("cargo:rustc-env=PINFRAME_VERSION_{}={version}", name.replace('-', "_").to_uppercase());
	}
}

/// Shared cache directory for verified source tarballs, next to pinframe's feature cache
/// (`~/Library/Caches/pinframe`), so every worktree on the same machine downloads each tarball once.
fn sources_cache_dir() -> PathBuf {
	let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
	let dir = home.join("Library/Caches/pinframe-sources");
	std::fs::create_dir_all(&dir).unwrap();
	dir
}

/// Downloads and unpacks a source tarball into OUT_DIR once, after checking its SHA-256.
fn fetch(out: &Path, source: &Source) -> PathBuf {
	let dir = out.join("src").join(source.name);
	if dir.join(".unpacked").exists() {
		return dir;
	}
	let tarball = cached_tarball(source);
	std::fs::create_dir_all(&dir).unwrap();
	run(Command::new("tar").args(["xf"]).arg(&tarball).args(["--strip-components", "1", "-C"]).arg(&dir));
	std::fs::write(dir.join(".unpacked"), "").unwrap();
	dir
}

/// Returns the path to a verified tarball in the shared cache, downloading it (trying the fallback
/// mirror if the primary fails) if it is not already there with a matching checksum.
fn cached_tarball(source: &Source) -> PathBuf {
	let cache = sources_cache_dir();
	let mirrors = std::iter::once(Mirror { url: source.url, sha256: source.sha256 })
		.chain(source.fallback.iter().map(|m| Mirror { url: m.url, sha256: m.sha256 }));
	let mut last_error = String::new();
	for mirror in mirrors {
		// Keyed by checksum, not name: primary and fallback never collide or shadow each other.
		let path = cache.join(format!("{}-{}.tar", source.name, &mirror.sha256[..16]));
		if path.exists() && sha256_matches(&path, mirror.sha256) {
			return path;
		}
		match download_verified(&mirror, &cache, &path) {
			Ok(()) => return path,
			Err(e) => last_error = e,
		}
	}
	panic!("{}: every mirror failed; last error: {last_error}", source.name);
}

/// Downloads `mirror.url` to a temporary file next to `dest`, verifies its SHA-256, then renames it
/// into place atomically so a concurrent build never sees a half-written tarball.
fn download_verified(mirror: &Mirror, cache: &Path, dest: &Path) -> Result<(), String> {
	let name = dest.file_name().unwrap().to_string_lossy();
	let tmp = cache.join(format!(".{name}.{}.partial", std::process::id()));
	let status = Command::new("curl")
		.args(["-sSfL", "--retry", "3", "-o"])
		.arg(&tmp)
		.arg(mirror.url)
		.status()
		.map_err(|e| format!("{}: curl: {e}", mirror.url))?;
	if !status.success() {
		let _ = std::fs::remove_file(&tmp);
		return Err(format!("{}: curl exited with {status}", mirror.url));
	}
	if let Err(e) = check_is_tarball(&tmp, mirror.url) {
		let _ = std::fs::remove_file(&tmp);
		return Err(e);
	}
	if !sha256_matches(&tmp, mirror.sha256) {
		let got = sha256_of(&tmp);
		let _ = std::fs::remove_file(&tmp);
		return Err(format!("{}: checksum mismatch: expected {}, got {got}", mirror.url, mirror.sha256));
	}
	std::fs::rename(&tmp, dest).map_err(|e| format!("{}: rename into cache: {e}", mirror.url))?;
	Ok(())
}

/// Rejects anything that is not a gzip, bzip2 or xz stream, naming what came back instead (a bot-check
/// or error page from the host is typically HTML).
fn check_is_tarball(path: &Path, url: &str) -> Result<(), String> {
	let mut header = [0u8; 16];
	let n = std::fs::File::open(path)
		.and_then(|mut f| std::io::Read::read(&mut f, &mut header))
		.map_err(|e| format!("{url}: {e}"))?;
	let header = &header[..n];
	let is_gzip = header.starts_with(&[0x1f, 0x8b]);
	let is_bzip2 = header.starts_with(b"BZh");
	let is_xz = header.starts_with(&[0xfd, b'7', b'z', b'X', b'Z', 0x00]);
	if is_gzip || is_bzip2 || is_xz {
		return Ok(());
	}
	let text: String =
		header.iter().map(|&b| if b.is_ascii_graphic() || b == b' ' { b as char } else { '.' }).collect();
	Err(format!(
		"{url}: expected a tarball, got {} bytes starting with {text:?}",
		std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
	))
}

fn sha256_matches(path: &Path, expected: &str) -> bool {
	sha256_of(path) == expected
}

fn sha256_of(path: &Path) -> String {
	let output = Command::new("shasum").args(["-a", "256"]).arg(path).output().expect("shasum");
	let stdout = String::from_utf8(output.stdout).expect("shasum output is not utf8");
	stdout.split_whitespace().next().unwrap_or("").to_string()
}

fn build_aom(out: &Path, src: &Path) -> PathBuf {
	cmake::Config::new(src)
		.out_dir(out.join("aom"))
		.generator("Ninja")
		.profile("Release")
		.define("BUILD_SHARED_LIBS", "OFF")
		.define("CONFIG_AV1_DECODER", "0")
		.define("ENABLE_APPS", "OFF")
		.define("ENABLE_DOCS", "OFF")
		.define("ENABLE_EXAMPLES", "OFF")
		.define("ENABLE_TESTDATA", "OFF")
		.define("ENABLE_TESTS", "OFF")
		.define("ENABLE_TOOLS", "OFF")
		.build()
}

fn build_dav1d(out: &Path, src: &Path) -> PathBuf {
	let prefix = out.join("dav1d");
	let build = out.join("dav1d-build");
	if !build.join("build.ninja").exists() {
		run(Command::new("meson")
			.arg("setup")
			.arg(&build)
			.arg(src)
			.args(["--buildtype", "release", "--default-library", "static", "--libdir", "lib"])
			.args(["-Denable_tools=false", "-Denable_tests=false"])
			.arg(format!("--prefix={}", prefix.display())));
	}
	run(Command::new("meson").args(["install", "--quiet", "-C"]).arg(&build));
	prefix
}

fn build_libavif(out: &Path, src: &Path, aom: &Path, dav1d: &Path) -> PathBuf {
	cmake::Config::new(src)
		.out_dir(out.join("libavif"))
		.generator("Ninja")
		.profile("Release")
		.define("BUILD_SHARED_LIBS", "OFF")
		.define("AVIF_BUILD_APPS", "OFF")
		.define("AVIF_LIBYUV", "OFF")
		.define("AVIF_CODEC_AOM", "SYSTEM")
		.define("AVIF_CODEC_AOM_DECODE", "OFF")
		.define("AOM_INCLUDE_DIR", aom.join("include"))
		.define("AOM_LIBRARY", aom.join("lib/libaom.a"))
		.define("AVIF_CODEC_DAV1D", "SYSTEM")
		.define("DAV1D_INCLUDE_DIR", dav1d.join("include"))
		.define("DAV1D_LIBRARY", dav1d.join("lib/libdav1d.a"))
		// Only the libraries built here, never a system aom or dav1d.
		.env(
			"PKG_CONFIG_LIBDIR",
			format!("{}:{}", aom.join("lib/pkgconfig").display(), dav1d.join("lib/pkgconfig").display()),
		)
		.build()
}

fn run(command: &mut Command) {
	let status = command.status().unwrap_or_else(|e| panic!("{command:?}: {e}"));
	assert!(status.success(), "{command:?}: {status}");
}
