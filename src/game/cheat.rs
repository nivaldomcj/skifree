//! The cheat keys: the original's `WM_CHAR` handler (`FUN_00406780`). The
//! live game and the replay both read this one table.

/// What one cheat key does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cheat {
    /// Move the skier by (dx, dy), the view with him (`FUN_00402390`).
    Teleport(i16, i16),
    /// Toggle fast-forward (`DAT_0040c670`): the integrator runs twice.
    FastForward,
    /// Repaint the window (the port repaints every frame anyway).
    Repaint,
    /// Run one tick on the spot, even while paused (it jumps to the timer
    /// callback `FUN_00401000`).
    Step,
}

impl Cheat {
    /// `FUN_00406780`'s table, case-sensitive (it reads `WM_CHAR`, so keys
    /// repeat while held): x moves the skier right (+2) and X left (-2), y
    /// down and Y up; f, r and t are lowercase only.
    pub fn from_char(ch: char) -> Option<Self> {
        Some(match ch {
            'x' => Self::Teleport(2, 0),
            'X' => Self::Teleport(-2, 0),
            'y' => Self::Teleport(0, 2),
            'Y' => Self::Teleport(0, -2),
            'f' => Self::FastForward,
            'r' => Self::Repaint,
            't' => Self::Step,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wm_char_table() {
        assert_eq!(Cheat::from_char('x'), Some(Cheat::Teleport(2, 0)));
        assert_eq!(Cheat::from_char('X'), Some(Cheat::Teleport(-2, 0)));
        assert_eq!(Cheat::from_char('y'), Some(Cheat::Teleport(0, 2)));
        assert_eq!(Cheat::from_char('Y'), Some(Cheat::Teleport(0, -2)));
        assert_eq!(Cheat::from_char('t'), Some(Cheat::Step));
        assert_eq!(Cheat::from_char('T'), None);
        assert_eq!(Cheat::from_char('F'), None);
    }
}
