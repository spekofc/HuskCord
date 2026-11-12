// language: Rust, file: src/icon.rs
//
// Window icon, decoded at runtime from the embedded PNG.
//
// winit wants RGBA8 pixels; we embed the 256px asset and decode with the image
// crate. A 77 KB PNG in the binary is cheaper than shipping a 2 MB raw blob.

use winit::window::Icon;

/// Decode the embedded 256x256 logo into a winit Icon.
/// Returns None if the PNG is malformed - a missing icon is not fatal.
pub fn load() -> Option<Icon> {
    let bytes = include_bytes!("../assets/logo-256.png");
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Icon::from_rgba(img.into_raw(), w, h).ok()
}
