/// Top-10 high-score tables (FUN_00402ec0): one per course, persisted by the
/// platform layer. The original packs them as space-separated numbers in
/// entpack.ini (section "Ski", keys "SS"/"GS"/"FS"); times are stored
/// negated so one descending sort fits both kinds — the port just sorts each
/// list in its natural order, which is equivalent.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct HighScores {
    /// Slalom ("SS") best times in ms, ascending (lower is better).
    pub slalom_ms: Vec<u64>,
    /// Giant slalom ("GS") best times in ms, ascending.
    pub giant_slalom_ms: Vec<u64>,
    /// Freestyle ("FS") best style scores, descending.
    pub freestyle_style: Vec<i32>,
}

pub const HIGH_SCORE_LEN: usize = 10;

impl HighScores {
    fn insert_time_into(list: &mut Vec<u64>, ms: u64) -> Option<usize> {
        let rank = list.iter().position(|&t| t >= ms).unwrap_or(list.len());
        if rank >= HIGH_SCORE_LEN {
            return None;
        }
        list.insert(rank, ms);
        list.truncate(HIGH_SCORE_LEN);
        Some(rank)
    }

    /// Record a course time; returns the 0-based rank, or None when the time
    /// doesn't make the top 10 ("try again").
    pub fn insert_time(&mut self, course: usize, ms: u64) -> Option<usize> {
        let list = if course == 0 {
            &mut self.slalom_ms
        } else {
            &mut self.giant_slalom_ms
        };
        Self::insert_time_into(list, ms)
    }

    /// Record a freestyle style score; returns the 0-based rank, or None.
    pub fn insert_style(&mut self, score: i32) -> Option<usize> {
        let rank = self
            .freestyle_style
            .iter()
            .position(|&s| s <= score)
            .unwrap_or(self.freestyle_style.len());
        if rank >= HIGH_SCORE_LEN {
            return None;
        }
        self.freestyle_style.insert(rank, score);
        self.freestyle_style.truncate(HIGH_SCORE_LEN);
        Some(rank)
    }
}

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

    /// "%2u:%2.2u:%2.2u.%2.2u" — h:mm:ss.cc (FUN_00401d70). Each part is
    /// computed with signed, truncating division and masked to 16 bits
    /// before printing, exactly like the original (a negative clock prints
    /// as large unsigned parts rather than being clamped).
    pub fn format_time(&self, ms: i64) -> String {
        format_clock(ms as i32)
    }

    /// "%5.2dm" — meters, at least 2 digits, field width 5 (FUN_00401b80).
    pub fn format_dist(&self, meters: i32) -> String {
        format!("{}m", c_int(meters, 5, 2))
    }

    /// "%5.2dm/s" (FUN_00401b80).
    pub fn format_speed(&self, speed: i32) -> String {
        format!("{}m/s", c_int(speed, 5, 2))
    }

    /// "%7ld" (FUN_00401b80).
    pub fn format_style(&self) -> String {
        format!("{:>7}", self.score)
    }
}

#[cfg(test)]
mod high_score_tests {
    use super::*;

    #[test]
    fn times_sort_ascending_and_truncate() {
        let mut hs = HighScores::default();
        for ms in [5000, 3000, 9000, 1000] {
            hs.insert_time(0, ms);
        }
        assert_eq!(hs.slalom_ms, vec![1000, 3000, 5000, 9000]);
    }

    #[test]
    fn style_sorts_descending() {
        let mut hs = HighScores::default();
        for s in [10, 100, 50] {
            hs.insert_style(s);
        }
        assert_eq!(hs.freestyle_style, vec![100, 50, 10]);
    }

    #[test]
    fn rank_is_returned_for_placed_entries() {
        let mut hs = HighScores::default();
        assert_eq!(hs.insert_time(0, 5000), Some(0));
        assert_eq!(hs.insert_time(0, 3000), Some(0));
        assert_eq!(hs.insert_time(0, 4000), Some(1));
    }

    #[test]
    fn eleventh_best_does_not_place() {
        let mut hs = HighScores::default();
        for i in 0..10 {
            hs.insert_time(1, (i + 1) * 1000);
        }
        assert_eq!(hs.insert_time(1, 99999), None);
        assert_eq!(hs.insert_time(1, 500), Some(0));
        assert_eq!(hs.giant_slalom_ms.len(), 10);
    }

    #[test]
    fn style_and_time_lists_are_independent() {
        let mut hs = HighScores::default();
        hs.insert_time(0, 5000);
        hs.insert_time(1, 6000);
        hs.insert_style(-22);
        assert_eq!(hs.slalom_ms.len(), 1);
        assert_eq!(hs.giant_slalom_ms.len(), 1);
        assert_eq!(hs.freestyle_style, vec![-22]);
    }
}

/// FUN_00401d70's clock string.
pub fn format_clock(ms: i32) -> String {
    let part = |v: i32| v as u32 & 0xffff;
    let hundredths = part(ms % 1000) / 10;
    let secs = ms / 1000;
    let mins = secs / 60;
    format!("{:>2}:{:02}:{:02}.{:02}", part(mins / 60), part(mins % 60), part(secs % 60), hundredths)
}

/// C's `%<width>.<prec>d`: at least `prec` digits, the sign in front, padded
/// to `width` with spaces.
pub fn c_int(v: i32, width: usize, prec: usize) -> String {
    let digits = format!("{:0prec$}", v.unsigned_abs(), prec = prec);
    let signed = if v < 0 { format!("-{}", digits) } else { digits };
    format!("{:>width$}", signed, width = width)
}

#[cfg(test)]
mod format_tests {
    use super::*;

    #[test]
    fn clock_like_fun_00401d70() {
        assert_eq!(format_clock(0), " 0:00:00.00");
        assert_eq!(format_clock(83_456), " 0:01:23.45");
        assert_eq!(format_clock(36_000_000), "10:00:00.00");
        // Negative: signed parts masked to 16 bits, printed unsigned.
        assert_eq!(format_clock(-1_500), " 0:00:65535.6503");
    }

    #[test]
    fn c_int_precision_and_sign() {
        assert_eq!(c_int(0, 5, 2), "   00");
        assert_eq!(c_int(169, 5, 2), "  169");
        assert_eq!(c_int(-5, 5, 2), "  -05");
        assert_eq!(c_int(-123, 5, 2), " -123");
    }
}
