//! The browser side of the platform layer: Rust bindings for
//! `web/skifree.js`, a miniquad plugin that adds the functions below to the
//! wasm module's imports.
//!
//! Rust passes strings and byte slices as (pointer, length) into its own
//! memory. Data coming back (a stored value, a URL parameter, a dropped
//! file) arrives in two calls: the first returns its length (-1 for none)
//! and holds the bytes on the JS side, then `skifree_take` copies them into
//! a buffer Rust allocated, at most that buffer's length.
//!
//! Every import that takes (pointer, length) reads or copies those bytes
//! before it returns and keeps no pointer (see `register_plugin` in
//! skifree.js), so a live slice for the call is all each `unsafe` needs.

#[link(wasm_import_module = "env")]
extern "C" {
    fn skifree_take(dst: *mut u8, len: usize);
    fn skifree_set_title(ptr: *const u8, len: usize);
    fn skifree_is_fullscreen() -> u32;
    fn skifree_now() -> f64;
    fn skifree_storage_get(key: *const u8, key_len: usize) -> i32;
    fn skifree_storage_set(key: *const u8, key_len: usize, val: *const u8, val_len: usize);
    fn skifree_url_param(name: *const u8, len: usize) -> i32;
    fn skifree_audio_load(index: u32, ptr: *const u8, len: usize);
    fn skifree_audio_play(index: u32);
    fn skifree_audio_stop();
    fn skifree_set_favicon(size: u32, ptr: *const u8, len: usize);
    fn skifree_capture_frame(tick: u32, w: u32, h: u32, ptr: *const u8, len: usize);
    fn skifree_capture_done();
    fn skifree_capture_error(ptr: *const u8, len: usize);
    #[cfg_attr(feature = "embed-assets", expect(dead_code, reason = "the public build's file input"))]
    fn skifree_request_files();
    #[cfg_attr(feature = "embed-assets", expect(dead_code, reason = "the public build's file input"))]
    fn skifree_file_count() -> i32;
    #[cfg_attr(feature = "embed-assets", expect(dead_code, reason = "the public build's file input"))]
    fn skifree_file_name(index: u32) -> i32;
    #[cfg_attr(feature = "embed-assets", expect(dead_code, reason = "the public build's file input"))]
    fn skifree_file_bytes(index: u32) -> i32;
    #[cfg_attr(feature = "embed-assets", expect(dead_code, reason = "the public build's file input"))]
    fn skifree_assets_result(ok: u32, msg: *const u8, len: usize);
}

/// The plugin's version check (miniquad's `init_plugins` compares it with
/// the `version` of the JS plugin named "skifree"). 2: `skifree_take`
/// takes the buffer length; 3: `skifree_now`.
#[no_mangle]
pub extern "C" fn skifree_crate_version() -> u32 {
    3
}

/// Monotonic seconds (`performance.now()`), for the game timer.
pub fn monotonic_secs() -> f64 {
    // SAFETY: no arguments; it only reads performance.now().
    unsafe { skifree_now() }
}

fn take(len: i32) -> Option<Vec<u8>> {
    let len = usize::try_from(len).ok()?;
    let mut buf = vec![0u8; len];
    // SAFETY: the import writes at most `buf.len()` bytes into `buf` (it
    // holds `len` of them when the caller passes the length the previous
    // import returned; any other length only truncates or zero-fills).
    unsafe { skifree_take(buf.as_mut_ptr(), buf.len()) };
    Some(buf)
}

fn take_string(len: i32) -> Option<String> {
    take(len).map(|b| String::from_utf8_lossy(&b).into_owned())
}

/// `document.title`.
pub fn set_title(title: &str) {
    // SAFETY: the import reads `title` before it returns and keeps no pointer.
    unsafe { skifree_set_title(title.as_ptr(), title.len()) }
}

/// Whether the page fills the screen, by element fullscreen (Fullscreen API)
/// or the browser's own (F11): either way no tab title is on screen.
pub fn is_fullscreen() -> bool {
    // SAFETY: no arguments; it only reads the document.
    unsafe { skifree_is_fullscreen() != 0 }
}

