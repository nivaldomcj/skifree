mod platform;

use macroquad::prelude::*;
use platform::audio::Audio;
use platform::input::{GameAction, Input};
use platform::persistence;
use platform::renderer::Renderer;
use platform::timer::GameTimer;
use platform::window;
use skifree::game::draw::{draw_list, DrawItem};
use skifree::game::scoring::HIGH_SCORES_CAPTION;
use skifree::game::state::GameState;

fn window_conf() -> Conf {
    window::conf()
}

/// The original's pause notice (string 2), shown in the window title.
const PAUSE_TEXT: &str = "Ski Paused ... Press F3 to continue";
/// Its normal title (string 1).
const TITLE_TEXT: &str = "SkiFree";

/// Frame-capture replay (`SKIFREE_REPLAY`): the real renderer plays a replay
/// scenario at its fixed tick rate, ignoring live input and the clock, and
/// saves the framebuffer after each tick in `ticks` to `dir` as raw RGBA
/// (`frame_<tick>_<w>x<h>.rgba`) for tools/vm/frames.py.
///
/// On the web: `?replay=<scenario name>&ticks=<t1,...>` loads
/// `replays/<name>.json` from the server and hands the frames to the page
/// (`window.skifreeCapture`, read by `tools/web/capture_frames.mjs`).
struct FrameCapture {
    scenario: skifree::replay::Scenario,
    #[cfg(not(target_arch = "wasm32"))]
    dir: std::path::PathBuf,
    ticks: Vec<u32>,
    tick: u32,
}

fn parse_ticks(list: &str) -> Result<Vec<u32>, String> {
    list.split(',').map(|t| t.trim().parse().map_err(|_| format!("bad tick {t:?} in {list:?}"))).collect()
}

/// `SKIFREE_REPLAY` set but the capture can't start: say why and exit,
/// rather than open a playable game that the unattended frame tools
/// (tools/vm/frames.sh) would wait on forever.
#[cfg(not(target_arch = "wasm32"))]
#[expect(clippy::unused_async, reason = "the same signature as the wasm32 version, which awaits")]
async fn load_frame_capture() -> Option<FrameCapture> {
    let path = std::env::var("SKIFREE_REPLAY").ok()?;
    match frame_capture(&path) {
        Ok(cap) => Some(cap),
        Err(e) => {
            eprintln!("SKIFREE_REPLAY: {e}");
            std::process::exit(1);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn frame_capture(path: &str) -> Result<FrameCapture, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    let scenario = serde_json::from_str(&text).map_err(|e| format!("bad scenario {path}: {e}"))?;
    let var = |name: &str| std::env::var(name).map_err(|_| format!("{name} is not set"));
    let dir = std::path::PathBuf::from(var("SKIFREE_FRAMES")?);
    let ticks = parse_ticks(&var("SKIFREE_FRAME_TICKS")?)?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    Ok(FrameCapture { scenario, dir, ticks, tick: 0 })
}

#[cfg(target_arch = "wasm32")]
async fn load_frame_capture() -> Option<FrameCapture> {
    let name = platform::web::url_param("replay")?;
    match web_frame_capture(&name).await {
        Ok(cap) => Some(cap),
        Err(e) => {
            // tools/web/capture_frames.mjs reads it and fails at once.
            platform::web::capture_error(&e);
            None
        }
    }
}

#[cfg(target_arch = "wasm32")]
async fn web_frame_capture(name: &str) -> Result<FrameCapture, String> {
    let ticks = parse_ticks(&platform::web::url_param("ticks").ok_or("no ticks in the address")?)?;
    let path = format!("replays/{name}.json");
    let text = load_string(&path).await.map_err(|e| format!("cannot load {path}: {e:?}"))?;
    let scenario = serde_json::from_str(&text).map_err(|e| format!("bad scenario {path}: {e}"))?;
    Ok(FrameCapture { scenario, ticks, tick: 0 })
}

/// Hand one captured frame (RGBA rows top-down) to the capture's sink.
fn save_frame(cap: &FrameCapture, width: u32, height: u32, rgba: &[u8]) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = cap.dir.join(format!("frame_{}_{}x{}.rgba", cap.tick, width, height));
        if let Err(e) = std::fs::write(&path, rgba) {
            eprintln!("SKIFREE_FRAMES: cannot write {}: {e}", path.display());
            std::process::exit(1);
        }
    }
    #[cfg(target_arch = "wasm32")]
    platform::web::capture_frame(cap.tick, width, height, rgba);
}

