use macroquad::prelude::*;

pub const MIN_WIDTH: i32 = 320;
pub const MIN_HEIGHT: i32 = 300;
pub const DEFAULT_WIDTH: i32 = 640;
pub const DEFAULT_HEIGHT: i32 = 480;

pub fn conf() -> Conf {
    // Frame-capture replays (SKIFREE_REPLAY) open at the scenario's client
    // size: growing the window later can be clamped by the screen.
    let (width, height) = std::env::var("SKIFREE_REPLAY")
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .map(|v| {
            let get = |k: &str, d: i32| v.get(k).and_then(|x| x.as_i64()).map_or(d, |x| x as i32);
            (get("screen_w", DEFAULT_WIDTH), get("screen_h", DEFAULT_HEIGHT))
        })
        .unwrap_or((DEFAULT_WIDTH, DEFAULT_HEIGHT));
    Conf {
        window_title: "SkiFree".to_owned(),
        window_width: width,
        window_height: height,
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
