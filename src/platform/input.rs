use macroquad::prelude::*;
use skifree::game::cheat::Cheat;
use skifree::game::skier::ArrowKey;

#[derive(Debug, Clone, PartialEq)]
pub enum GameAction {
    /// A cardinal steering key: one state step per key-down.
    Steer(ArrowKey),
    /// Diagonal keys set a skiing state directly (1/4 = slight, 3/6 = hard).
    SteerAbsolute(u16),
    /// Keyboard jump (Insert / numpad0) — smaller hop that scrubs speed.
    Jump,
    /// F2: restart at any time.
    Restart,
    /// Enter: restart only once the skier is gone (eaten / game over).
    RestartIfDead,
    PauseToggle,
    /// Esc, natively (nothing on the web).
    #[cfg_attr(target_arch = "wasm32", expect(dead_code, reason = "a browser tab cannot quit"))]
    Quit,
    /// A cheat key (`WM_CHAR`): teleports, fast-forward, repaint, step.
    Cheat(Cheat),
    /// M: mute or unmute the sound (the port's own key; the original has
    /// no mute key, only the `nosound` command line).
    ToggleMute,
    MouseSteer(f32, f32),
    Click(f32, f32),
    /// The window gained (`true`) or lost (`false`) focus, or was minimized
    /// (`WM_ACTIVATE` / `WM_SIZE` in the original). Always delivered first.
    Focus(bool),
}

pub struct Input {
    /// macroquad input-event subscriber: window, key, character and mouse
    /// events in arrival order.
    subscriber: usize,
}

/// One window event, in arrival order, like the original's message queue.
#[derive(Debug, Clone, Copy, PartialEq)]
enum WindowEvent {
    /// Activation change: miniquad reports focus loss as
    /// `window_minimized_event` and focus gain as `window_restored_event` on
    /// Windows, macOS and X11 — the transitions the original's `WM_ACTIVATE` /
    /// `WM_SIZE` handlers react to.
    Focus(bool),
    /// A key-down from the OS, auto-repeats included — the original reacts
    /// to each `WM_KEYDOWN`, so holding a key steps once on the press and then
    /// at the OS repeat rate.
    KeyDown(KeyCode),
    /// A typed character (`WM_CHAR`), repeats included: the cheat keys.
    Char(char),
    /// Pointer moved, in miniquad's (physical) pixels (`WM_MOUSEMOVE`).
    MouseMove(f32, f32),
    /// Left button pressed, in miniquad's pixels (`WM_LBUTTONDOWN`).
    MouseDown(f32, f32),
    /// M pressed without Ctrl, Alt or Cmd (not a repeat): the port's own
    /// mute key. Cmd+M is macOS's minimize, Ctrl+M and Alt+M belong to the
    /// browser or the desktop.
    Mute,
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
    fn key_down_event(&mut self, keycode: KeyCode, mods: macroquad::miniquad::KeyMods, repeat: bool) {
        if keycode == KeyCode::M {
            // Not a cheat key either: the original's WM_CHAR handler ignores M.
            if !(repeat || mods.ctrl || mods.alt || mods.logo) {
                self.0.push(WindowEvent::Mute);
            }
            return;
        }
        self.0.push(WindowEvent::KeyDown(keycode));
    }
    fn char_event(&mut self, character: char, mods: macroquad::miniquad::KeyMods, _repeat: bool) {
        // WM_CHAR never carries a plain letter with Ctrl (a control code),
        // Alt (WM_SYSCHAR) or the Win/Cmd key; X11, Wayland and macOS do.
        if !(mods.ctrl || mods.alt || mods.logo) {
            self.0.push(WindowEvent::Char(character));
        }
    }
    fn mouse_motion_event(&mut self, x: f32, y: f32) {
        self.0.push(WindowEvent::MouseMove(x, y));
    }
    fn mouse_button_down_event(&mut self, button: MouseButton, x: f32, y: f32) {
        if button == MouseButton::Left {
            self.0.push(WindowEvent::MouseDown(x, y));
        }
    }
}

