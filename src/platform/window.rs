use macroquad::prelude::*;

pub const MIN_WIDTH: i32 = 320;
pub const MIN_HEIGHT: i32 = 300;
pub const DEFAULT_WIDTH: i32 = 640;
pub const DEFAULT_HEIGHT: i32 = 480;
pub const HUD_HEIGHT: f32 = 20.0;

pub fn conf() -> Conf {
    Conf {
        window_title: "SkiFree".to_owned(),
        window_width: DEFAULT_WIDTH,
        window_height: DEFAULT_HEIGHT,
        window_resizable: true,
        ..Default::default()
    }
}

pub fn enforce_min_size() {
    let w = screen_width().max(MIN_WIDTH as f32);
    let h = screen_height().max(MIN_HEIGHT as f32);
    if screen_width() < MIN_WIDTH as f32 || screen_height() < MIN_HEIGHT as f32 {
        request_new_screen_size(w, h);
    }
}
