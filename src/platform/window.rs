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
        icon: window_icon(),
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

/// The window/taskbar icon: the original registers its window class with
/// `LoadIconA(hInstance, "iconSki")` (FUN_004052d0), the ICONSKI group —
/// the goggled skier's face, 32x32 only. Windows shows its 16-colour image
/// (icon 3) and shrinks it for the title bar itself; `small` reproduces that
/// exactly (colour from every odd pixel, opaque if any pixel of the 2x2
/// block is — matched pixel for pixel against the original's title bar in
/// the VM). `big` is the 32x32 doubled.
///
/// On macOS miniquad uses the icon only as the Dock image: inside the .app
/// bundle we leave it unset so the bundle's own icon (tools/make_icon.py)
/// stays; `cargo run` gets the face instead of miniquad's logo.
fn window_icon() -> Option<miniquad::conf::Icon> {
    if cfg!(target_os = "macos") && in_app_bundle() {
        return None;
    }
    let path = skifree::game::assets::asset_root().join("icons/icon_3.png");
    let bytes = std::fs::read(path).ok()?;
    let img = Image::from_file_with_format(&bytes, Some(ImageFormat::Png)).ok()?;
    if img.width != 32 || img.height != 32 {
        return None;
    }
    let px = |x: usize, y: usize| &img.bytes[(y * 32 + x) * 4..(y * 32 + x) * 4 + 4];
    let mut icon = miniquad::conf::Icon {
        small: [0; 16 * 16 * 4],
        medium: [0; 32 * 32 * 4],
        big: [0; 64 * 64 * 4],
    };
    icon.medium.copy_from_slice(&img.bytes);
    for y in 0..16 {
        for x in 0..16 {
            let o = (y * 16 + x) * 4;
            icon.small[o..o + 3].copy_from_slice(&px(2 * x + 1, 2 * y + 1)[..3]);
            let opaque = (0..2).any(|dy| (0..2).any(|dx| px(2 * x + dx, 2 * y + dy)[3] > 0));
            icon.small[o + 3] = if opaque { 255 } else { 0 };
        }
    }
    for y in 0..64 {
        for x in 0..64 {
            let o = (y * 64 + x) * 4;
            icon.big[o..o + 4].copy_from_slice(px(x / 2, y / 2));
        }
    }
    Some(icon)
}

fn in_app_bundle() -> bool {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().contains(".app/Contents/MacOS/"))
        .unwrap_or(false)
}
