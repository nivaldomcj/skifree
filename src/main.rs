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

#[macroquad::main(window_conf)]
async fn main() {
    let renderer = Renderer::new().await;
    let mut timer = GameTimer::new();
    let mut input = Input::new();
    let audio = Audio::new();
    audio.load_sounds();

    let scores = persistence::load_scores();
    persistence::save_scores(&scores);

    let mut game = GameState::new(window::DEFAULT_WIDTH as i16, window::DEFAULT_HEIGHT as i16);
    game.new_game();

    let mut running = true;

    while running {
        window::enforce_min_size();

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
                    let center_x = window::DEFAULT_WIDTH as f32 / 2.0;
                    let center_y = window::DEFAULT_HEIGHT as f32 / 2.0;
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

        if steer_left || steer_right || steer_up || steer_down {
            game.steer_keyboard(steer_left, steer_right, steer_up, steer_down);
        }

        if !game.paused && timer.should_tick() {
            let delta = (timer.frame_delta() * 1000.0) as u32;
            game.tick(delta);
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
                    window::DEFAULT_WIDTH as i16,
                    window::DEFAULT_HEIGHT as i16,
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

        // HUD
        let hud_y = screen_height() - window::HUD_HEIGHT;
        draw_line(0.0, hud_y, screen_width(), hud_y, 1.0, LIGHTGRAY);

        let time_str = game.scoring.format_time(game.elapsed_ms);
        let (_px, py, _) = game.player_pos().unwrap_or((0, 0, 0));
        let (vx, _, _) = game.player_velocity().unwrap_or((0, 0, 0));
        let dist_str = game.scoring.format_dist(py, 0x21c0, true);
        let speed_str = game.scoring.format_speed(vx, game.last_tick_ms);
        let style_str = game.scoring.format_style();

        let labels = ["Time:", "Dist:", "Speed:", "Style:"];
        let values = [&time_str, &dist_str, &speed_str, &style_str];
        let spacing = screen_width() / 4.0;
        let font_size = 14u16;

        for (i, (label, value)) in labels.iter().zip(values.iter()).enumerate() {
            let x = i as f32 * spacing + 4.0;
            draw_text(label, x, hud_y + 14.0, font_size as f32, DARKGRAY);
            let label_width = measure_text(label, None, font_size, 1.0).width;
            draw_text(
                value,
                x + label_width + 2.0,
                hud_y + 14.0,
                font_size as f32,
                BLACK,
            );
        }

        next_frame().await;
    }
}
