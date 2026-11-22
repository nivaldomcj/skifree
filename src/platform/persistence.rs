use skifree::game::scoring::HighScores;
use std::fs;
use std::path::PathBuf;

/// High-score persistence, replacing the original's entpack.ini (section
/// "Ski", keys "SS"/"GS"/"FS"). Stored as JSON in ~/.skifree/scores.json.
fn scores_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".skifree")
}

fn scores_path() -> PathBuf {
    scores_dir().join("scores.json")
}

pub fn load_scores() -> HighScores {
    let path = scores_path();
    if !path.exists() {
        return HighScores::default();
    }
    match fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => HighScores::default(),
    }
}

pub fn save_scores(scores: &HighScores) {
    let dir = scores_dir();
    let _ = fs::create_dir_all(&dir);
    if let Ok(data) = serde_json::to_string_pretty(scores) {
        let _ = fs::write(scores_path(), data);
    }
}
