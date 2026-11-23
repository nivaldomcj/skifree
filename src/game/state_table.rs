/// State-to-sprite mapping table (DAT_0040a1ac in the original binary)
pub const STATE_TO_SPRITE: [u16; 64] = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, // 0x00-0x0f
    17, 18, 19, 20, 21, 22, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, // 0x10-0x1f
    38, 39, 40, 41, 42, 43, 44, 65, 66, 67, 68, 69, 70, 71, 72, 73, // 0x20-0x2f
    74, 75, 76, 77, 78, 79, 80, 81, 83, 84, 85, 84, 49, 87, 88, 89, // 0x30-0x3f
];

/// Skier physics table (DAT_0040a308), 8 i16 per state
/// Fields: [accel, max_speed, decel, friction_div, direction, ?, sprite_id, ?]
pub struct PhysicsEntry {
    pub accel: i16,
    pub max_speed: i16,
    pub decel: i16,
    pub friction_div: i16,
    pub direction: i16,
}

#[rustfmt::skip]
pub const SKIER_PHYSICS: [PhysicsEntry; 22] = [
    PhysicsEntry { accel: 1, max_speed: 16, decel: 0, friction_div: 0, direction: 0 },  // 0x00
    PhysicsEntry { accel: 1, max_speed: 12, decel: 1, friction_div: 1, direction: -1 }, // 0x01
    PhysicsEntry { accel: 1, max_speed: 6, decel: 1, friction_div: 4, direction: -1 },  // 0x02
    PhysicsEntry { accel: 1, max_speed: 0, decel: 1, friction_div: 8, direction: -1 },  // 0x03
    PhysicsEntry { accel: 1, max_speed: 12, decel: 1, friction_div: 1, direction: 1 },  // 0x04
    PhysicsEntry { accel: 1, max_speed: 6, decel: 1, friction_div: 4, direction: 1 },   // 0x05
    PhysicsEntry { accel: 1, max_speed: 0, decel: 1, friction_div: 8, direction: 1 },   // 0x06
    PhysicsEntry { accel: 1, max_speed: 0, decel: 1, friction_div: 8, direction: -1 },  // 0x07
    PhysicsEntry { accel: 1, max_speed: 0, decel: 1, friction_div: 8, direction: 1 },   // 0x08
    PhysicsEntry { accel: 1, max_speed: 0, decel: 0, friction_div: 0, direction: 0 },   // 0x09
    PhysicsEntry { accel: 1, max_speed: 0, decel: 0, friction_div: 0, direction: 0 },   // 0x0a
    PhysicsEntry { accel: 0, max_speed: 0, decel: 0, friction_div: 0, direction: 0 },   // 0x0b
    PhysicsEntry { accel: 0, max_speed: 0, decel: 0, friction_div: 0, direction: 0 },   // 0x0c
    PhysicsEntry { accel: 1, max_speed: 24, decel: 0, friction_div: 0, direction: 0 },  // 0x0d
    PhysicsEntry { accel: 1, max_speed: 22, decel: 0, friction_div: 0, direction: 0 },  // 0x0e
    PhysicsEntry { accel: 1, max_speed: 22, decel: 0, friction_div: 0, direction: 0 },  // 0x0f
    PhysicsEntry { accel: 1, max_speed: 20, decel: 0, friction_div: 0, direction: 0 },  // 0x10
    PhysicsEntry { accel: 1, max_speed: 24, decel: 0, friction_div: 0, direction: 0 },  // 0x11
    PhysicsEntry { accel: 1, max_speed: 20, decel: 0, friction_div: 0, direction: 0 },  // 0x12
    PhysicsEntry { accel: 1, max_speed: 20, decel: 0, friction_div: 0, direction: 0 },  // 0x13
    PhysicsEntry { accel: 1, max_speed: 22, decel: 0, friction_div: 0, direction: 0 },  // 0x14
    PhysicsEntry { accel: 1, max_speed: 22, decel: 0, friction_div: 0, direction: 0 },  // 0x15
];

