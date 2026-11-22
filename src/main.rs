mod platform;

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
        game.screen_w = view_w;
        game.screen_h = view_h;

        let mut steer_left = false;
        let mut steer_right = false;
        let mut steer_up = false;
        let mut steer_down = false;

        for action in input.poll() {
            match action {
                GameAction::Quit => running = false,
                GameAction::PauseToggle => {
                    game.paused = !game.paused;
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
                GameAction::SteerLeft => steer_left = true,
                GameAction::SteerRight => steer_right = true,
                GameAction::SteerUp => steer_up = true,
                GameAction::SteerDown => steer_down = true,
                GameAction::FastForward => {
                    game.fast_forward = !game.fast_forward;
                }
                GameAction::StepTick => {
                    // Single-step one tick even when paused
                    let delta = (timer.frame_delta() * 1000.0) as u32;
                    game.tick(delta);
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
                    let center_x = screen_width() / 2.0;
                    let center_y = screen_height() / 3.0;
                    game.steer_mouse(
                        (mx - center_x) as i16,
                        (my - center_y) as i16,
                    );
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

        if steer_left || steer_right || steer_up || steer_down {
            game.steer_keyboard(steer_left, steer_right, steer_up, steer_down);
        }

        if !game.paused && timer.should_tick() {
            let delta = (timer.frame_delta() * 1000.0) as u32;
            game.tick(delta);
        }

        if game.pending_score_save {
            persistence::save_scores(&game.high_scores);
            game.pending_score_save = false;
        }

        // Render
        clear_background(WHITE);

        // Draw all active actors
        let indices = game.pool.active_indices();
        for idx in indices {
            if let Some(actor) = game.pool.get(idx) {
                let (sx, sy) = game.camera.world_to_screen(
                    actor.x,
                    actor.y,
                    view_w,
                    view_h,
                );
                renderer.draw_sprite(actor.sprite_id as u32, sx as f32, sy as f32);
            }
        }

        // Pause overlay
        if game.paused {
            let text = "Ski Paused ... Press F3 to continue";
            let dims = measure_text(text, None, 24, 1.0);
            let x = (screen_width() - dims.width) / 2.0;
            let y = screen_height() / 2.0 + 40.0;
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
        let value_x = screen_width() - 62.0;
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

        next_frame().await;
    }
}
