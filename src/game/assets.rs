use std::path::PathBuf;
use std::sync::OnceLock;

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

static SPRITE_DIMS: OnceLock<Vec<(i16, i16)>> = OnceLock::new();

/// Sprite sizes by id from the sprites the game actually loaded (the
/// platform layer installs them once at startup, before the first
/// `GameState`). Without them `GameState` reads `assets/meta.json`, as the
/// replay tools and tests do.
pub fn install_sprite_dims(dims: Vec<(i16, i16)>) {
    let _ = SPRITE_DIMS.set(dims);
}

pub fn installed_sprite_dims() -> Option<&'static [(i16, i16)]> {
    SPRITE_DIMS.get().map(Vec::as_slice)
}
