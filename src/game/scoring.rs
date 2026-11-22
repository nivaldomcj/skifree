/// Scoring system (FUN_00403420)
pub struct Scoring {
    pub score: i32,
    pub enabled: bool, // mirrors DAT_0040c954
}

impl Scoring {
    pub fn new() -> Self {
        Self {
            score: 0,
            enabled: false,
        }
    }

    pub fn add(&mut self, points: i32) {
        if self.enabled {
            self.score += points;
        }
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// "%2u:%2.2u:%2.2u.%2.2u" — h:mm:ss.cc (FUN_00401d70).
    pub fn format_time(&self, elapsed_ms: u64) -> String {
        let total_secs = elapsed_ms / 1000;
        let hours = total_secs / 3600;
        let mins = (total_secs / 60) % 60;
        let secs = total_secs % 60;
        let hundredths = (elapsed_ms % 1000) / 10;
        format!("{}:{:02}:{:02}.{:02}", hours, mins, secs, hundredths)
    }

    /// "%5.2dm" — meters, at least 2 digits, field width 5 (FUN_00401b80).
    pub fn format_dist(&self, meters: i32) -> String {
        format!("{:>5}m", format!("{:02}", meters))
    }

    /// "%5.2dm/s" (FUN_00401b80).
    pub fn format_speed(&self, speed: i32) -> String {
        format!("{:>5}m/s", format!("{:02}", speed))
    }

    /// "%7ld" (FUN_00401b80).
    pub fn format_style(&self) -> String {
        format!("{:>7}", self.score)
    }
}
