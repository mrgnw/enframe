use std::path::{Path, PathBuf};
use std::process::Command;

// The same versions Homebrew ships, so the static build encodes and decodes exactly like the dynamic one.
const AOM: Source = Source {
	name: "libaom-3.15.0",
	url: "https://storage.googleapis.com/aom-releases/libaom-3.15.0.tar.gz",
	sha256: "ea08c38ecc078bc85bb1b691020e52b06250f1a81fe7ca5b624629225081af96",
};
const DAV1D: Source = Source {
	name: "dav1d-1.5.4",
	url: "https://code.videolan.org/videolan/dav1d/-/archive/1.5.4/dav1d-1.5.4.tar.bz2",
	sha256: "2abfb0c89212e6e4733a54e0ae509ec00a5b845a6360946f918806e14aedb011",
};
const LIBAVIF: Source = Source {
	name: "libavif-1.4.2",
	url: "https://github.com/AOMediaCodec/libavif/archive/refs/tags/v1.4.2.tar.gz",
	sha256: "2b645287340ba5a631d268b551dc2d72bd73ac33335962dd36dcdb6d8366921d",
};

struct Source {
	name: &'static str,
	url: &'static str,
	sha256: &'static str,
}

fn main() {
	println!("cargo:rerun-if-changed=build.rs");
	println!("cargo:rerun-if-env-changed=MACOSX_DEPLOYMENT_TARGET");
	println!("cargo:rustc-link-lib=framework=ImageIO");
	println!("cargo:rustc-link-lib=framework=CoreGraphics");
	println!("cargo:rustc-link-lib=framework=CoreFoundation");
	if std::env::var_os("MACOSX_DEPLOYMENT_TARGET").is_none() {
		// rustc's default for aarch64-apple-darwin. Without it, cc and meson target the running macOS.
		// SAFETY: the build script is single-threaded here.
		unsafe { std::env::set_var("MACOSX_DEPLOYMENT_TARGET", "11.0") };
	}
	let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
	let aom = build_aom(&out, &fetch(&out, &AOM));
	let dav1d = build_dav1d(&out, &fetch(&out, &DAV1D));
	let avif = build_libavif(&out, &fetch(&out, &LIBAVIF), &aom, &dav1d);

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

/// Downloads and unpacks a source tarball into OUT_DIR once, after checking its SHA-256.
fn fetch(out: &Path, source: &Source) -> PathBuf {
	let dir = out.join("src").join(source.name);
	if dir.join(".unpacked").exists() {
		return dir;
	}
	let tarball = out.join("src").join(format!("{}.tar", source.name));
	std::fs::create_dir_all(&dir).unwrap();
	run(Command::new("curl").args(["-sSfL", "--retry", "3", "-o"]).arg(&tarball).arg(source.url));
	let sum = Command::new("shasum").args(["-a", "256"]).arg(&tarball).output().expect("shasum");
	let sum = String::from_utf8(sum.stdout).unwrap();
	assert!(sum.starts_with(source.sha256), "{}: checksum mismatch: {sum}", source.url);
	run(Command::new("tar").args(["xf"]).arg(&tarball).args(["--strip-components", "1", "-C"]).arg(&dir));
	std::fs::remove_file(&tarball).unwrap();
	std::fs::write(dir.join(".unpacked"), "").unwrap();
	dir
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