/// Per-class initial state for populator-spawned actors (DAT_0040a22c,
/// classes 0..=10). Classes >= 0xb spawn with a raw sprite instead
/// (FUN_00402850, see populator.rs).
#[rustfmt::skip]
pub const CLASS_INITIAL_STATE: [u16; 11] = [
    0x06, // 0 skier
    0x16, // 1 dog
    0x1b, // 2 snowboarder
    0x1f, // 3 obstacle / crossing skier
    0x27, // 4 ambient mob
    0x2a, // 5-8 monsters
    0x2a,
    0x2a,
    0x2a,
    0x38, // 9 eating
    0x3c, // 10 lift
];

/// Dog physics table (DAT_0040a490), states 0x16-0x1a
#[rustfmt::skip]
pub const DOG_PHYSICS: [PhysicsEntry; 5] = [
    PhysicsEntry { accel: 1, max_speed: 1, decel: 0, friction_div: 0, direction: 0 },   // 0x16
    PhysicsEntry { accel: 1, max_speed: 1, decel: 1, friction_div: 4, direction: -1 },  // 0x17
    PhysicsEntry { accel: 1, max_speed: 1, decel: 1, friction_div: 4, direction: 1 },   // 0x18
    PhysicsEntry { accel: 0, max_speed: 0, decel: 0, friction_div: 0, direction: 0 },   // 0x19
    PhysicsEntry { accel: 0, max_speed: 0, decel: 0, friction_div: 0, direction: 0 },   // 0x1a
];

/// Snowboarder physics table (DAT_0040a4e0), states 0x1f-0x26
pub const SNOWBOARDER_PHYSICS: [PhysicsEntry; 8] = [
    PhysicsEntry { accel: 2, max_speed: 18, decel: 2, friction_div: 1, direction: -1 }, // 0x1f
    PhysicsEntry { accel: 2, max_speed: 18, decel: 2, friction_div: 1, direction: 1 },  // 0x20
    PhysicsEntry { accel: 1, max_speed: 22, decel: 0, friction_div: 0, direction: 0 },  // 0x21
    PhysicsEntry { accel: 1, max_speed: 4, decel: 0, friction_div: 0, direction: 0 },   // 0x22
    PhysicsEntry { accel: 1, max_speed: 4, decel: 0, friction_div: 0, direction: 0 },   // 0x23
    PhysicsEntry { accel: 1, max_speed: 4, decel: 0, friction_div: 0, direction: 0 },   // 0x24
    PhysicsEntry { accel: 1, max_speed: 4, decel: 0, friction_div: 0, direction: 0 },   // 0x25
    PhysicsEntry { accel: 1, max_speed: 4, decel: 0, friction_div: 0, direction: 0 },   // 0x26
];

/// Crash landing table (DAT_0040a434), indexed by state 0x0d-0x15
pub const CRASH_LANDING: [u16; 9] = [0x0, 0x3, 0x6, 0xb, 0xb, 0xb, 0xb, 0xb, 0xb];

/// Skier direction transition table (DAT_0040a258), raw interleaved bytes.
/// Each state occupies 8 bytes: a LEFT-transition u32 at +0 and a
/// RIGHT-transition u32 at +4. This is the flat u32 view (states 0..=10).
pub const DIRECTION_TRANSITION: [u16; 22] = [1, 4, 2, 0, 3, 1, 7, 2, 0, 5, 4, 6, 5, 8, 3, 2, 5, 6, 9, 2, 5, 10];

/// LEFT-key transition table, one u32 per state (DAT_0040a258 + state*8).
/// Cycles 0→1→2→3→7 (state 7 = hard carve, also kicks vx).
pub const LEFT_TRANSITION: [u16; 22] =
    [1, 2, 3, 7, 0, 4, 5, 3, 5, 9, 5, 3, 3, 0xe, 0x10, 0xd, 0xf, 0xe, 0x14, 0x14, 0x10, 0xd];

/// RIGHT-key transition table, one u32 per state (DAT_0040a258 + state*8 + 4).
/// Cycles 0→4→5→6→8 (state 8 = hard carve, also kicks vx).
pub const RIGHT_TRANSITION: [u16; 22] =
    [4, 0, 1, 2, 5, 6, 8, 2, 6, 2, 0xa, 6, 6, 0xf, 0xd, 0x10, 0xe, 0xf, 0x15, 0x15, 0xd, 0x10];

