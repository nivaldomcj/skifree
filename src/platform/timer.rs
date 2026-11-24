//! The original's game timer: `SetTimer(hwnd, 666, 40, GameTimerProc)`.
//!
//! Windows only checks timers on its timer interrupt, every 15.625 ms by
//! default, so a 40 ms timer fires on the first interrupt at or after 40 ms:
//! every **3 × 15.625 = 46.875 ms** (≈ 21.3 ticks/s), not 40. The tick
//! callback reads `GetTickCount`, which also advances by 15.625 ms steps
//! truncated to whole ms, so the frame delta the game sees is 46 ms, then
//! seven 47 ms, repeating (375 ms per 8 ticks). Measured in the VM
//! (docs/vm-ground-truth.md item 24); the VM's extra late ticks are the
//! VM's own and are not reproduced.
//!
//! Expiries are scheduled on the interrupt grid independently of when the
//! game looks (the port polls once per rendered frame), and expiries missed
//! while the loop was busy coalesce into one tick, like `WM_TIMER`.

/// Monotonic seconds since the first call, like `GetTickCount`: unlike the
/// wall clock (`get_time`), it never steps when the system time changes.
#[cfg(not(target_arch = "wasm32"))]
pub fn now_secs() -> f64 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64()
}

/// `performance.now()`: `Instant` panics on wasm32-unknown-unknown.
#[cfg(target_arch = "wasm32")]
pub fn now_secs() -> f64 {
    super::web::monotonic_secs()
}

/// Windows' default system timer resolution.
const RESOLUTION_MS: f64 = 15.625;
/// Timer interrupts per tick: the first interrupt at or after the
/// requested 40 ms (2 × 15.625 = 31.25 is too early).
const PERIODS: u64 = 3;

pub struct GameTimer {
    origin: f64,
    /// Interrupt index of the next expiry.
    next_expiry: u64,
    /// `GetTickCount` at the last processed tick (`DAT_0040c698`), kept in
    /// 64 bits: a 32-bit count would saturate after ~49.7 days and freeze
    /// the game (the deltas, which are what the game sees, stay 32-bit).
    last_tick_count: u64,
}

impl GameTimer {
    pub fn new() -> Self {
        let mut timer = Self { origin: now_secs(), next_expiry: 0, last_tick_count: 0 };
        timer.reset();
        timer
    }

    fn interrupt_now(&self) -> u64 {
        ((now_secs() - self.origin) * 1000.0 / RESOLUTION_MS) as u64
    }

    /// `GetTickCount` at timer interrupt `k` (whole ms, truncated).
    fn tick_count(k: u64) -> u64 {
        (k as f64 * RESOLUTION_MS) as u64
    }

    /// A tick delta as the game takes it (a `GetTickCount` difference).
    fn delta(&mut self, tick_count: u64) -> u32 {
        // Never behind the last tick: a T step can read the clock past a
        // still-pending expiry (the original's WM_TIMER then reads about
        // the same GetTickCount, so that tick's delta is 0).
        let tick_count = tick_count.max(self.last_tick_count);
        let delta = u32::try_from(tick_count - self.last_tick_count).unwrap_or(u32::MAX);
        self.last_tick_count = tick_count;
        delta
    }

    /// Re-arm from now (the original's `SetTimer` after a pause or a modal
    /// dialog): time that passed meanwhile isn't caught up.
    /// Returns the milliseconds since the last tick: the time the stopped
    /// timer skipped.
    pub fn reset(&mut self) -> u32 {
        self.reset_at(self.interrupt_now())
    }

    fn reset_at(&mut self, k: u64) -> u32 {
        self.next_expiry = k + PERIODS;
        self.delta(Self::tick_count(k))
    }

    /// `Some(delta_ms)` when the timer has fired since the last call: the
    /// `GetTickCount` delta the original's tick would compute.
    pub fn poll(&mut self) -> Option<u32> {
        self.poll_at(self.interrupt_now())
    }

    fn poll_at(&mut self, k: u64) -> Option<u32> {
        if k < self.next_expiry {
            return None;
        }
        // All expiries up to now coalesce into one tick.
        let fired = self.next_expiry + (k - self.next_expiry) / PERIODS * PERIODS;
        self.next_expiry = fired + PERIODS;
        Some(self.delta(Self::tick_count(fired)))
    }

    /// The T cheat calls the tick routine directly (`FUN_00406780` jumps to
    /// `FUN_00401000`): its delta is the `GetTickCount` time since the last
    /// tick, and it becomes the new reference.
    pub fn step(&mut self) -> u32 {
        self.step_at(self.interrupt_now())
    }

    fn step_at(&mut self, k: u64) -> u32 {
        self.delta(Self::tick_count(k))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_step_before_a_pending_expiry_never_wraps() {
        let mut t = GameTimer { origin: 0.0, next_expiry: 3, last_tick_count: 0 };
        assert_eq!(t.step_at(4), 62); // T at interrupt 4 (62 ms)
        assert_eq!(t.poll_at(4), Some(0)); // the expiry due at 3 reads ~the same time
        assert_eq!(t.poll_at(6), Some(31)); // then the cadence resumes
    }

    #[test]
    fn a_reset_reports_the_time_the_stopped_timer_skipped() {
        let mut t = at_zero();
        assert_eq!(t.poll_at(3), Some(46)); // GetTickCount 46 at interrupt 3
        assert_eq!(t.reset_at(64), 1000 - 46); // stopped until 1 s
        assert_eq!(t.poll_at(66), None);
        assert_eq!(t.poll_at(67), Some(46)); // 1046 - 1000
    }

    #[test]
    fn the_clock_runs_past_49_days() {
        // 2^32 ms of uptime: a 32-bit tick count would saturate there and
        // every later delta would be 0.
        let k = (f64::from(u32::MAX) / RESOLUTION_MS) as u64 + 100;
        let mut t = GameTimer { origin: 0.0, next_expiry: k, last_tick_count: 0 };
        t.poll_at(k);
        for n in 1..=8 {
            let d = t.poll_at(k + 3 * n).unwrap_or(0);
            assert!((46..=47).contains(&d), "delta {d} at tick {n}");
        }
    }

    fn at_zero() -> GameTimer {
        GameTimer { origin: 0.0, next_expiry: PERIODS, last_tick_count: 0 }
    }

    #[test]
    fn fires_every_three_interrupts_with_windows_deltas() {
        let mut t = at_zero();
        let mut deltas = Vec::new();
        for k in 0..=24 {
            if let Some(d) = t.poll_at(k) {
                deltas.push(d);
            }
        }
        // 46.875 ms ticks, GetTickCount truncated: 46 then 47s, 375 per 8.
        assert_eq!(deltas, vec![46, 47, 47, 47, 47, 47, 47, 47]);
        assert_eq!(deltas.iter().sum::<u32>(), 375);
    }

    #[test]
    fn missed_expiries_coalesce_into_one_tick() {
        let mut t = at_zero();
        assert_eq!(t.poll_at(10), Some(140)); // expiries 3, 6, 9 -> one tick at 9
        assert_eq!(t.poll_at(11), None);
        assert_eq!(t.poll_at(12), Some(47)); // back on the grid
    }
}
