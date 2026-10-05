use std::path::PathBuf;

fn main() {
	let avif = pkg_config::Config::new()
		.atleast_version("1.1")
		.probe("libavif")
		.expect("brew install libavif");
	let header = avif
		.include_paths
		.iter()
		.map(|p| p.join("avif/avif.h"))
		.find(|p| p.exists())
		.expect("avif/avif.h");
	let bindings = bindgen::Builder::default()
		.header(header.to_string_lossy())
		.allowlist_item("avif.*")
		.allowlist_item("AVIF_.*")
		.prepend_enum_name(false)
		.derive_default(true)
		.generate()
		.expect("bindgen");
	let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
	bindings.write_to_file(out.join("avif_sys.rs")).unwrap();
	println!("cargo:rerun-if-changed=build.rs");
}
