//! The original's game timer: `SetTimer(hwnd, 666, 40, GameTimerProc)`.
//!
//! Windows only checks timers on its timer interrupt, every 15.625 ms by
//! default, so a 40 ms timer fires on the first interrupt at or after 40 ms:
//! every **3 × 15.625 = 46.875 ms** (≈ 21.3 ticks/s), not 40. The tick
//! callback reads `GetTickCount`, which also advances by 15.625 ms steps
//! truncated to whole ms, so the frame delta the game sees is 47, 47, 47,
//! 47, 47, 47, 47, 46 ms (375 ms per 8 ticks). Measured in the VM
//! (docs/vm-ground-truth.md item 24); the VM's extra late ticks are the
//! VM's own and are not reproduced.
//!
//! Expiries are scheduled on the interrupt grid independently of when the
//! game looks (the port polls once per rendered frame), and expiries missed
//! while the loop was busy coalesce into one tick, like `WM_TIMER`.

use macroquad::prelude::get_time;

/// Windows' default system timer resolution.
const RESOLUTION_MS: f64 = 15.625;
/// Timer interrupts per tick: the first interrupt at or after the
/// requested 40 ms (2 × 15.625 = 31.25 is too early).
const PERIODS: u64 = 3;

pub struct GameTimer {
    origin: f64,
    /// Interrupt index of the next expiry.
    next_expiry: u64,
    /// GetTickCount at the last processed tick (`DAT_0040c698`).
    last_tick_count: u32,
}

impl GameTimer {
    pub fn new() -> Self {
        let mut timer = Self { origin: get_time(), next_expiry: 0, last_tick_count: 0 };
        timer.reset();
        timer
    }

    fn interrupt_now(&self) -> u64 {
        ((get_time() - self.origin) * 1000.0 / RESOLUTION_MS) as u64
    }

    /// GetTickCount at timer interrupt `k` (whole ms, truncated).
    fn tick_count(k: u64) -> u32 {
        (k as f64 * RESOLUTION_MS) as u32
    }

    /// Re-arm from now (the original's SetTimer after a pause or a modal
    /// dialog): time that passed meanwhile isn't caught up.
    pub fn reset(&mut self) {
        let k = self.interrupt_now();
        self.next_expiry = k + PERIODS;
        self.last_tick_count = Self::tick_count(k);
    }

    /// `Some(delta_ms)` when the timer has fired since the last call: the
    /// GetTickCount delta the original's tick would compute.
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
        let tick_count = Self::tick_count(fired);
        let delta = tick_count.wrapping_sub(self.last_tick_count);
        self.last_tick_count = tick_count;
        Some(delta)
    }

    /// The T cheat calls the tick routine directly (FUN_00406780 jumps to
    /// FUN_00401000): its delta is the GetTickCount time since the last
    /// tick, and it becomes the new reference.
    pub fn step(&mut self) -> u32 {
        let tick_count = Self::tick_count(self.interrupt_now());
        let delta = tick_count.wrapping_sub(self.last_tick_count);
        self.last_tick_count = tick_count;
        delta
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
