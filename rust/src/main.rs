mod avif_sys;
fn main() {
	let v = unsafe { std::ffi::CStr::from_ptr(avif_sys::avifVersion()) };
	println!("libavif {}", v.to_string_lossy());
}