/// Paint one frame of `game` at its client size (sw x sh): sprites, the
/// pause notice, the status display and the high-score table.
fn draw_frame(game: &GameState, renderer: &Renderer, items: &[DrawItem], pause_in_window: bool) {
    let (sw, sh) = (f32::from(game.screen_w), f32::from(game.screen_h));
    // Render
    clear_background(WHITE);

    // Sprites in the original's painting order and placement
    // (FUN_00401540 / FUN_004014b0, see game::draw).
    for item in items {
        renderer.draw_sprite(u32::from(item.sprite), f32::from(item.left), f32::from(item.top));
    }

    // Pause notice: the original shows it only in the window title. Drawn
    // in the window (status-display style) when no title bar can show it:
    // fullscreen, or a platform where the title can't be set.
    if game.paused && pause_in_window {
        renderer.draw_notice(PAUSE_TEXT, sw, sh);
    }

    // Status display (child window SkiStatus, FUN_00406970/FUN_00401b80):
    // the values as last painted, in the OEM fixed font, see
    // renderer::draw_status.
    renderer.draw_status(&game.status_values, sw);
}

/// `ski32.exe nosound` (the whole command line, any case) disables the
/// original's sound (`FUN_004047e0`); here it starts the game muted. On the
/// web: `?nosound` in the page URL.
fn nosound() -> bool {
    #[cfg(target_arch = "wasm32")]
    return platform::web::url_param("nosound").is_some();
    #[cfg(not(target_arch = "wasm32"))]
    std::env::args().skip(1).collect::<Vec<_>>().join(" ").eq_ignore_ascii_case("nosound")
}

/// Milliseconds since the Unix epoch, truncated (the clock `GameState`
/// seeds new games with).
fn clock_ms() -> u32 {
    (macroquad::miniquad::date::now() * 1000.0) as u64 as u32
}

#[macroquad::main(window_conf)]
async fn main() {
    run().await;
}