/// `WM_KEYDOWN` handling (`FUN_00406170`): the action for one key-down.
fn key_action(key: KeyCode) -> Option<GameAction> {
    use KeyCode::{
        Down, End, Enter, Home, Insert, Kp0, Kp1, Kp2, Kp3, Kp4, Kp6, Kp7, Kp8, Kp9, KpEnter, Left, PageDown, PageUp,
        Right, Up, F2, F3,
    };
    Some(match key {
        Left | Kp4 => GameAction::Steer(ArrowKey::Left),
        Right | Kp6 => GameAction::Steer(ArrowKey::Right),
        Up | Kp8 => GameAction::Steer(ArrowKey::Up),
        Down | Kp2 => GameAction::Steer(ArrowKey::Down),
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
        // The original minimizes; natively the port quits (see
        // docs/porting-guide.md). A browser tab can do neither: nothing.
        #[cfg(not(target_arch = "wasm32"))]
        KeyCode::Escape => GameAction::Quit,
        _ => return None,
    })
}

/// `WM_CHAR` handling: the cheat keys (`Cheat::from_char`).
fn char_action(ch: char) -> Option<GameAction> {
    Cheat::from_char(ch).map(GameAction::Cheat)
}

/// A point in miniquad's pixels as client pixels, as `WM_MOUSEMOVE`
/// reports them (whole pixels, origin at the top-left).
///
/// miniquad's macOS backend converts Cocoa's bottom-up position with
/// `height - y*scale - 1` in backing pixels, so after the division by the
/// scale the pointer reads one backing pixel too high — measured with
/// synthetic `NSEvent`s: a pointer at client y 100.1 read 99.1, i.e. row 99.
/// Undo that there; the Windows and X11/Wayland backends pass client
/// coordinates through unchanged. Cocoa positions are fractional: floor.
fn client_point(x: f32, y: f32, scale: f32) -> (f32, f32) {
    let (x, y) = (x / scale, y / scale);
    let y = if cfg!(target_os = "macos") { y + 1.0 / scale } else { y };
    (x.floor(), y.floor())
}

/// The frame's window events as game actions, in arrival order.
/// `focus_loss_counts` tells a real focus loss from miniquad's macOS
/// "minimized" on a mere window move.
fn translate(events: &[WindowEvent], scale: f32, focus_loss_counts: impl Fn() -> bool) -> Vec<GameAction> {
    let mut actions = Vec::new();
    let mut activated = false;
    for &event in events {
        match event {
            // miniquad on macOS also sends the "minimized" event when the
            // window is merely moved, and no "restored" follows while it
            // stays key: deactivating then froze the game after a drag.
            // The original keeps running while its window moves, so a focus
            // loss counts only if the window really is inactive.
            WindowEvent::Focus(false) if !focus_loss_counts() => {}
            WindowEvent::Focus(active) => {
                activated |= active;
                actions.push(GameAction::Focus(active));
            }
            WindowEvent::KeyDown(key) => actions.extend(key_action(key)),
            WindowEvent::Char(ch) => actions.extend(char_action(ch)),
            WindowEvent::Mute => actions.push(GameAction::ToggleMute),
            WindowEvent::MouseMove(x, y) => {
                let (cx, cy) = client_point(x, y, scale);
                actions.push(GameAction::MouseSteer(cx, cy));
            }
            // The click that activates the window is swallowed, like the
            // original's WM_MOUSEACTIVATE -> MA_ACTIVATEANDEAT.
            // A move to the point first: Windows always sends WM_MOUSEMOVE
            // before a click somewhere new, but a touch tap (macroquad's
            // touch emulation) arrives as a bare button-down. The game
            // ignores a move to where the pointer already is.
            WindowEvent::MouseDown(x, y) => {
                let (cx, cy) = client_point(x, y, scale);
                actions.push(GameAction::MouseSteer(cx, cy));
                if !activated {
                    actions.push(GameAction::Click(cx, cy));
                }
            }
        }
    }
    actions
}

impl Input {
    pub fn new() -> Self {
        Self { subscriber: macroquad::input::utils::register_input_subscriber() }
    }

