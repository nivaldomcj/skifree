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

    pub fn format_time(&self, elapsed_ms: u32) -> String {
        let total_secs = elapsed_ms / 1000;
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        let hundredths = (elapsed_ms % 1000) / 10;
        format!("{:02}:{:02}:{:02}.{:02}", mins, secs, hundredths / 10, hundredths % 10)
    }

    pub fn format_dist(&self, y: i16, _course_length: i16, is_freestyle: bool) -> String {
        let remaining = if is_freestyle {
            0x21c0i16.wrapping_sub(y)
        } else {
            0x4100i16.wrapping_sub(y)
        };
        let meters = remaining as i32 / 16;
        format!(" {:04}m", meters.max(0))
    }

    pub fn format_speed(&self, vx: i16, frame_delta_ms: u32) -> String {
        if frame_delta_ms == 0 {
            return " 0000m/s".to_string();
        }
        let speed = (vx as i32 * 1000) / (frame_delta_ms as i32 * 16);
        format!(" {:04}m/s", speed.max(0))
    }

    pub fn format_style(&self) -> String {
        format!("{:07}", self.score.max(0))
    }
}