/// `localStorage[key]`, if set and readable.
pub fn storage_get(key: &str) -> Option<String> {
    // SAFETY: the import reads `key` before it returns and keeps no pointer.
    take_string(unsafe { skifree_storage_get(key.as_ptr(), key.len()) })
}

/// `localStorage[key] = value` (silently dropped where storage is blocked).
pub fn storage_set(key: &str, value: &str) {
    // SAFETY: the import reads `key` and `value` before it returns and keeps no pointer.
    unsafe { skifree_storage_set(key.as_ptr(), key.len(), value.as_ptr(), value.len()) }
}

/// A query parameter of the page URL: `Some("")` for a bare `?name`.
pub fn url_param(name: &str) -> Option<String> {
    // SAFETY: the import reads `name` before it returns and keeps no pointer.
    take_string(unsafe { skifree_url_param(name.as_ptr(), name.len()) })
}

/// Hand WAV `index` (sound id - 1) to Web Audio.
pub fn audio_load(index: u32, wav: &[u8]) {
    // SAFETY: the import reads (copies) `wav` before it returns and keeps no pointer.
    unsafe { skifree_audio_load(index, wav.as_ptr(), wav.len()) }
}

/// Play WAV `index`, cutting off the one playing.
pub fn audio_play(index: u32) {
    // SAFETY: plain integer argument; an unloaded index plays nothing.
    unsafe { skifree_audio_play(index) }
}

pub fn audio_stop() {
    // SAFETY: no arguments.
    unsafe { skifree_audio_stop() }
}

/// A `size`x`size` RGBA favicon.
pub fn set_favicon(size: u32, rgba: &[u8]) {
    // SAFETY: the import reads (copies) `rgba` before it returns and keeps no pointer.
    unsafe { skifree_set_favicon(size, rgba.as_ptr(), rgba.len()) }
}

/// One captured frame (frame-capture mode), RGBA rows top-down.
pub fn capture_frame(tick: u32, width: u32, height: u32, rgba: &[u8]) {
    // SAFETY: the import reads (copies) `rgba` before it returns and keeps no pointer.
    unsafe { skifree_capture_frame(tick, width, height, rgba.as_ptr(), rgba.len()) }
}

/// Frame capture can't start: the page's capture tool reports `message`.
pub fn capture_error(message: &str) {
    // SAFETY: the import reads `message` before it returns and keeps no pointer.
    unsafe { skifree_capture_error(message.as_ptr(), message.len()) }
}

/// Frame capture finished.
pub fn capture_done() {
    // SAFETY: no arguments.
    unsafe { skifree_capture_done() }
}

/// Ask the page for the player's files (its cache, else the drop box)
/// (public build).
#[cfg_attr(feature = "embed-assets", expect(dead_code, reason = "the public build's file input"))]
pub fn request_files() {
    // SAFETY: no arguments.
    unsafe { skifree_request_files() }
}

/// The files the player gave, once they are there (each call after that
/// returns them again until `assets_result`).
#[cfg_attr(feature = "embed-assets", expect(dead_code, reason = "the public build's file input"))]
pub fn files() -> Option<Vec<(String, Vec<u8>)>> {
    // SAFETY: no arguments; -1 (no files yet) fails the conversion.
    let count = u32::try_from(unsafe { skifree_file_count() }).ok()?;
    (0..count)
        .map(|i| {
            // SAFETY: `i` < the count just read; the page keeps its files
            // until `assets_result`.
            let name = take_string(unsafe { skifree_file_name(i) })?;
            // SAFETY: as for the name.
            let bytes = take(unsafe { skifree_file_bytes(i) })?;
            Some((name, bytes))
        })
        .collect()
}

/// Tell the page whether the files worked: on success it caches them and
/// shows the game; otherwise it shows `message` and waits for others.
#[cfg_attr(feature = "embed-assets", expect(dead_code, reason = "the public build's file input"))]
pub fn assets_result(ok: bool, message: &str) {
    // SAFETY: the import reads `message` before it returns and keeps no pointer.
    unsafe { skifree_assets_result(u32::from(ok), message.as_ptr(), message.len()) }
}
