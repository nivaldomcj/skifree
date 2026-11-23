mod platform;

use skifree::game::draw::{draw_list, DrawItem};
use skifree::game::scoring::HIGH_SCORES_CAPTION;
use skifree::game::state::GameState;
use macroquad::prelude::*;
use platform::audio::Audio;
use platform::input::{GameAction, Input};
use platform::persistence;
use platform::renderer::Renderer;
use platform::timer::GameTimer;
use platform::window;

fn window_conf() -> Conf {
    window::conf()
}

#[derive(serde::Deserialize)]
struct ScriptEvent {
    at_ms: u64,
    key: String,
}

fn load_script() -> Option<Vec<ScriptEvent>> {
    let path = std::env::var("SKIFREE_SCRIPT").ok()?;
    let data = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

/// The original's pause notice (string 2), shown in the window title.
const PAUSE_TEXT: &str = "Ski Paused ... Press F3 to continue";
/// Its normal title (string 1).
const TITLE_TEXT: &str = "SkiFree";

/// Frame-capture replay (SKIFREE_REPLAY): the real renderer plays a replay
/// scenario at its fixed tick rate, ignoring live input and the clock, and
/// saves the framebuffer after each tick in `ticks` to `dir` as raw RGBA
/// (`frame_<tick>_<w>x<h>.rgba`) for tools/vm/frames.py.
struct FrameCapture {
    scenario: skifree::replay::Scenario,
    dir: std::path::PathBuf,
    ticks: Vec<u32>,
    tick: u32,
}

fn load_frame_capture() -> Option<FrameCapture> {
    let path = std::env::var("SKIFREE_REPLAY").ok()?;
    let scenario = serde_json::from_str(&std::fs::read_to_string(&path).ok()?).ok()?;
    let dir = std::path::PathBuf::from(std::env::var("SKIFREE_FRAMES").ok()?);
    let ticks = std::env::var("SKIFREE_FRAME_TICKS")
        .ok()?
        .split(',')
        .filter_map(|t| t.trim().parse().ok())
        .collect();
    std::fs::create_dir_all(&dir).ok()?;
    Some(FrameCapture { scenario, dir, ticks, tick: 0 })
}

/// Paint one frame of `game` at its client size (sw x sh): sprites, the
/// pause notice, the status display and the high-score table.
fn draw_frame(game: &GameState, renderer: &Renderer, items: &[DrawItem], pause_in_window: bool) {
    let (sw, sh) = (game.screen_w as f32, game.screen_h as f32);
    // Render
    clear_background(WHITE);

    // Sprites in the original's painting order and placement
    // (FUN_00401540 / FUN_004014b0, see game::draw).
    for item in items {
        renderer.draw_sprite(item.sprite as u32, item.left as f32, item.top as f32);
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

#[macroquad::main(window_conf)]
async fn main() {
    let renderer = Renderer::new().await;
    let mut timer = GameTimer::new();
    let mut input = Input::new();
    let audio = Audio::new();
    audio.load_sounds();

    // SKIFREE_SEED=<u32> forces a deterministic world for side-by-side
    // testing against the original running a patched seed.
    let mut game = match std::env::var("SKIFREE_SEED").ok().and_then(|s| s.parse::<u32>().ok()) {
        Some(seed) => GameState::new_seeded(window::DEFAULT_WIDTH as i16, window::DEFAULT_HEIGHT as i16, seed),
        None => GameState::new(window::DEFAULT_WIDTH as i16, window::DEFAULT_HEIGHT as i16),
    };
    game.new_game();
    game.high_scores = persistence::load_scores();

    let mut capture = load_frame_capture();
    if let Some(cap) = &capture {
        let (w, h) = (cap.scenario.screen_w.unwrap_or(640), cap.scenario.screen_h.unwrap_or(480));
        request_new_screen_size(w as f32, h as f32);
        for _ in 0..120 {
            next_frame().await;
            if screen_width() as i16 == w && screen_height() as i16 == h {
                break;
            }
        }
        game = GameState::new_seeded(w, h, cap.scenario.seed);
        game.course_type = cap.scenario.course.into();
        game.new_game();
    }

    // SKIFREE_SCRIPT=<json file> replays timed key events for side-by-side
    // comparison with the original (which is driven by the same schedule
    // through Frida PostMessage). Format: [{"at_ms": 2000, "key": "down"}]
    let script = load_script();
    let script_start = std::time::Instant::now();
    let mut script_fired = 0usize;

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
            let opened = std::time::Instant::now();
            if platform::os_window::message_box(HIGH_SCORES_CAPTION, &text) {
                game.add_wall_time(opened.elapsed().as_millis() as u32);
                game.set_active(true);
                timer.reset();
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
        if let Some((_, (bx, by, bw, bh))) = &dialog_in_window {
            // The drawn box is modal: only OK (Enter, Space, Esc, or a click
            // on the button) closes it; everything else is dropped.
            let (mx, my) = platform::input::client_mouse_position();
            let clicked = is_mouse_button_pressed(MouseButton::Left)
                && mx >= *bx && mx < bx + bw && my >= *by && my < by + bh;
            let keyed = [KeyCode::Enter, KeyCode::KpEnter, KeyCode::Space, KeyCode::Escape]
                .iter()
                .any(|&k| is_key_pressed(k));
            if live_input.iter().any(|a| matches!(a, GameAction::Quit)) && !keyed {
                running = false;
            }
            if clicked || keyed {
                dialog_in_window = None;
                game.set_active(true);
            }
        }
        let live_input = if dialog_in_window.is_some() { Vec::new() } else { live_input };
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
                GameAction::FastForward => {
                    game.fast_forward = !game.fast_forward;
                }
                GameAction::StepTick => {
                    // Single-step one tick even when paused
                    game.step_tick(timer.step());
                }
                GameAction::Repaint => {
                    // Force repaint — no-op in macroquad (always repaints)
                }
                GameAction::TeleportLeft => {
                    if let Some(idx) = game.player_idx {
                        if let Some(a) = game.pool.get_mut(idx) {
                            a.x = a.x.wrapping_sub(2);
                        }
                    }
                }
                GameAction::TeleportRight => {
                    if let Some(idx) = game.player_idx {
                        if let Some(a) = game.pool.get_mut(idx) {
                            a.x = a.x.wrapping_add(2);
                        }
                    }
                }
                GameAction::TeleportUp => {
                    if let Some(idx) = game.player_idx {
                        if let Some(a) = game.pool.get_mut(idx) {
                            a.y = a.y.wrapping_sub(2);
                        }
                    }
                }
                GameAction::TeleportDown => {
                    if let Some(idx) = game.player_idx {
                        if let Some(a) = game.pool.get_mut(idx) {
                            a.y = a.y.wrapping_add(2);
                        }
                    }
                }
                GameAction::MouseSteer(mx, my) => {
                    game.mouse_move(mx as i16, my as i16);
                }
                GameAction::Click(_, _) => {
                    game.click();
                }
            }
        }

        // Fire due scripted events (side-by-side testing only).
        if let Some(events) = &script {
            let now_ms = script_start.elapsed().as_millis() as u64;
            while script_fired < events.len() && events[script_fired].at_ms <= now_ms {
                if !game.input_enabled {
                    script_fired += 1;
                    continue;
                }
                match events[script_fired].key.as_str() {
                    "down" => game.steer_keyboard(false, false, false, true),
                    "up" => game.steer_keyboard(false, false, true, false),
                    "left" => game.steer_keyboard(true, false, false, false),
                    "right" => game.steer_keyboard(false, true, false, false),
                    "jump" => game.jump_keyboard(),
                    "click" => game.click(),
                    _ => {}
                }
                script_fired += 1;
            }
        }


        // Ticks keep arriving while paused: the game state stays frozen but
        // the wall clock advances (see GameState::wall_ms).
        // F3 / focus loss kill the original's timer and resuming calls
        // SetTimer again (FUN_00404ad0): re-arm on the same edge.
        if game.timer_running && !timer_was_running {
            timer.reset();
        }
        timer_was_running = game.timer_running;

        if let Some(cap) = &capture {
            if cap.tick < cap.scenario.ticks {
                skifree::replay::apply_inputs(&mut game, &cap.scenario, cap.tick);
                game.tick(cap.scenario.delta_ms);
            }
        } else if let Some(delta) = timer.poll() {
            // Every 46.875 ms with the original's GetTickCount deltas (see
            // platform::timer), not the rendered frame's duration.
            game.tick(delta);
        }

        if game.paused != shown_paused {
            shown_paused = game.paused;
            let text = if game.paused { PAUSE_TEXT } else { TITLE_TEXT };
            title_set = platform::os_window::set_title(text);
        }
        let pause_in_window = game.paused && !(title_set && platform::os_window::title_visible());

        if game.pending_score_save && capture.is_none() {
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
            *button = renderer.draw_message_box(HIGH_SCORES_CAPTION, text, game.screen_w as f32, game.screen_h as f32);
        }

        if let Some(cap) = &mut capture {
            if cap.ticks.contains(&cap.tick) {
                // Same drawing into an offscreen target of the exact client
                // size (the window itself may be clamped by the screen).
                let target = render_target(game.screen_w as u32, game.screen_h as u32);
                target.texture.set_filter(FilterMode::Nearest);
                let mut cam = Camera2D::from_display_rect(Rect::new(
                    0.0, 0.0, game.screen_w as f32, game.screen_h as f32,
                ));
                cam.render_target = Some(target.clone());
                set_camera(&cam);
                draw_frame(&game, &renderer, &shown_items, pause_in_window);
                set_default_camera();
                let img = target.texture.get_texture_data();
                // The render target's rows come out bottom-up: save top-down.
                let row = img.width as usize * 4;
                let top_down: Vec<u8> = img.bytes.chunks(row).rev().flatten().copied().collect();
                let name = format!("frame_{}_{}x{}.rgba", cap.tick, img.width, img.height);
                let _ = std::fs::write(cap.dir.join(name), top_down);
            }
            cap.tick += 1;
            if cap.ticks.iter().all(|&t| t < cap.tick) {
                running = false;
            }
        }

        next_frame().await;
    }
}
