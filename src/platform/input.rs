use macroquad::prelude::*;

#[derive(Debug, Clone, PartialEq)]
pub enum GameAction {
    SteerLeft,
    SteerRight,
    SteerUp,
    SteerDown,
    /// Diagonal keys set a skiing state directly (1/4 = slight, 3/6 = hard).
    SteerAbsolute(u16),
    /// Keyboard jump (Insert / numpad0) — smaller hop that scrubs speed.
    Jump,
    /// F2: restart at any time.
    Restart,
    /// Enter: restart only once the skier is gone (eaten / game over).
    RestartIfDead,
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

        // Diagonals set a skiing state directly (numpad + nav cluster).
        // Top row = hard turns (3 left / 6 right), bottom row = slight (1 / 4).
        if is_key_down(KeyCode::Kp7) || is_key_down(KeyCode::Home) {
            actions.push(GameAction::SteerAbsolute(3));
        }
        if is_key_down(KeyCode::Kp9) || is_key_down(KeyCode::PageUp) {
            actions.push(GameAction::SteerAbsolute(6));
        }
        if is_key_down(KeyCode::Kp1) || is_key_down(KeyCode::End) {
            actions.push(GameAction::SteerAbsolute(1));
        }
        if is_key_down(KeyCode::Kp3) || is_key_down(KeyCode::PageDown) {
            actions.push(GameAction::SteerAbsolute(4));
        }

        // Keyboard jump (Insert / numpad0)
        if is_key_pressed(KeyCode::Insert) || is_key_pressed(KeyCode::Kp0) {
            actions.push(GameAction::Jump);
        }

        // System keys
        if is_key_pressed(KeyCode::Enter) {
            actions.push(GameAction::RestartIfDead);
        }
        if is_key_pressed(KeyCode::F2) {
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
