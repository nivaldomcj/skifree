use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Score {
    pub time: String,
    pub dist: u32,
    pub speed: f32,
    pub style: String,
    pub marker: Option<String>,
}

impl Default for Score {
    fn default() -> Self {
        Self {
            time: "00:00:00.00".to_string(),
            dist: 0,
            speed: 0.0,
            style: "0000000".to_string(),
            marker: None,
        }
    }
}

fn scores_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".skifree")
}

fn scores_path() -> PathBuf {
    scores_dir().join("scores.json")
}

pub fn load_scores() -> Vec<Score> {
    let path = scores_path();
    if !path.exists() {
        return Vec::new();
    }
    match fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

pub fn save_scores(scores: &[Score]) {
    let dir = scores_dir();
    let _ = fs::create_dir_all(&dir);
    if let Ok(data) = serde_json::to_string_pretty(scores) {
        let _ = fs::write(scores_path(), data);
    }
}

#[allow(dead_code)] // Phase 3
pub fn is_high_score(score: &Score, existing: &[Score]) -> bool {
    if existing.is_empty() {
        return true;
    }
    score.dist > existing.iter().map(|s| s.dist).max().unwrap_or(0)
}
