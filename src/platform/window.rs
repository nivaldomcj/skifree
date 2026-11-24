use macroquad::prelude::*;

/// Smallest client area. The original limits its *window* to 320x300
/// (`WM_GETMINMAXINFO`, ptMinTrackSize), frame included; with Windows 10/11
/// frames that leaves a 304x261 client (measured in the VM), which the port
/// uses on every platform.
pub const MIN_WIDTH: i32 = 304;
pub const MIN_HEIGHT: i32 = 261;

pub fn conf() -> Conf {
    // Frame-capture replays (SKIFREE_REPLAY) open at the scenario's client
    // size: growing the window later can be clamped by the screen.
    let (width, height) = std::env::var("SKIFREE_REPLAY")
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str::<skifree::replay::Scenario>(&t).ok())
        .map_or(skifree::game::DEFAULT_CLIENT_SIZE, |s| s.client_size());
    Conf {
        window_title: "SkiFree".to_owned(),
        window_width: i32::from(width),
        window_height: i32::from(height),
        icon: window_icon(),
        window_resizable: true,
        platform: miniquad::conf::Platform {
            linux_backend: linux_backend(),
            // X11 WM_CLASS and Wayland app id: matches skifree.desktop
            // (tools/install_linux_desktop.sh) for the icon, and lets
            // platform::os_window find the window on X11.
            linux_wm_class: "skifree",
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Linux window system, chosen with `SKIFREE_LINUX_BACKEND`:
///
/// - `x11` (default, miniquad's own default): X11 only — under a Wayland
///   session through XWayland;
/// - `wayland`: native Wayland only;
/// - `wayland-first`: Wayland, falling back to X11 when it's unavailable;
/// - `x11-first`: X11, falling back to Wayland.
///
/// Ignored on other platforms. An unknown value keeps the default.
fn linux_backend() -> miniquad::conf::LinuxBackend {
    use miniquad::conf::LinuxBackend;
    match std::env::var("SKIFREE_LINUX_BACKEND").as_deref() {
        Err(_) | Ok("" | "x11") => LinuxBackend::X11Only,
        Ok("wayland") => LinuxBackend::WaylandOnly,
        Ok("wayland-first") => LinuxBackend::WaylandWithX11Fallback,
        Ok("x11-first") => LinuxBackend::X11WithWaylandFallback,
        Ok(other) => {
            eprintln!(
                "SKIFREE_LINUX_BACKEND={other:?} not recognised \
                 (x11, wayland, wayland-first, x11-first); using x11"
            );
            LinuxBackend::X11Only
        }
    }
}

pub fn enforce_min_size() {
    let w = screen_width().max(MIN_WIDTH as f32);
    let h = screen_height().max(MIN_HEIGHT as f32);
    // Set the client area itself where the platform can: on macOS
    // request_new_screen_size sets the frame, title bar included.
    if (screen_width() < MIN_WIDTH as f32 || screen_height() < MIN_HEIGHT as f32)
        && !super::os_window::set_client_size(f64::from(w), f64::from(h))
    {
        request_new_screen_size(w, h);
    }
}

/// The window/taskbar icon: the original registers its window class with
/// `LoadIconA(hInstance, "iconSki")` (`FUN_004052d0`), the ICONSKI group —
/// the goggled skier's face, 32x32 only. Windows shows its 16-colour image
/// (icon 3) and shrinks it for the title bar itself; `small` reproduces that
/// exactly (colour from every odd pixel, opaque if any pixel of the 2x2
/// block is — matched pixel for pixel against the original's title bar in
/// the VM). `big` is the 32x32 doubled.
///
/// On macOS miniquad uses the icon only as the Dock image: inside the .app
/// bundle we leave it unset so the bundle's own icon (`tools/make_icon.py`)
/// stays; `cargo run` gets the face instead of miniquad's logo.
fn window_icon() -> Option<miniquad::conf::Icon> {
    if cfg!(target_os = "macos") && in_app_bundle() {
        return None;
    }
    let path = skifree::assets::asset_root().join("icons/icon_3.png");
    let bytes = std::fs::read(path).ok()?;
    let img = Image::from_file_with_format(&bytes, Some(ImageFormat::Png)).ok()?;
    if img.width != 32 || img.height != 32 {
        return None;
    }
    let px = |x: usize, y: usize| &img.bytes[(y * 32 + x) * 4..(y * 32 + x) * 4 + 4];
    let mut icon =
        miniquad::conf::Icon { small: icon_small(&img.bytes), medium: [0; 32 * 32 * 4], big: [0; 64 * 64 * 4] };
    icon.medium.copy_from_slice(&img.bytes);
    for y in 0..64 {
        for x in 0..64 {
            let o = (y * 64 + x) * 4;
            icon.big[o..o + 4].copy_from_slice(px(x / 2, y / 2));
        }
    }
    Some(icon)
}

/// The 16x16 title-bar image Windows makes from the 32x32 icon: colour
/// from every odd pixel, opaque if any pixel of the 2x2 block is.
pub fn icon_small(rgba32: &[u8]) -> [u8; 16 * 16 * 4] {
    let px = |x: usize, y: usize| &rgba32[(y * 32 + x) * 4..(y * 32 + x) * 4 + 4];
    let mut small = [0; 16 * 16 * 4];
    for y in 0..16 {
        for x in 0..16 {
            let o = (y * 16 + x) * 4;
            small[o..o + 3].copy_from_slice(&px(2 * x + 1, 2 * y + 1)[..3]);
            let opaque = (0..2).any(|dy| (0..2).any(|dx| px(2 * x + dx, 2 * y + dy)[3] > 0));
            small[o + 3] = if opaque { 255 } else { 0 };
        }
    }
    small
}

/// Web: the page's favicon from the player's ICONSKI face, 16x16 as
/// `icon_small` makes it and 32x32 as is.
#[cfg(target_arch = "wasm32")]
pub fn set_favicon(assets: &skifree::extract::AssetSet) {
    use crate::platform::web;
    if let Some(icon) = assets.icon.as_ref().filter(|i| i.width == 32 && i.height == 32) {
        web::set_favicon(16, &icon_small(&icon.data));
        web::set_favicon(32, &icon.data);
    }
}

fn in_app_bundle() -> bool {
    std::env::current_exe().is_ok_and(|p| p.to_string_lossy().contains(".app/Contents/MacOS/"))
}
