//! Where the extracted assets live on disk, and the sprite sizes read from
//! them for the native tools: the replay runner (`src/bin/replay.rs`) and
//! the replay tests. The game itself gets its sizes from the sprites it
//! loaded (`extract::AssetSet::sprite_sizes`). The game logic (`game/`)
//! never touches the filesystem: it is handed a `SpriteSizes`.

use std::path::PathBuf;

use crate::extract::SPRITE_COUNT;
use crate::game::sprites::SpriteSizes;

/// Locate the extracted assets directory: `assets/` under the working
/// directory in development, `assets/` next to the executable (a Windows or
/// Linux folder), or `Contents/Resources/assets` relative to the executable
/// inside a packaged .app bundle. A directory counts only when it holds the
/// extractor's `meta.json`, so an unrelated `assets/` folder where the game
/// happens to be started is skipped.
pub fn asset_root() -> PathBuf {
    let cwd = PathBuf::from("assets");
    let has_game = |dir: &PathBuf| dir.join("meta.json").is_file();
    if has_game(&cwd) {
        return cwd;
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for candidate in [dir.join("assets"), dir.join("../Resources/assets")] {
                if has_game(&candidate) {
                    return candidate;
                }
            }
        }
    }
    cwd
}

/// The sprite sizes `tools/extract_assets.py` recorded in `meta.json`.
///
/// # Errors
///
/// When `meta.json` is missing or unreadable: a run without the real sizes
/// would be silently wrong (collisions, culling and density depend on them).
pub fn load_sprite_sizes() -> Result<SpriteSizes, String> {
    let path = asset_root().join("meta.json");
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("{}: {e} (extract the assets first: README, \"Getting the assets\")", path.display()))?;
    sprite_sizes_from_meta(&text).map_err(|e| format!("{}: {e}", path.display()))
}

#[derive(serde::Deserialize)]
struct Meta {
    resources: Vec<Resource>,
}

#[derive(serde::Deserialize)]
struct Resource {
    /// A number, or a name (foone's `ski32sounds.exe` has "WAVE").
    #[serde(rename = "type")]
    kind: serde_json::Value,
    /// A number, or a name (the `RT_GROUP_ICON` "ICONSKI").
    id: serde_json::Value,
    #[serde(default)]
    width: Option<i16>,
    #[serde(default)]
    height: Option<i16>,
}

/// Bitmap resource type (`RT_BITMAP`).
const RT_BITMAP: u64 = 2;

/// The sprite sizes in a `meta.json` text: 16x16 for ids without a bitmap.
///
/// # Errors
///
/// When the text is not a `meta.json` (no `resources` list).
pub fn sprite_sizes_from_meta(text: &str) -> Result<SpriteSizes, String> {
    let meta: Meta = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let mut sizes = SpriteSizes::placeholder();
    // Only bitmaps are sprites: the icon resources (RT_ICON = 3, ids 1-6,
    // 32x32) share ids with the skier sprites and must not overwrite them.
    for r in meta.resources.iter().filter(|r| r.kind.as_u64() == Some(RT_BITMAP)) {
        let id = r.id.as_u64().and_then(|id| u16::try_from(id).ok()).filter(|&id| usize::from(id) <= SPRITE_COUNT);
        if let (Some(id), Some(w), Some(h)) = (id, r.width, r.height) {
            sizes.set(id, (w, h));
        }
    }
    Ok(sizes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_do_not_overwrite_sprite_sizes() {
        let meta = r#"{"resources": [
            {"type": 2, "id": 1, "width": 16, "height": 32},
            {"type": 3, "id": 1, "width": 32, "height": 32},
            {"type": 2, "id": 3, "width": 24, "height": 28},
            {"type": 14, "id": "ICONSKI"},
            {"type": "WAVE", "id": 2}
        ]}"#;
        let sizes = sprite_sizes_from_meta(meta).unwrap();
        assert_eq!(sizes.get(1), (16, 32));
        assert_eq!(sizes.get(2), (16, 16), "no bitmap: the fallback");
        assert_eq!(sizes.get(3), (24, 28));
        assert!(sprite_sizes_from_meta("{}").is_err());
    }
}
