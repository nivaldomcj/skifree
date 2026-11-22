/// State-to-sprite mapping table (DAT_0040a1ac in the original binary)
pub const STATE_TO_SPRITE: [u16; 64] = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, // 0x00-0x0f
    17, 18, 19, 20, 21, 22, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, // 0x10-0x1f
    38, 39, 41, 42, 43, 44, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, // 0x20-0x2f
    75, 76, 77, 78, 79, 80, 81, 83, 84, 85, 84, 49, 87, 88, 89, 89, // 0x30-0x3f
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

/// Dog physics table (DAT_0040a490), states 0x16-0x1a
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

/// Skier direction transition table (DAT_0040a258)
/// Maps current direction to new direction on left/right input
pub const DIRECTION_TRANSITION: [u16; 22] = [1, 4, 2, 0, 3, 1, 7, 2, 0, 5, 4, 6, 5, 8, 3, 2, 5, 6, 9, 2, 5, 10];

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
    } else {
        3
    }
}
