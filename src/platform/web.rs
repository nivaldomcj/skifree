//! The browser side of the platform layer: Rust bindings for
//! `web/skifree.js`, a miniquad plugin that adds the functions below to the
//! wasm module's imports.
//!
//! Rust passes strings and byte slices as (pointer, length) into its own
//! memory. Data coming back (a stored value, a URL parameter, a dropped
//! file) arrives in two calls: the first returns its length (-1 for none)
//! and holds the bytes on the JS side, then `skifree_take` copies them into
//! a buffer Rust allocated.

#[link(wasm_import_module = "env")]
extern "C" {
    fn skifree_take(dst: *mut u8);
    fn skifree_set_title(ptr: *const u8, len: usize);
    fn skifree_is_fullscreen() -> u32;
    fn skifree_storage_get(key: *const u8, key_len: usize) -> i32;
    fn skifree_storage_set(key: *const u8, key_len: usize, val: *const u8, val_len: usize);
    fn skifree_url_param(name: *const u8, len: usize) -> i32;
    fn skifree_audio_load(index: u32, ptr: *const u8, len: usize);
    fn skifree_audio_play(index: u32);
    fn skifree_audio_stop();
    fn skifree_set_favicon(size: u32, ptr: *const u8, len: usize);
    fn skifree_capture_frame(tick: u32, w: u32, h: u32, ptr: *const u8, len: usize);
    fn skifree_capture_done();
    #[cfg_attr(feature = "embed-assets", allow(dead_code))]
    fn skifree_request_files();
    #[cfg_attr(feature = "embed-assets", allow(dead_code))]
    fn skifree_file_count() -> i32;
    #[cfg_attr(feature = "embed-assets", allow(dead_code))]
    fn skifree_file_name(index: u32) -> i32;
    #[cfg_attr(feature = "embed-assets", allow(dead_code))]
    fn skifree_file_bytes(index: u32) -> i32;
    #[cfg_attr(feature = "embed-assets", allow(dead_code))]
    fn skifree_assets_result(ok: u32, msg: *const u8, len: usize);
}

/// The plugin's version check (miniquad's `init_plugins` compares it with
/// the `version` of the JS plugin named "skifree").
#[no_mangle]
pub extern "C" fn skifree_crate_version() -> u32 {
    1
}

fn take(len: i32) -> Option<Vec<u8>> {
    let len = usize::try_from(len).ok()?;
    let mut buf = vec![0u8; len];
    unsafe { skifree_take(buf.as_mut_ptr()) };
    Some(buf)
}

fn take_string(len: i32) -> Option<String> {
    take(len).map(|b| String::from_utf8_lossy(&b).into_owned())
}

/// `document.title`.
pub fn set_title(title: &str) {
    unsafe { skifree_set_title(title.as_ptr(), title.len()) }
}

/// Whether the page is in fullscreen (no tab title on screen).
pub fn is_fullscreen() -> bool {
    unsafe { skifree_is_fullscreen() != 0 }
}

/// `localStorage[key]`, if set and readable.
pub fn storage_get(key: &str) -> Option<String> {
    take_string(unsafe { skifree_storage_get(key.as_ptr(), key.len()) })
}

/// `localStorage[key] = value` (silently dropped where storage is blocked).
pub fn storage_set(key: &str, value: &str) {
    unsafe { skifree_storage_set(key.as_ptr(), key.len(), value.as_ptr(), value.len()) }
}

/// A query parameter of the page URL: `Some("")` for a bare `?name`.
pub fn url_param(name: &str) -> Option<String> {
    take_string(unsafe { skifree_url_param(name.as_ptr(), name.len()) })
}

/// Hand WAV `index` (sound id - 1) to Web Audio.
pub fn audio_load(index: u32, wav: &[u8]) {
    unsafe { skifree_audio_load(index, wav.as_ptr(), wav.len()) }
}

/// Play WAV `index`, cutting off the one playing.
pub fn audio_play(index: u32) {
    unsafe { skifree_audio_play(index) }
}

pub fn audio_stop() {
    unsafe { skifree_audio_stop() }
}

/// A `size`x`size` RGBA favicon.
pub fn set_favicon(size: u32, rgba: &[u8]) {
    unsafe { skifree_set_favicon(size, rgba.as_ptr(), rgba.len()) }
}

/// One captured frame (frame-capture mode), RGBA rows top-down.
pub fn capture_frame(tick: u32, width: u32, height: u32, rgba: &[u8]) {
    unsafe { skifree_capture_frame(tick, width, height, rgba.as_ptr(), rgba.len()) }
}

/// Frame capture finished.
pub fn capture_done() {
    unsafe { skifree_capture_done() }
}

/// Ask the page for the player's files (its cache, else the drop box)
/// (public build).
#[cfg_attr(feature = "embed-assets", allow(dead_code))]
pub fn request_files() {
    unsafe { skifree_request_files() }
}

/// The files the player gave, once they are there (each call after that
/// returns them again until `assets_result`).
#[cfg_attr(feature = "embed-assets", allow(dead_code))]
pub fn files() -> Option<Vec<(String, Vec<u8>)>> {
    let count = u32::try_from(unsafe { skifree_file_count() }).ok()?;
    (0..count)
        .map(|i| {
            let name = take_string(unsafe { skifree_file_name(i) })?;
            let bytes = take(unsafe { skifree_file_bytes(i) })?;
            Some((name, bytes))
        })
        .collect()
}

/// Tell the page whether the files worked: on success it caches them and
/// shows the game; otherwise it shows `message` and waits for others.
#[cfg_attr(feature = "embed-assets", allow(dead_code))]
pub fn assets_result(ok: bool, message: &str) {
    unsafe { skifree_assets_result(ok as u32, message.as_ptr(), message.len()) }
}