/// Mouse steering direction table (FUN_004065e0)
/// Returns direction 0-6 based on slope ratio dy/dx
pub fn mouse_direction(dx: i16, dy: i16) -> u16 {
    if dy <= 0 {
        return if dx >= 0 { 6 } else { 3 };
    }
    if dx == 0 {
        return 0;
    }
    let ratio = ((dy as i32) << 2) / (dx as i32);
    let r = ratio as i16;
    if r < -11 {
        0
    } else if r < -5 {
        1
    } else if r < -2 {
        2
    } else if r < 0 {
        3
    } else if r > 11 {
        0
    } else if r > 5 {
        4
    } else if r > 2 {
        5
    } else if r > 0 {
        6
    } else if dx >= 0 {
        // ratio truncated to 0 (nearly horizontal): side by dx, like the
        // dy <= 0 case (0x406655)
        6
    } else {
        3
    }
}

/// Airborne mouse steering (FUN_00406670): picks a jump pose 0xd-0x10 by
/// the cursor's quadrant relative to the skier and which axis dominates.
pub fn mouse_air_direction(dx: i16, dy: i16) -> u16 {
    if dx < 0 {
        if dy < 0 {
            if dy >= dx {
                0x0e
            } else {
                0x10
            }
        } else if -dy >= dx {
            0x0e
        } else {
            0x0d
        }
    } else if dy < 0 {
        if -dy > dx {
            0x10
        } else {
            0x0f
        }
    } else if dy <= dx {
        0x0e
    } else {
        0x0d
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// The tables live in .rdata, which is identity-mapped in this binary
    /// (vaddr 0xa000 == rawptr 0xa000), so RVA == file offset.
    ///
    /// Returns None when the (gitignored, copyrighted) binary is absent so the
    /// byte-comparison tests skip instead of failing for contributors.
    fn read_exe() -> Option<Vec<u8>> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("originals").join("ski32.exe");
        std::fs::read(path).ok()
    }

    macro_rules! exe_or_skip {
        () => {
            match read_exe() {
                Some(e) => e,
                None => {
                    eprintln!("skip: originals/ski32.exe not present");
                    return;
                }
            }
        };
    }

    fn u16_at(data: &[u8], off: usize) -> u16 {
        u16::from_le_bytes([data[off], data[off + 1]])
    }
    fn i16_at(data: &[u8], off: usize) -> i16 {
        i16::from_le_bytes([data[off], data[off + 1]])
    }
    fn u32_at(data: &[u8], off: usize) -> u32 {
        u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
    }

    #[test]
    fn state_to_sprite_matches_binary() {
        let exe = exe_or_skip!();
        for (state, &want) in STATE_TO_SPRITE.iter().enumerate() {
            let got = u16_at(&exe, 0xa1ac + state * 2);
            assert_eq!(got, want, "state 0x{state:02x}");
        }
    }

    #[test]
    fn direction_transition_matches_binary() {
        let exe = exe_or_skip!();
        for (i, &want) in DIRECTION_TRANSITION.iter().enumerate() {
            let got = u32_at(&exe, 0xa258 + i * 4) as u16;
            assert_eq!(got, want, "index {i}");
        }
    }

    #[test]
    fn left_right_transitions_match_binary() {
        let exe = exe_or_skip!();
        for state in 0..22usize {
            let base = 0xa258 + state * 8;
            assert_eq!(u32_at(&exe, base) as u16, LEFT_TRANSITION[state], "state {state:#04x} left");
            assert_eq!(u32_at(&exe, base + 4) as u16, RIGHT_TRANSITION[state], "state {state:#04x} right");
        }
    }

    #[test]
    fn skier_physics_matches_binary() {
        let exe = exe_or_skip!();
        for (state, entry) in SKIER_PHYSICS.iter().enumerate() {
            let off = 0xa308 + state * 16;
            assert_eq!(i16_at(&exe, off), entry.accel, "state {state:#04x} accel");
            assert_eq!(i16_at(&exe, off + 2), entry.max_speed, "state {state:#04x} max_speed");
            assert_eq!(i16_at(&exe, off + 4), entry.decel, "state {state:#04x} decel");
            assert_eq!(i16_at(&exe, off + 6), entry.friction_div, "state {state:#04x} friction_div");
            assert_eq!(i16_at(&exe, off + 8), entry.direction, "state {state:#04x} direction");
        }
    }

    #[test]
    fn crash_landing_matches_binary() {
        let exe = exe_or_skip!();
        for (i, &want) in CRASH_LANDING.iter().enumerate() {
            // Table is indexed by absolute state (0x0d..=0x15)
            let got = u32_at(&exe, 0xa434 + (0x0d + i) * 4) as u16;
            assert_eq!(got, want, "state {:#04x}", 0x0d + i);
        }
    }

    #[test]
    fn class_initial_state_matches_binary() {
        let exe = exe_or_skip!();
        for (class, &want) in CLASS_INITIAL_STATE.iter().enumerate() {
            let got = u32_at(&exe, 0xa22c + class * 4) as u16;
            assert_eq!(got, want, "class {class}");
        }
    }

    #[test]
    fn dog_physics_matches_binary() {
        let exe = exe_or_skip!();
        for (i, entry) in DOG_PHYSICS.iter().enumerate() {
            let off = 0xa490 + i * 16;
            assert_eq!(i16_at(&exe, off), entry.accel, "dog state {i} accel");
            assert_eq!(i16_at(&exe, off + 2), entry.max_speed, "dog state {i} max_speed");
            assert_eq!(i16_at(&exe, off + 4), entry.decel, "dog state {i} decel");
            assert_eq!(i16_at(&exe, off + 6), entry.friction_div, "dog state {i} friction_div");
            assert_eq!(i16_at(&exe, off + 8), entry.direction, "dog state {i} direction");
        }
    }

    #[test]
    fn snowboarder_physics_matches_binary() {
        let exe = exe_or_skip!();
        for (i, entry) in SNOWBOARDER_PHYSICS.iter().enumerate() {
            let off = 0xa4e0 + i * 16;
            assert_eq!(i16_at(&exe, off), entry.accel, "sb state {i} accel");
            assert_eq!(i16_at(&exe, off + 2), entry.max_speed, "sb state {i} max_speed");
            assert_eq!(i16_at(&exe, off + 4), entry.decel, "sb state {i} decel");
            assert_eq!(i16_at(&exe, off + 6), entry.friction_div, "sb state {i} friction_div");
            assert_eq!(i16_at(&exe, off + 8), entry.direction, "sb state {i} direction");
        }
    }

    #[test]
    fn mouse_direction_nearly_horizontal_follows_dx() {
        // dy * 4 / dx truncates to 0: right of the skier -> 6, left -> 3.
        assert_eq!(mouse_direction(50, 10), 6);
        assert_eq!(mouse_direction(-50, 10), 3);
    }

    #[test]
    fn mouse_air_direction_quadrants() {
        assert_eq!(mouse_air_direction(-10, -20), 0x10);
        assert_eq!(mouse_air_direction(-20, -10), 0x0e);
        assert_eq!(mouse_air_direction(-20, 10), 0x0e);
        assert_eq!(mouse_air_direction(-5, 10), 0x0d); // -dy (-10) < dx (-5)
        assert_eq!(mouse_air_direction(10, -20), 0x10);
        assert_eq!(mouse_air_direction(20, -10), 0x0f);
        assert_eq!(mouse_air_direction(20, 10), 0x0e);
        assert_eq!(mouse_air_direction(10, 20), 0x0d);
    }

    #[test]
    fn mouse_direction_cardinals() {
        assert_eq!(mouse_direction(0, 10), 0); // straight down
        assert_eq!(mouse_direction(1, 0), 6); // horizontal right (dy<=0)
        assert_eq!(mouse_direction(-1, 0), 3); // horizontal left (dy<=0)
        assert_eq!(mouse_direction(0, -1), 6); // dy<=0, dx>=0
    }
}
