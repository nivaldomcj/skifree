use super::types::*;

/// Generate spawn event lists for all course types (FUN_00404b50)
pub fn generate_course(course_type: CourseType, screen_w: i16, screen_h: i16, player_x: i16, player_y: i16) -> Vec<SpawnEvent> {
    let mut events = Vec::new();

    match course_type {
        CourseType::Freestyle => generate_freestyle(&mut events, screen_w, screen_h, player_x, player_y),
        CourseType::Slalom => generate_slalom(&mut events, screen_w, screen_h, player_x, player_y),
        CourseType::TreeSlalom => generate_tree_slalom(&mut events, screen_w, screen_h, player_x, player_y),
    }

    // Add ambient mob spawns (shared across all courses)
    generate_mob_spawns(&mut events);

    // Add monster spawn events (shared)
    generate_monster_spawns(&mut events);

    events
}

fn generate_freestyle(events: &mut Vec<SpawnEvent>, screen_w: i16, screen_h: i16, player_x: i16, player_y: i16) {
    // Start marker
    let start_x = (player_x - screen_w / 2).wrapping_add(0x3c).wrapping_add(player_x);
    let start_y = (player_y - screen_h / 2).wrapping_add(player_y).wrapping_sub(0x3c);
    events.push(SpawnEvent {
        actor_idx: None, event_type: 0x11, state: 0x3d, class: 0,
        x: clamp_i16(start_x, -320, 320), y: clamp_i16(start_y, 0, 0x208),
        z: 0, param: 0,
    });

    // Gates from 0x3c0 to 0x21c0, alternating left/right
    let mut y = 0x3c0i16;
    let mut left = true;
    while y < 0x21c0 {
        let x = if left { -96 } else { -32 }; // -(bVar1 & 0xffa0) - 400
        events.push(SpawnEvent {
            actor_idx: None, event_type: 0x0c, state: if left { 0x17 } else { 0x18 }, class: 0,
            x, y, z: 0, param: 0,
        });
        left = !left;
        y = y.wrapping_add(0x140);
    }

    // Finish markers
    events.push(SpawnEvent {
        actor_idx: None, event_type: 0x11, state: 0x3b, class: 0,
        x: -576, y: 0x21c0, z: 0, param: 0,
    });
    events.push(SpawnEvent {
        actor_idx: None, event_type: 0x11, state: 0x3c, class: 0,
        x: -320, y: 0x21c0, z: 0, param: 0,
    });
}

fn generate_slalom(events: &mut Vec<SpawnEvent>, screen_w: i16, screen_h: i16, player_x: i16, player_y: i16) {
    // Start marker
    let start_x = (player_x - screen_w / 2).wrapping_sub(0x3c).wrapping_add(player_x);
    let start_y = (player_y - screen_h / 2).wrapping_add(player_y).wrapping_sub(0x3c);
    events.push(SpawnEvent {
        actor_idx: None, event_type: 0x11, state: 0x3e, class: 0,
        x: clamp_i16(start_x, 0x140, 0x140), y: clamp_i16(start_y, 0, 0x208),
        z: 0, param: 0,
    });

    // Gates from 0x410 to 0x4100, alternating left/right with random obstacles
    let mut y = 0x410i16;
    let mut left = true;
    while y < 0x4100 {
        let x = if left { -32 } else { 432 }; // (-(bVar1 & 0xffe0) + 0x1b0)
        events.push(SpawnEvent {
            actor_idx: None, event_type: 0x0c, state: if left { 0x17 } else { 0x18 }, class: 0,
            x, y, z: 0, param: 0,
        });
        left = !left;
        y = y.wrapping_add(400);
    }

    // Finish markers
    events.push(SpawnEvent {
        actor_idx: None, event_type: 0x11, state: 0x3b, class: 0,
        x: 0x140, y: 0x4100, z: 0, param: 0,
    });
    events.push(SpawnEvent {
        actor_idx: None, event_type: 0x11, state: 0x3c, class: 0,
        x: 0x200, y: 0x4100, z: 0, param: 0,
    });
}

fn generate_tree_slalom(events: &mut Vec<SpawnEvent>, _screen_w: i16, screen_h: i16, _player_x: i16, player_y: i16) {
    // Start marker
    let start_y = (player_y - screen_h / 2).wrapping_add(player_y).wrapping_sub(0x3c);
    events.push(SpawnEvent {
        actor_idx: None, event_type: 0x11, state: 0x3f, class: 0,
        x: 0, y: clamp_i16(start_y, 0, 0x208), z: 0, param: 0,
    });

    // Dense tree obstacles at start and finish
    events.push(SpawnEvent {
        actor_idx: None, event_type: 0x11, state: 0x39, class: 0,
        x: -160, y: 0x280, z: 0, param: 0,
    });
    events.push(SpawnEvent {
        actor_idx: None, event_type: 0x11, state: 0x3a, class: 0,
        x: 0xa0, y: 0x280, z: 0, param: 0,
    });
    events.push(SpawnEvent {
        actor_idx: None, event_type: 0x11, state: 0x3b, class: 0,
        x: -160, y: 0x4100, z: 0, param: 0,
    });
    events.push(SpawnEvent {
        actor_idx: None, event_type: 0x11, state: 0x3c, class: 0,
        x: 0xa0, y: 0x4100, z: 0, param: 0,
    });
}

/// Ambient mob spawner records (FUN_00404b50): pairs of class-4 proxies every
/// 0x800 units of y, floating at z = 0x20. State 0x27 drifts up (vy = -2) at
/// x = -112; state 0x29 drifts down (vy = +2) at x = -144. `param` carries
/// the initial vy for the port's merged actor.
fn generate_mob_spawns(events: &mut Vec<SpawnEvent>) {
    let mut y = -0x400i16;
    while y < 0x5c01 {
        // Up-drifting mob (all but the first y)
        if y > -0x400 {
            events.push(SpawnEvent {
                actor_idx: None, event_type: 0x04, state: 0x27, class: 4,
                x: -112, y, z: 0x20, param: -2,
            });
        }
        // Down-drifting mob (all but the last y)
        if y < 0x5c00 {
            events.push(SpawnEvent {
                actor_idx: None, event_type: 0x04, state: 0x29, class: 4,
                x: -144, y, z: 0x20, param: 2,
            });
        }
        y = y.wrapping_add(0x800);
    }
}

fn generate_monster_spawns(events: &mut Vec<SpawnEvent>) {
    // Monster from top (type 7)
    events.push(SpawnEvent {
        actor_idx: None, event_type: 0x07, state: 0x2a, class: 7,
        x: -16060, y: 0, z: 0, param: 0,
    });
    // Monster from bottom (type 8)
    events.push(SpawnEvent {
        actor_idx: None, event_type: 0x08, state: 0x2a, class: 8,
        x: 16060, y: 0, z: 0, param: 0,
    });
    // Monster from left (type 5)
    events.push(SpawnEvent {
        actor_idx: None, event_type: 0x05, state: 0x2a, class: 5,
        x: 0, y: -2060, z: 0, param: 0,
    });
    // Monster from right (type 6)
    events.push(SpawnEvent {
        actor_idx: None, event_type: 0x06, state: 0x2a, class: 6,
        x: 0, y: 32060, z: 0, param: 0,
    });
}

fn clamp_i16(v: i16, min: i16, max: i16) -> i16 {
    if v < min { min } else if v > max { max } else { v }
}