// Not on `main`: #[macroquad::main] drops the attributes after it.
#[expect(clippy::too_many_lines, reason = "the frame loop, top to bottom")]
async fn run() {
    skifree::game::state::set_clock_source(clock_ms);
    let assets = platform::assets::load().await;
    #[cfg(target_arch = "wasm32")]
    window::set_favicon(&assets);
    let renderer = Renderer::new(&assets);
    let sprites = assets.sprite_sizes();
    // Where the platform can, stop the user's resize at the minimum, like
    // the original's WM_GETMINMAXINFO; elsewhere enforce_min_size bounces.
    platform::os_window::set_min_client_size(f64::from(window::MIN_WIDTH), f64::from(window::MIN_HEIGHT));
    let mut timer = GameTimer::new();
    let mut input = Input::new();
    let mut audio = Audio::new(nosound(), &assets);
    drop(assets);

    // SKIFREE_SEED=<u32> forces a deterministic world for side-by-side
    // testing against the original running a patched seed.
    let (w, h) = skifree::game::DEFAULT_CLIENT_SIZE;
    let mut game = match std::env::var("SKIFREE_SEED").ok().and_then(|s| s.parse::<u32>().ok()) {
        Some(seed) => GameState::new_seeded(w, h, seed, sprites.clone()),
        None => GameState::new(w, h, sprites.clone()),
    };
    game.new_game();
    game.high_scores = persistence::load_scores();

    let mut capture = load_frame_capture().await;
    if let Some(cap) = &capture {
        let (w, h) = cap.scenario.client_size();
        // On macOS miniquad's request_new_screen_size sets the window frame,
        // title bar included, so the client would come out short (and
        // screen_width/height report the requested size at once): set the
        // client size directly there. A web page can't resize its canvas:
        // frames are drawn offscreen at the scenario's size anyway.
        if !platform::os_window::set_client_size(f64::from(w), f64::from(h)) {
            request_new_screen_size(f32::from(w), f32::from(h));
        }
        #[cfg(not(target_arch = "wasm32"))]
        for _ in 0..120 {
            next_frame().await;
            if screen_width() as i16 == w && screen_height() as i16 == h {
                break;
            }
        }
        game = GameState::new_seeded(w, h, cap.scenario.seed, sprites);
        game.new_game();
    }

    let mut running = true;
    // Pause notice: the title follows `paused` (FUN_00405760's
    // SetWindowTextA); `title_set` records whether the platform could set it.
    let mut shown_paused = false;
    let mut timer_was_running = game.timer_running;
    let mut title_set = false;
    // High-score message box (FUN_00402ec0's MessageBoxA): `dialog_due`
    // holds its text for the frame after the finish (so the finish frame is
    // on screen behind it); `dialog_in_window` is the drawn fallback while
    // it is open, with its OK button's rectangle.
    let mut dialog_due: Option<String> = None;
    let mut dialog_in_window: Option<(String, (f32, f32, f32, f32))> = None;
    // The sprites of the frame on screen: the original shows the box before
    // the finish tick's repaint, so the previous frame stays behind it.
    let mut shown_items: Vec<DrawItem> = Vec::new();

    while running {
        if let Some(text) = dialog_due.take() {
            // The box takes the focus: the window goes inactive (timer
            // stopped, input dropped) until OK, like the original.
            game.set_active(false);
            let opened = platform::timer::now_secs();
            if platform::os_window::message_box(HIGH_SCORES_CAPTION, &text) {
                game.add_wall_time(((platform::timer::now_secs() - opened) * 1000.0) as u32);
                game.set_active(true);
                // The box's own measure covers the gap: don't count it twice.
                let _ = timer.reset();
            } else {
                dialog_in_window = Some((text, (0.0, 0.0, 0.0, 0.0)));
            }
        }
        window::enforce_min_size();

        // Track the live window size (the original recomputes its client
        // rect on every WM_SIZE). The status display is a small overlay at
        // the top-right, so the game viewport is the full window.
        let view_w = screen_width().max(window::MIN_WIDTH as f32) as i16;
        let view_h = screen_height().max(window::MIN_HEIGHT as f32) as i16;
        if capture.is_none() {
            game.resize(view_w, view_h);
        }

        let live_input = if capture.is_some() { Vec::new() } else { input.poll() };
        // The frame's input belongs to the box while it is open, including
        // the key or click that closes it: Esc must not quit the game, nor
        // a click on OK make the skier jump (in the original they go to the
        // MessageBoxA, not to the game window).
        let box_was_open = dialog_in_window.is_some();
        if let Some((_, (bx, by, bw, bh))) = &dialog_in_window {
            // The drawn box is modal: only OK (Enter, Space, Esc, or a click
            // on the button) closes it; everything else is dropped.
            let (mx, my) = platform::input::client_mouse_position();
            let clicked =
                is_mouse_button_pressed(MouseButton::Left) && mx >= *bx && mx < bx + bw && my >= *by && my < by + bh;
            let keyed =
                [KeyCode::Enter, KeyCode::KpEnter, KeyCode::Space, KeyCode::Escape].iter().any(|&k| is_key_pressed(k));
            if live_input.iter().any(|a| matches!(a, GameAction::Quit)) && !keyed {
                running = false;
            }
            if clicked || keyed {
                dialog_in_window = None;
                game.set_active(true);
            }
        }
        let live_input = if box_was_open { Vec::new() } else { live_input };
        for action in live_input {
            // While the window is inactive or minimized the original's window
            // procedure drops every key and mouse message (DAT_0040c67c).
            if !game.input_enabled && !matches!(action, GameAction::Focus(_)) {
                continue;
            }
            match action {
                GameAction::Focus(active) => game.set_active(active),
                GameAction::Quit => running = false,
                GameAction::PauseToggle => {
                    game.toggle_pause();
                }
                GameAction::Restart => {
                    game.new_game();
                }
                GameAction::RestartIfDead => {
                    if !game.has_player() {
                        game.new_game();
                    }
                }
                GameAction::SteerAbsolute(state) => {
                    game.steer_absolute(state);
                }
                GameAction::Jump => {
                    game.jump_keyboard();
                }
                // One state step per key-down (WM_KEYDOWN), auto-repeats
                // included.
                GameAction::SteerLeft => game.steer_keyboard(true, false, false, false),
                GameAction::SteerRight => game.steer_keyboard(false, true, false, false),
                GameAction::SteerUp => game.steer_keyboard(false, false, true, false),
                GameAction::SteerDown => game.steer_keyboard(false, false, false, true),
                GameAction::ToggleMute => audio.toggle_mute(),
                // A t step runs for the GetTickCount time since the last tick.
                GameAction::Cheat(cheat) => {
                    game.cheat(cheat, || timer.step());
                }
                GameAction::MouseSteer(mx, my) => {
                    game.mouse_move(mx as i16, my as i16);
                }
                GameAction::Click(_, _) => {
                    game.click();
                }
            }
        }

        // F3 / focus loss kill the original's timer (no WM_TIMER while
        // stopped) and resuming calls SetTimer again (FUN_00404ad0): re-arm
        // on the same edge. The wall clock catches up with the stopped time
        // there (GameState::wall_ms; the meal and the status age see it).
        if game.timer_running && !timer_was_running {
            let skipped = timer.reset();
            // A capture's ticks carry their own wall time (GameState::tick
            // adds it while stopped too), and its timer is never polled.
            if capture.is_none() {
                game.add_wall_time(skipped);
            }
        }
        timer_was_running = game.timer_running;

        if let Some(cap) = &capture {
            // A `t` cheat input steps the game itself: that is the tick.
            if cap.tick < cap.scenario.ticks && !skifree::replay::apply_inputs(&mut game, &cap.scenario, cap.tick) {
                game.tick(cap.scenario.delta_ms);
            }
        } else if let Some(delta) = game.timer_running.then(|| timer.poll()).flatten() {
            // Every 46.875 ms with the original's GetTickCount deltas (see
            // platform::timer), not the rendered frame's duration.
            game.tick(delta);
        }

        // This frame's sounds (normally one tick's): each cuts off the one
        // before, as sndPlaySoundA does. Capture runs stay silent.
        let sounds = std::mem::take(&mut game.sounds);
        if capture.is_none() {
            audio.play_tick(&sounds);
        }

        if game.paused != shown_paused {
            shown_paused = game.paused;
            let text = if game.paused { PAUSE_TEXT } else { TITLE_TEXT };
            title_set = platform::os_window::set_title(text);
        }
        let pause_in_window = game.paused && !(title_set && platform::os_window::title_visible());

        if game.pending_score_save && capture.is_none() {
            game.rank_finish_in(persistence::load_scores());
            persistence::save_scores(&game.high_scores);
            game.pending_score_save = false;
        }

        // A finish this tick: the box opens at the start of the next frame,
        // over this frame drawn with the previous sprites and the status
        // display as repainted at the finish. No box in frame-capture runs
        // (like tools/vm/no_dialogs.js) unless SKIFREE_REPLAY_DIALOGS is set,
        // to check the box itself.
        if game.show_high_scores {
            game.show_high_scores = false;
            if capture.is_none() || std::env::var_os("SKIFREE_REPLAY_DIALOGS").is_some() {
                dialog_due = game.high_score_text();
            }
        }
        if dialog_due.is_none() && dialog_in_window.is_none() {
            shown_items = draw_list(&game);
        }
        draw_frame(&game, &renderer, &shown_items, pause_in_window);
        if let Some((text, button)) = &mut dialog_in_window {
            *button = renderer.draw_message_box(
                HIGH_SCORES_CAPTION,
                text,
                f32::from(game.screen_w),
                f32::from(game.screen_h),
            );
        }

        if let Some(cap) = &mut capture {
            if cap.ticks.contains(&cap.tick) {
                // Same drawing into an offscreen target of the exact client
                // size (the window itself may be clamped by the screen).
                let target = render_target(game.screen_w as u32, game.screen_h as u32);
                target.texture.set_filter(FilterMode::Nearest);
                let mut cam = Camera2D::from_display_rect(Rect::new(
                    0.0,
                    0.0,
                    f32::from(game.screen_w),
                    f32::from(game.screen_h),
                ));
                cam.render_target = Some(target.clone());
                set_camera(&cam);
                draw_frame(&game, &renderer, &shown_items, pause_in_window);
                set_default_camera();
                let img = target.texture.get_texture_data();
                // The render target's rows come out bottom-up: save top-down.
                let row = img.width as usize * 4;
                let top_down: Vec<u8> = img.bytes.chunks(row).rev().flatten().copied().collect();
                save_frame(cap, u32::from(img.width), u32::from(img.height), &top_down);
            }
            cap.tick += 1;
            if cap.ticks.iter().all(|&t| t < cap.tick) {
                #[cfg(target_arch = "wasm32")]
                platform::web::capture_done();
                running = false;
            }
        }

        next_frame().await;
    }
}

#[cfg(test)]
mod tests {
    use super::parse_ticks;

    #[test]
    fn frame_ticks_parse_strictly() {
        assert_eq!(parse_ticks("1, 100,300"), Ok(vec![1, 100, 300]));
        for bad in ["100;200", "100,2OO", ""] {
            assert!(parse_ticks(bad).is_err(), "{bad:?} accepted");
        }
    }
}
