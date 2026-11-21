use macroquad::prelude::*;

#[derive(Debug, Clone, PartialEq)]
pub enum GameAction {
    SteerLeft,
    SteerRight,
    SteerUp,
    SteerDown,
    Restart,
    PauseToggle,
    Quit,
    FastForward,
    StepTick,
    Repaint,
    TeleportLeft,
    TeleportRight,
    TeleportUp,
    TeleportDown,
    MouseSteer(f32, f32),
    Click(f32, f32),
}

pub struct Input {
    prev_mouse_pos: (f32, f32),
}

impl Input {
    pub fn new() -> Self {
        Self {
            prev_mouse_pos: (0.0, 0.0),
        }
    }

    /// Poll all input and return actions for this frame.
    pub fn poll(&mut self) -> Vec<GameAction> {
        let mut actions = Vec::new();

        // Steering — arrows and numpad
        if is_key_down(KeyCode::Left) || is_key_down(KeyCode::Kp4) {
            actions.push(GameAction::SteerLeft);
        }
        if is_key_down(KeyCode::Right) || is_key_down(KeyCode::Kp6) {
            actions.push(GameAction::SteerRight);
        }
        if is_key_down(KeyCode::Up) || is_key_down(KeyCode::Kp8) {
            actions.push(GameAction::SteerUp);
        }
        if is_key_down(KeyCode::Down) || is_key_down(KeyCode::Kp2) {
            actions.push(GameAction::SteerDown);
        }

        // Diagonals
        if is_key_down(KeyCode::Kp7) {
            actions.push(GameAction::SteerLeft);
            actions.push(GameAction::SteerUp);
        }
        if is_key_down(KeyCode::Kp9) {
            actions.push(GameAction::SteerRight);
            actions.push(GameAction::SteerUp);
        }
        if is_key_down(KeyCode::Kp1) {
            actions.push(GameAction::SteerLeft);
            actions.push(GameAction::SteerDown);
        }
        if is_key_down(KeyCode::Kp3) {
            actions.push(GameAction::SteerRight);
            actions.push(GameAction::SteerDown);
        }

        // System keys
        if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::F2) {
            actions.push(GameAction::Restart);
        }
        if is_key_pressed(KeyCode::F3) {
            actions.push(GameAction::PauseToggle);
        }
        if is_key_pressed(KeyCode::Escape) {
            actions.push(GameAction::Quit);
        }

        // Cheat keys (WM_CHAR in the original)
        if is_key_pressed(KeyCode::F) {
            actions.push(GameAction::FastForward);
        }
        if is_key_pressed(KeyCode::T) {
            actions.push(GameAction::StepTick);
        }
        if is_key_pressed(KeyCode::R) {
            actions.push(GameAction::Repaint);
        }
        if is_key_pressed(KeyCode::X) {
            if is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift) {
                actions.push(GameAction::TeleportRight);
            } else {
                actions.push(GameAction::TeleportLeft);
            }
        }
        if is_key_pressed(KeyCode::Y) {
            if is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift) {
                actions.push(GameAction::TeleportUp);
            } else {
                actions.push(GameAction::TeleportDown);
            }
        }

        // Mouse steering
        let (mx, my) = mouse_position();
        if (mx, my) != self.prev_mouse_pos {
            actions.push(GameAction::MouseSteer(mx, my));
            self.prev_mouse_pos = (mx, my);
        }
        if is_mouse_button_pressed(MouseButton::Left) {
            actions.push(GameAction::Click(mx, my));
        }

        actions
    }
}