    /// Poll all input and return actions for this frame, in arrival order.
    pub fn poll(&mut self) -> Vec<GameAction> {
        let mut events = WindowEvents(Vec::new());
        macroquad::input::utils::repeat_all_miniquad_input(&mut events, self.subscriber);
        let scale = macroquad::miniquad::window::dpi_scale();
        translate(&events.0, scale, || super::os_window::window_is_active() != Some(true))
    }
}

/// The mouse position in client pixels, as `WM_MOUSEMOVE` reports it (see
/// `client_point`), for the drawn message box's OK button.
pub fn client_mouse_position() -> (f32, f32) {
    let scale = macroquad::miniquad::window::dpi_scale();
    let (x, y) = mouse_position();
    client_point(x * scale, y * scale, scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A key without an action does nothing; only Esc quits. (Esc is
    /// spelled `KeyCode::Escape` in `key_action`: an import list without
    /// it would make the arm a catch-all binding that quits on every key.)
    #[test]
    fn only_mapped_keys_act() {
        assert_eq!(key_action(KeyCode::A), None);
        assert_eq!(key_action(KeyCode::Space), None);
        assert_eq!(key_action(KeyCode::Left), Some(GameAction::Steer(ArrowKey::Left)));
        assert_eq!(key_action(KeyCode::F3), Some(GameAction::PauseToggle));
        #[cfg(not(target_arch = "wasm32"))]
        assert_eq!(key_action(KeyCode::Escape), Some(GameAction::Quit));
    }

    #[test]
    fn m_mutes_only_without_modifiers() {
        use macroquad::miniquad::{EventHandler, KeyMods};
        let plain = KeyMods { shift: false, ctrl: false, alt: false, logo: false };
        let mut events = WindowEvents(Vec::new());
        events.key_down_event(KeyCode::M, plain, false);
        events.key_down_event(KeyCode::M, plain, true);
        events.key_down_event(KeyCode::M, KeyMods { logo: true, ..plain }, false);
        events.key_down_event(KeyCode::M, KeyMods { ctrl: true, ..plain }, false);
        events.key_down_event(KeyCode::M, KeyMods { shift: true, ..plain }, false);
        assert_eq!(events.0, vec![WindowEvent::Mute, WindowEvent::Mute]);
        let actions = translate(&[WindowEvent::KeyDown(KeyCode::Left), WindowEvent::Mute], 1.0, || true);
        assert_eq!(actions, vec![GameAction::Steer(ArrowKey::Left), GameAction::ToggleMute], "in arrival order");
    }

    #[test]
    fn cheat_keys_follow_the_wm_char_table() {
        assert_eq!(char_action('x'), Some(GameAction::Cheat(Cheat::Teleport(2, 0))));
        assert_eq!(char_action('T'), None);
    }

    #[test]
    fn events_keep_their_order_and_activation_eats_the_click() {
        use WindowEvent::{Focus, MouseDown, MouseMove};
        let actions = translate(&[MouseMove(10.0, 20.0), MouseDown(10.0, 20.0), MouseMove(30.0, 40.0)], 1.0, || true);
        let y = |v: f32| if cfg!(target_os = "macos") { v + 1.0 } else { v };
        assert_eq!(
            actions,
            vec![
                GameAction::MouseSteer(10.0, y(20.0)),
                GameAction::MouseSteer(10.0, y(20.0)),
                GameAction::Click(10.0, y(20.0)),
                GameAction::MouseSteer(30.0, y(40.0))
            ]
        );
        // A touch tap: a bare button-down steers to the point, then clicks.
        let actions = translate(&[MouseDown(5.0, 6.0)], 1.0, || true);
        assert_eq!(actions, vec![GameAction::MouseSteer(5.0, y(6.0)), GameAction::Click(5.0, y(6.0))]);
        let actions = translate(&[Focus(true), MouseDown(1.0, 1.0)], 1.0, || true);
        assert_eq!(actions, vec![GameAction::Focus(true), GameAction::MouseSteer(1.0, y(1.0))]);
        assert!(translate(&[], 1.0, || true).is_empty(), "nothing invented without events");
    }
}
