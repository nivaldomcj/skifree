//! High-score persistence, replacing the original's entpack.ini (section
//! "Ski", keys "SS"/"GS"/"FS"). Stored as JSON: in ~/.skifree/scores.json
//! natively, in the page's localStorage (key `skifree.scores`, same JSON)
//! on the web.
//!
//! A table that can't be read is kept aside (`scores.json.unreadable`, or
//! the `skifree.scores.unreadable` key), not overwritten by the next finish;
//! natively the file is written to a temporary name and renamed into place,
//! so an interrupted write can't leave it truncated.

use skifree::game::scoring::HighScores;

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::fs;
    use std::path::PathBuf;

    fn scores_dir() -> PathBuf {
        dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".skifree")
    }

    fn scores_path() -> PathBuf {
        scores_dir().join("scores.json")
    }

    /// The table, or None when there is none yet. A file that exists but
    /// can't be read is kept aside like an unparsable one, not overwritten.
    pub fn read() -> Option<String> {
        match fs::read_to_string(scores_path()) {
            Ok(data) => Some(data),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => {
                eprintln!("high scores unreadable ({e}); kept aside, starting a new table");
                set_aside();
                None
            }
        }
    }

    pub fn write(data: &str) {
        let _ = fs::create_dir_all(scores_dir());
        let tmp = scores_dir().join("scores.json.tmp");
        if fs::write(&tmp, data).is_ok() {
            let _ = fs::rename(&tmp, scores_path());
        }
    }

    pub fn set_aside() {
        let _ = fs::rename(scores_path(), scores_dir().join("scores.json.unreadable"));
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

    pub fn set_aside() {
        if let Some(data) = web::storage_get(KEY) {
            web::storage_set("skifree.scores.unreadable", &data);
        }
    }
}

pub fn load_scores() -> HighScores {
    let Some(data) = imp::read() else { return HighScores::default() };
    serde_json::from_str(&data).unwrap_or_else(|e| {
        eprintln!("high scores unreadable ({e}); kept aside, starting a new table");
        imp::set_aside();
        HighScores::default()
    })
}

pub fn save_scores(scores: &HighScores) {
    if let Ok(data) = serde_json::to_string_pretty(scores) {
        imp::write(&data);
    }
}
