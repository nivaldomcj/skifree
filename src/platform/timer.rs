use macroquad::prelude::*;

pub const TICK_RATE: f32 = 1.0 / 25.0; // 40 ms = 25 fps

pub struct GameTimer {
    accumulator: f32,
    last_frame_time: f64,
    frame_delta: f32,
}

impl GameTimer {
    pub fn new() -> Self {
        Self {
            accumulator: 0.0,
            last_frame_time: get_time(),
            frame_delta: 0.0,
        }
    }

    /// Returns true if a game tick should fire this frame.
    /// Mirrors the original SetTimer(hwnd, 0x29a, 0x28, GameTimerProc).
    pub fn should_tick(&mut self) -> bool {
        let now = get_time();
        self.frame_delta = (now - self.last_frame_time) as f32;
        self.last_frame_time = now;
        self.accumulator += self.frame_delta;

        if self.accumulator >= TICK_RATE {
            self.accumulator -= TICK_RATE;
            true
        } else {
            false
        }
    }

    /// Forget time that passed while the game loop was blocked (a modal
    /// dialog): the original restarts its timer then, it doesn't catch up.
    pub fn reset(&mut self) {
        self.accumulator = 0.0;
        self.last_frame_time = get_time();
    }

    #[allow(dead_code)] // Phase 3
    pub fn frame_delta(&self) -> f32 {
        self.frame_delta
    }
}
