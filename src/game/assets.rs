use std::path::PathBuf;

/// Locate the extracted assets directory: `assets/` under the working
/// directory in development, `assets/` next to the executable (a Windows or
/// Linux folder), or `Contents/Resources/assets` relative to the executable
/// inside a packaged .app bundle.
pub fn asset_root() -> PathBuf {
    let cwd = PathBuf::from("assets");
    if cwd.is_dir() {
        return cwd;
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let beside = dir.join("assets");
            if beside.is_dir() {
                return beside;
            }
            let bundled = dir.join("../Resources/assets");
            if bundled.is_dir() {
                return bundled;
            }
        }
    }
    cwd
}
