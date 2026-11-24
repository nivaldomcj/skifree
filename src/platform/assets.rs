//! The game's data, loaded once at startup into an `AssetSet`:
//!
//! - native: the files `tools/extract_assets.py` wrote under `assets/`
//!   (sound comes from the WAVs build.rs embeds, see `platform::audio`);
//! - private web build (`--features embed-assets`): the same files,
//!   embedded by build.rs;
//! - public web build: extracted in the page from the player's own
//!   `ski32.exe` (`skifree::extract`).

use macroquad::prelude::*;
use skifree::extract::{AssetSet, Rgba};

pub async fn load() -> AssetSet {
    imp::load().await
}

/// Decode a PNG the way macroquad's `load_texture` does.
#[cfg_attr(
    all(target_arch = "wasm32", not(feature = "embed-assets")),
    expect(dead_code, reason = "the public web build gets its images from extract, not PNGs")
)]
fn decode_png(bytes: &[u8]) -> Option<Rgba> {
    let img = Image::from_file_with_format(bytes, Some(ImageFormat::Png)).ok()?;
    Some(Rgba { width: u32::from(img.width), height: u32::from(img.height), data: img.bytes })
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use super::decode_png;
    use skifree::extract::{AssetSet, Rgba};
    use std::path::Path;

    fn read_png(path: &Path) -> Option<Rgba> {
        decode_png(&std::fs::read(path).ok()?)
    }

    #[expect(clippy::unused_async, reason = "the same signature as the public web version, which awaits")]
    pub async fn load() -> AssetSet {
        let root = skifree::assets::asset_root();
        let mut set = AssetSet::new();
        // Slot 0 is unused: sprites are indexed by resource id, 1..=SPRITE_COUNT.
        for (id, sprite) in set.sprites.iter_mut().enumerate().skip(1) {
            let path = root.join(format!("sprites/bitmap_{id:02}.png"));
            *sprite = read_png(&path);
            if sprite.is_none() {
                eprintln!("warning: failed to load sprite {}", path.display());
            }
        }
        // Terminal extracted from the user's vgaoem.fon if present; the
        // renderer falls back to the bundled IBM VGA font.
        set.font = read_png(&root.join("fonts/oem_fixed.png"));
        set
    }
}

#[cfg(all(target_arch = "wasm32", feature = "embed-assets"))]
mod imp {
    use super::decode_png;
    use skifree::extract::AssetSet;

    mod embedded {
        include!(concat!(env!("OUT_DIR"), "/web_assets.rs"));
    }

    #[expect(clippy::unused_async, reason = "the same signature as the public web version, which awaits")]
    pub async fn load() -> AssetSet {
        let mut set = AssetSet::new();
        for (i, png) in embedded::SPRITES.iter().enumerate() {
            set.sprites[i + 1] = decode_png(png);
        }
        set.font = embedded::FONT.and_then(decode_png);
        set.icon = embedded::ICON.and_then(decode_png);
        set.sounds = embedded::SOUNDS.map(|wavs| wavs.iter().map(|w| w.to_vec()).collect());
        set
    }
}

#[cfg(all(target_arch = "wasm32", not(feature = "embed-assets")))]
mod imp {
    use crate::platform::web;
    use macroquad::prelude::next_frame;
    use skifree::extract::AssetSet;

    /// Wait for the player's files (the page's cache or its drop box) and
    /// extract them; a file set that doesn't work is reported to the page,
    /// which then waits for another.
    pub async fn load() -> AssetSet {
        web::request_files();
        loop {
            if let Some(files) = web::files() {
                match skifree::extract::from_files(&files) {
                    Ok(set) => {
                        web::assets_result(true, "");
                        return set;
                    }
                    Err(e) => web::assets_result(false, &e),
                }
            }
            next_frame().await;
        }
    }
}
