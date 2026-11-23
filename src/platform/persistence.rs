//! High-score persistence, replacing the original's entpack.ini (section
//! "Ski", keys "SS"/"GS"/"FS"). Stored as JSON: in ~/.skifree/scores.json
//! natively, in the page's localStorage (key `skifree.scores`, same JSON)
//! on the web.

use skifree::game::scoring::HighScores;

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::fs;
    use std::path::PathBuf;

    fn scores_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".skifree")
    }

    fn scores_path() -> PathBuf {
        scores_dir().join("scores.json")
    }

    pub fn read() -> Option<String> {
        let path = scores_path();
        if !path.exists() {
            return None;
        }
        fs::read_to_string(&path).ok()
    }

    pub fn write(data: &str) {
        let _ = fs::create_dir_all(scores_dir());
        let _ = fs::write(scores_path(), data);
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use crate::platform::web;

    const KEY: &str = "skifree.scores";

    pub fn read() -> Option<String> {
        web::storage_get(KEY)
    }

    pub fn write(data: &str) {
        web::storage_set(KEY, data);
    }
}

pub fn load_scores() -> HighScores {
    imp::read()
        .and_then(|data| serde_json::from_str(&data).ok())
        .unwrap_or_default()
}

pub fn save_scores(scores: &HighScores) {
    if let Ok(data) = serde_json::to_string_pretty(scores) {
        imp::write(&data);
    }
}
