use super::types::*;

pub const SCROLL_STEP: i16 = 0x3c; // 60 units = 3.75 m

impl Camera {
    /// Center camera on the skier, applying discrete scroll steps
    /// Mirrors the scroll logic in FUN_00401e50
    pub fn follow(&mut self, target_x: i16, target_y: i16) {
        self.scroll_x = self.scroll_x.wrapping_sub(target_x.wrapping_sub(self.cx));
        self.scroll_y = self.scroll_y.wrapping_sub(target_y.wrapping_sub(self.cy));
        self.cx = target_x;
        self.cy = target_y;

        while self.scroll_y > SCROLL_STEP {
            self.scroll_y -= SCROLL_STEP;
        }
        while self.scroll_y < -SCROLL_STEP {
            self.scroll_y += SCROLL_STEP;
        }
        while self.scroll_x > SCROLL_STEP {
            self.scroll_x -= SCROLL_STEP;
        }
        while self.scroll_x < -SCROLL_STEP {
            self.scroll_x += SCROLL_STEP;
        }
    }

    /// Convert world coordinates to screen coordinates
    pub fn world_to_screen(&self, wx: i16, wy: i16, screen_w: i16, screen_h: i16) -> (i16, i16) {
        let sx = wx.wrapping_sub(self.cx).wrapping_add(screen_w / 2);
        let sy = wy.wrapping_sub(self.cy).wrapping_add(screen_h / 2);
        (sx, sy)
    }

    /// Check if a world-space AABB intersects the viewport
    pub fn is_visible(&self, x: i16, y: i16, w: i16, h: i16, screen_w: i16, screen_h: i16) -> bool {
        let (sx, sy) = self.world_to_screen(x, y, screen_w, screen_h);
        sx + w > 0 && sx < screen_w && sy + h > 0 && sy < screen_h
    }
}
