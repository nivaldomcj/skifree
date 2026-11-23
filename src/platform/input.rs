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
    /// The window gained (`true`) or lost (`false`) focus, or was minimized
    /// (WM_ACTIVATE / WM_SIZE in the original). Always delivered first.
    Focus(bool),
}

pub struct Input {
    prev_mouse_pos: (f32, f32),
    /// macroquad input-event subscriber, for the window focus events.
    subscriber: usize,
}

/// One window event, in arrival order.
enum WindowEvent {
    /// Activation change: miniquad reports focus loss as
    /// `window_minimized_event` and focus gain as `window_restored_event` on
    /// Windows, macOS and X11 — the transitions the original's WM_ACTIVATE /
    /// WM_SIZE handlers react to.
    Focus(bool),
    /// A key-down from the OS, auto-repeats included — the original reacts
    /// to each WM_KEYDOWN, so holding a key steps once on the press and then
    /// at the OS repeat rate.
    KeyDown(KeyCode),
}

struct WindowEvents(Vec<WindowEvent>);

impl macroquad::miniquad::EventHandler for WindowEvents {
    fn update(&mut self) {}
    fn draw(&mut self) {}
    fn window_minimized_event(&mut self) {
        self.0.push(WindowEvent::Focus(false));
    }
    fn window_restored_event(&mut self) {
        self.0.push(WindowEvent::Focus(true));
    }
    fn key_down_event(&mut self, keycode: KeyCode, _mods: macroquad::miniquad::KeyMods, _repeat: bool) {
        self.0.push(WindowEvent::KeyDown(keycode));
    }
}

/// WM_KEYDOWN handling (FUN_00406170): the action for one key-down.
fn key_action(key: KeyCode) -> Option<GameAction> {
    use KeyCode::*;
    Some(match key {
        Left | Kp4 => GameAction::SteerLeft,
        Right | Kp6 => GameAction::SteerRight,
        Up | Kp8 => GameAction::SteerUp,
        Down | Kp2 => GameAction::SteerDown,
        // Diagonals set a skiing state directly (numpad + nav cluster).
        // Top row = hard turns (3 left / 6 right), bottom row = slight (1 / 4).
        Kp7 | Home => GameAction::SteerAbsolute(3),
        Kp9 | PageUp => GameAction::SteerAbsolute(6),
        Kp1 | End => GameAction::SteerAbsolute(1),
        Kp3 | PageDown => GameAction::SteerAbsolute(4),
        // Keyboard jump (Insert / numpad0)
        Insert | Kp0 => GameAction::Jump,
        Enter | KpEnter => GameAction::RestartIfDead,
        F2 => GameAction::Restart,
        F3 => GameAction::PauseToggle,
        Escape => GameAction::Quit,
        _ => return None,
    })
}

impl Input {
    pub fn new() -> Self {
        Self {
            prev_mouse_pos: (0.0, 0.0),
            subscriber: macroquad::input::utils::register_input_subscriber(),
        }
    }

    /// Poll all input and return actions for this frame.
    pub fn poll(&mut self) -> Vec<GameAction> {
        let mut actions = Vec::new();

        // Window events in arrival order: activation changes and every OS
        // key-down (repeats included), like the original's message queue.
        let mut events = WindowEvents(Vec::new());
        macroquad::input::utils::repeat_all_miniquad_input(&mut events, self.subscriber);
        let mut activated = false;
        for event in events.0 {
            match event {
                WindowEvent::Focus(active) => {
                    activated |= active;
                    actions.push(GameAction::Focus(active));
                }
                WindowEvent::KeyDown(key) => {
                    if let Some(action) = key_action(key) {
                        actions.push(action);
                    }
                }
            }
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
        let (mx, my) = client_mouse_position();
        if (mx, my) != self.prev_mouse_pos {
            actions.push(GameAction::MouseSteer(mx, my));
            self.prev_mouse_pos = (mx, my);
        }
        // The click that activates the window is swallowed, like the
        // original's WM_MOUSEACTIVATE -> MA_ACTIVATEANDEAT.
        if is_mouse_button_pressed(MouseButton::Left) && !activated {
            actions.push(GameAction::Click(mx, my));
        }

        actions
    }
}

/// The mouse position in client pixels, as WM_MOUSEMOVE reports it
/// (whole pixels, origin at the top-left).
///
/// miniquad's macOS backend converts Cocoa's bottom-up position with
/// `height - y*scale - 1` in backing pixels, so after macroquad's division
/// by the scale the pointer reads one backing pixel too high — measured
/// with synthetic NSEvents: a pointer at client y 100.1 read 99.1, i.e. row
/// 99. Undo that there; the Windows and X11/Wayland backends pass client
/// coordinates through unchanged. Cocoa positions are fractional: floor.
pub fn client_mouse_position() -> (f32, f32) {
    let (x, y) = mouse_position();
    let y = if cfg!(target_os = "macos") {
        y + 1.0 / macroquad::miniquad::window::dpi_scale()
    } else {
        y
    };
    (x.floor(), y.floor())
}

