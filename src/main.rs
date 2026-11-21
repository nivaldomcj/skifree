mod game;
mod platform;

use game::hud::Hud;
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
    let hud = Hud::new();
    let audio = Audio::new();
    audio.load_sounds();

    let scores = persistence::load_scores();
    persistence::save_scores(&scores);

    let mut paused = false;
    let mut running = true;

    while running {
        window::enforce_min_size();

        for action in input.poll() {
            match action {
                GameAction::Quit => running = false,
                GameAction::PauseToggle => {
                    paused = !paused;
                }
                GameAction::Restart => {
                    // Phase 3: reset game state
                    paused = false;
                }
                _ => {} // Phase 3: game logic actions
            }
        }

        if !paused && timer.should_tick() {
            // Phase 3: world update (FUN_00401e50)
        }

        clear_background(WHITE);

        // Title logo (bitmap_53) centered
        let logo_id = 53;
        if let Some(tex) = renderer.sprite(logo_id) {
            let x = (screen_width() - tex.width()) / 2.0;
            let y = (screen_height() - window::HUD_HEIGHT - tex.height()) / 2.0;
            renderer.draw_sprite(logo_id, x, y);
        }

        // Pause overlay
        if paused {
            let text = "Ski Paused ... Press F3 to continue";
            let dims = measure_text(text, None, 24, 1.0);
            let x = (screen_width() - dims.width) / 2.0;
            let y = screen_height() / 2.0 + 40.0;
            draw_rectangle(x - 8.0, y - 20.0, dims.width + 16.0, 28.0, WHITE);
            draw_rectangle_lines(x - 8.0, y - 20.0, dims.width + 16.0, 28.0, 1.0, BLACK);
            draw_text(text, x, y, 24.0, BLACK);
        }

        // HUD strip at bottom
        let hud_y = screen_height() - window::HUD_HEIGHT;
        draw_line(0.0, hud_y, screen_width(), hud_y, 1.0, LIGHTGRAY);
        hud.draw(hud_y);

        next_frame().await;
    }
}
