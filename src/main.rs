mod platform;

use skifree::game::draw::draw_list;
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
fn draw_frame(game: &GameState, renderer: &Renderer) {
    let (sw, sh) = (game.screen_w as f32, game.screen_h as f32);
    // Render
    clear_background(WHITE);

    // Sprites in the original's painting order and placement
    // (FUN_00401540 / FUN_004014b0, see game::draw).
    for item in draw_list(&game) {
        renderer.draw_sprite(item.sprite as u32, item.left as f32, item.top as f32);
    }

    // Pause overlay
    if game.paused {
        let text = "Ski Paused ... Press F3 to continue";
        let dims = measure_text(text, None, 24, 1.0);
        let x = (sw - dims.width) / 2.0;
        let y = sh / 2.0 + 40.0;
        draw_rectangle(x - 8.0, y - 20.0, dims.width + 16.0, 28.0, WHITE);
        draw_rectangle_lines(x - 8.0, y - 20.0, dims.width + 16.0, 28.0, 1.0, BLACK);
        draw_text(text, x, y, 24.0, BLACK);
    }

    // Status display (FUN_00401b80): stacked rows at the top-right.
    let rows = [
        ("Time:", game.scoring.format_time(game.hud_time_ms())),
        ("Dist:", game.scoring.format_dist(game.hud_dist_m())),
        ("Speed:", game.scoring.format_speed(game.hud_speed())),
        ("Style:", game.scoring.format_style()),
    ];
    let font_size = 15u16;
    let value_x = sw - 62.0;
    let label_x = value_x - 48.0;
    let mut row_y = 16.0;
    for (label, value) in rows {
        draw_text(label, label_x, row_y, font_size as f32, DARKGRAY);
        draw_text(&value, value_x, row_y, font_size as f32, BLACK);
        row_y += 16.0;
    }

    // High-score table after a course finish (FUN_00402ec0 display),
    // with " <-- that's you!" / " <-- try again!" markers.
    if let Some(finish) = game.last_finish {
        let title = match finish.course {
            0 => "Slalom High Scores",
            1 => "Giant Slalom High Scores",
            _ => "Freestyle High Scores",
        };
        let entries: Vec<String> = if finish.course == 2 {
            game.high_scores
                .freestyle_style
                .iter()
                .map(|s| format!("{:>9}", s))
                .collect()
        } else {
            let times = if finish.course == 0 {
                &game.high_scores.slalom_ms
            } else {
                &game.high_scores.giant_slalom_ms
            };
            times.iter().map(|&ms| game.scoring.format_time(ms)).collect()
        };
        let mut y = row_y + 24.0;
        draw_text(title, label_x, y, font_size as f32, DARKGRAY);
        y += 16.0;
        for (i, entry) in entries.iter().enumerate() {
            let marker = if finish.rank == Some(i) {
                " <-- that's you!"
            } else {
                ""
            };
            draw_text(&format!("{}{}", entry, marker), label_x, y, font_size as f32, BLACK);
            y += 16.0;
        }
        if finish.rank.is_none() {
            // Didn't place: show the own result below with try-again.
            y += 16.0;
            let own = if finish.course == 2 {
                format!("{:>9}", finish.style)
            } else {
                game.scoring.format_time(finish.ms)
            };
            draw_text(&format!("{} <-- try again!", own), label_x, y, font_size as f32, BLACK);
        }
    }
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

    while running {
        window::enforce_min_size();

        // Track the live window size (the original recomputes its client
        // rect on every WM_SIZE). The status display is a small overlay at
        // the top-right, so the game viewport is the full window.
        let view_w = screen_width().max(window::MIN_WIDTH as f32) as i16;
        let view_h = screen_height().max(window::MIN_HEIGHT as f32) as i16;
        if capture.is_none() {
            game.screen_w = view_w;
            game.screen_h = view_h;
        }


        let live_input = if capture.is_some() { Vec::new() } else { input.poll() };
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
                    let delta = (timer.frame_delta() * 1000.0) as u32;
                    game.step_tick(delta);
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
        if let Some(cap) = &capture {
            if cap.tick < cap.scenario.ticks {
                skifree::replay::apply_inputs(&mut game, &cap.scenario, cap.tick);
                game.tick(cap.scenario.delta_ms);
            }
        } else if timer.should_tick() {
            let delta = (timer.frame_delta() * 1000.0) as u32;
            game.tick(delta);
        }

        if game.pending_score_save && capture.is_none() {
            persistence::save_scores(&game.high_scores);
            game.pending_score_save = false;
        }

        draw_frame(&game, &renderer);

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
                draw_frame(&game, &renderer);
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
