use super::types::*;

pub const SCROLL_STEP: i16 = 0x3c; // 60 units = 3.75 m

/// How many 0x3c scroll boundaries the camera crossed in each direction this
/// tick. The obstacle populator spawns one actor per crossing at the entering
/// edge (FUN_00401e50's DAT_0040c5d8/DAT_0040c714 loops).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ScrollSteps {
    pub left: u16,
    pub right: u16,
    pub up: u16,
    pub down: u16,
}

impl Camera {
    /// Move the camera center to the skier and accumulate the scroll delta,
    /// without emitting scroll steps. The original updates its camera
    /// (DAT_0040c640/DAT_0040c5f2) inside FUN_00402390 the moment the player
    /// moves, so later phases of the tick (mob nodes) see the new view; the
    /// scroll accumulators are only drained at the end of the tick.
    pub fn track(&mut self, target_x: i16, target_y: i16) {
        self.scroll_x = self.scroll_x.wrapping_sub(target_x.wrapping_sub(self.cx));
        self.scroll_y = self.scroll_y.wrapping_sub(target_y.wrapping_sub(self.cy));
        self.cx = target_x;
        self.cy = target_y;
    }

    /// Center camera on the skier, applying discrete scroll steps
    /// Mirrors the scroll logic in FUN_00401e50
    pub fn follow(&mut self, target_x: i16, target_y: i16) -> ScrollSteps {
        self.track(target_x, target_y);

        let mut steps = ScrollSteps::default();
        while self.scroll_y > SCROLL_STEP {
            self.scroll_y -= SCROLL_STEP;
            steps.up += 1;
        }
        while self.scroll_y < -SCROLL_STEP {
            self.scroll_y += SCROLL_STEP;
            steps.down += 1;
        }
        while self.scroll_x > SCROLL_STEP {
            self.scroll_x -= SCROLL_STEP;
            steps.left += 1;
        }
        while self.scroll_x < -SCROLL_STEP {
            self.scroll_x += SCROLL_STEP;
            steps.right += 1;
        }
        steps
    }

    /// Convert world coordinates to screen coordinates. The original anchors
    /// the player at (w/2, h/3) via FUN_00406060, not at the viewport center.
    pub fn world_to_screen(&self, wx: i16, wy: i16, screen_w: i16, screen_h: i16) -> (i16, i16) {
        let sx = wx.wrapping_sub(self.cx).wrapping_add(screen_w / 2);
        let sy = wy.wrapping_sub(self.cy).wrapping_add(screen_h / 3);
        (sx, sy)
    }

    /// Check if a world-space AABB intersects the viewport
    pub fn is_visible(&self, x: i16, y: i16, w: i16, h: i16, screen_w: i16, screen_h: i16) -> bool {
        let (sx, sy) = self.world_to_screen(x, y, screen_w, screen_h);
        sx + w > 0 && sx < screen_w && sy + h > 0 && sy < screen_h
    }
}
