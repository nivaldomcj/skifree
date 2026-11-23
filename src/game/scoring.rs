use super::course::Course;

/// Top-10 high-score tables (`FUN_00402ec0`): one per course, persisted by the
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
/// Caption of the high-score message box (string 15).
pub const HIGH_SCORES_CAPTION: &str = "High Scores";
/// String 16.
pub const THATS_YOU: &str = " <-- that's you!";
/// String 17.
pub const TRY_AGAIN: &str = " <-- try again!";

impl HighScores {
    /// The message box text `FUN_00402ec0` shows after a finish (caption
    /// "High Scores"): one line per entry — a clock string (`FUN_00401d70`)
    /// for the timed courses, `%9ld` for freestyle — with " <-- that's
    /// you!" on the new entry; when it didn't place, a blank line and the
    /// result with " <-- try again!".
    pub fn finish_text(&self, course: Course, ms: u64, style: i32, rank: Option<usize>) -> String {
        let fmt_time = |ms: u64| format_clock(ms as i32);
        let fmt_style = |s: i32| format!("{s:>9}");
        let lines: Vec<String> = match course {
            Course::Slalom => self.slalom_ms.iter().map(|&t| fmt_time(t)).collect(),
            Course::TreeSlalom => self.giant_slalom_ms.iter().map(|&t| fmt_time(t)).collect(),
            Course::Freestyle => self.freestyle_style.iter().map(|&s| fmt_style(s)).collect(),
        };
        let mut text = String::new();
        for (i, line) in lines.iter().enumerate() {
            if i > 0 {
                text.push('\n');
            }
            text.push_str(line);
            if rank == Some(i) {
                text.push_str(THATS_YOU);
            }
        }
        if rank.is_none() {
            text.push_str("\n\n");
            text.push_str(&if course.is_timed() { fmt_time(ms) } else { fmt_style(style) });
            text.push_str(TRY_AGAIN);
        }
        text
    }

    fn insert_time_into(list: &mut Vec<u64>, ms: u64) -> Option<usize> {
        // FUN_00402ec0 inserts before the first entry the new result is
        // strictly better than, so a tie ranks after the existing score.
        let rank = list.iter().position(|&t| t > ms).unwrap_or(list.len());
        if rank >= HIGH_SCORE_LEN {
            return None;
        }
        list.insert(rank, ms);
        list.truncate(HIGH_SCORE_LEN);
        Some(rank)
    }

    /// Record a timed course's time; returns the 0-based rank, or None when
    /// the time doesn't make the top 10 ("try again"). Freestyle has no
    /// time list: see `insert_style`.
    pub fn insert_time(&mut self, course: Course, ms: u64) -> Option<usize> {
        let list = match course {
            Course::Slalom => &mut self.slalom_ms,
            Course::TreeSlalom => &mut self.giant_slalom_ms,
            Course::Freestyle => return None,
        };
        Self::insert_time_into(list, ms)
    }

    /// Record a freestyle style score; returns the 0-based rank, or None.
    pub fn insert_style(&mut self, score: i32) -> Option<usize> {
        let rank = self.freestyle_style.iter().position(|&s| s < score).unwrap_or(self.freestyle_style.len());
        if rank >= HIGH_SCORE_LEN {
            return None;
        }
        self.freestyle_style.insert(rank, score);
        self.freestyle_style.truncate(HIGH_SCORE_LEN);
        Some(rank)
    }
}

/// Scoring system (`FUN_00403420`)
pub struct Scoring {
    pub score: i32,
    pub enabled: bool, // mirrors DAT_0040c954
}

impl Default for Scoring {
    fn default() -> Self {
        Self::new()
    }
}

impl Scoring {
    pub fn new() -> Self {
        Self { score: 0, enabled: false }
    }

    pub fn add(&mut self, points: i32) {
        if self.enabled {
            self.score += points;
        }
    }

    /// "%2u:%2.2u:%2.2u.%2.2u" — h:mm:ss.cc (`FUN_00401d70`). Each part is
    /// computed with signed, truncating division and masked to 16 bits
    /// before printing, exactly like the original (a negative clock prints
    /// as large unsigned parts rather than being clamped).
    pub fn format_time(&self, ms: i64) -> String {
        format_clock(ms as i32)
    }

    /// "%5.2dm" — meters, at least 2 digits, field width 5 (`FUN_00401b80`).
    pub fn format_dist(&self, meters: i32) -> String {
        format!("{}m", c_int(meters, 5, 2))
    }

    /// "%5.2dm/s" (`FUN_00401b80`).
    pub fn format_speed(&self, speed: i32) -> String {
        format!("{}m/s", c_int(speed, 5, 2))
    }

    /// "%7ld" (`FUN_00401b80`).
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
            hs.insert_time(Course::Slalom, ms);
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
        assert_eq!(hs.insert_time(Course::Slalom, 5000), Some(0));
        assert_eq!(hs.insert_time(Course::Slalom, 3000), Some(0));
        assert_eq!(hs.insert_time(Course::Slalom, 4000), Some(1));
    }

    #[test]
    fn eleventh_best_does_not_place() {
        let mut hs = HighScores::default();
        for i in 0..10 {
            hs.insert_time(Course::TreeSlalom, (i + 1) * 1000);
        }
        assert_eq!(hs.insert_time(Course::TreeSlalom, 99999), None);
        assert_eq!(hs.insert_time(Course::TreeSlalom, 500), Some(0));
        assert_eq!(hs.giant_slalom_ms.len(), 10);
    }

    #[test]
    fn style_and_time_lists_are_independent() {
        let mut hs = HighScores::default();
        hs.insert_time(Course::Slalom, 5000);
        hs.insert_time(Course::TreeSlalom, 6000);
        hs.insert_style(-22);
        assert_eq!(hs.slalom_ms.len(), 1);
        assert_eq!(hs.giant_slalom_ms.len(), 1);
        assert_eq!(hs.freestyle_style, vec![-22]);
    }
}

/// `FUN_00401d70`'s clock string.
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
    let signed = if v < 0 { format!("-{digits}") } else { digits };
    format!("{signed:>width$}")
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
    fn finish_text_like_fun_00402ec0() {
        let mut hs = HighScores::default();
        for _ in 0..6 {
            hs.insert_style(209);
        }
        let rank = hs.insert_style(209);
        assert_eq!(rank, Some(6), "a tie ranks after the existing scores");
        let text = hs.finish_text(Course::Freestyle, 0, 209, rank);
        // As captured from the original's MessageBoxA in the VM.
        assert_eq!(text, "      209\n      209\n      209\n      209\n      209\n      209\n      209 <-- that's you!");
        for _ in 0..3 {
            hs.insert_style(209);
        }
        assert_eq!(hs.insert_style(209), None, "a full list keeps the earlier ties");
        assert!(hs.finish_text(Course::Freestyle, 0, 209, None).ends_with("      209\n\n      209 <-- try again!"));
        let mut t = HighScores::default();
        let r = t.insert_time(Course::Slalom, 83_456);
        assert_eq!(t.finish_text(Course::Slalom, 83_456, 0, r), " 0:01:23.45 <-- that's you!");
    }

    #[test]
    fn c_int_precision_and_sign() {
        assert_eq!(c_int(0, 5, 2), "   00");
        assert_eq!(c_int(169, 5, 2), "  169");
        assert_eq!(c_int(-5, 5, 2), "  -05");
        assert_eq!(c_int(-123, 5, 2), " -123");
    }
}
