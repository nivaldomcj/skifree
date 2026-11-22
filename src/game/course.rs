use super::types::*;

/// Course strip x-ranges (world units), from FUN_00402c60 / FUN_00403250 /
/// FUN_00403180 and the zone tests in FUN_004025c0. Identified by the
/// banner sprites: 61 "Slalom ↙" points at the left strip, 62 "Tree Slalom
/// ↘" at the right strip, 63 "Free style ↓" at the center strip.
pub const SLALOM_STRIP: (i16, i16) = (-0x240, -0x140); // -576..-320
pub const TREE_SLALOM_STRIP: (i16, i16) = (0x140, 0x200); // 320..512
pub const FREESTYLE_STRIP: (i16, i16) = (-0xa0, 0xa0); // -160..160

pub const COURSE_START_Y: i16 = 0x280;
pub const SLALOM_FINISH_Y: i16 = 0x21c0;
pub const LONG_FINISH_Y: i16 = 0x4100; // tree slalom and freestyle

/// Generate every spawn record for a new game (FUN_00404b50).
///
/// The original builds all three courses at once — slalom on the left
/// strip, freestyle in the center, tree slalom on the right — plus the lift
/// pole line, ambient mob proxies and the four boundary monsters. The
/// `left`/`top` arguments are the camera viewport origin at game start
/// (player anchored at w/2, h/3), used by the "↓" banner formulas.
pub fn generate_course(screen_w: i16, screen_h: i16, rng: &mut super::rng::Rng) -> Vec<SpawnEvent> {
    let mut events = Vec::new();
    let left = -screen_w / 2;
    let top = -screen_h / 3;

    generate_slalom(&mut events, left, top, screen_h);
    generate_tree_slalom(&mut events, left, top, screen_w, screen_h, rng);
    generate_freestyle(&mut events, top, screen_h);
    generate_lift_poles(&mut events);
    generate_mob_spawns(&mut events);
    generate_monster_spawns(&mut events);

    events
}

/// The "↓" banner y used by all three courses: one screen down minus 0x3c,
/// clamped to 0x208 when that lands past 0x280.
fn banner_y(top: i16, screen_h: i16) -> i16 {
    let y = top + screen_h - 0x3c;
    if y > 0x280 {
        0x208
    } else {
        y
    }
}

fn marker(events: &mut Vec<SpawnEvent>, sprite: u16, x: i16, y: i16) {
    events.push(SpawnEvent {
        actor_idx: None,
        event_type: 0x11,
        state: 0,
        class: 0x11,
        sprite: Some(sprite),
        x,
        y,
        z: 0,
        param: 0,
    });
}

fn gate(events: &mut Vec<SpawnEvent>, sprite: u16, x: i16, y: i16) {
    events.push(SpawnEvent {
        actor_idx: None,
        event_type: 0x0c,
        state: 0,
        class: 0x0c,
        sprite: Some(sprite),
        x,
        y,
        z: 0,
        param: 0,
    });
}

/// Slalom (left strip): short timed course with flag gates.
fn generate_slalom(events: &mut Vec<SpawnEvent>, left: i16, top: i16, screen_h: i16) {
    // "Slalom ↙" banner: x = viewport_left + 0x3c, clamped to >= -0x140.
    let bx = (left + 0x3c).max(-0x140);
    marker(events, 0x3d, bx, banner_y(top, screen_h));

    // Start posts.
    marker(events, 0x39, -576, COURSE_START_Y);
    marker(events, 0x3a, -320, COURSE_START_Y);

    // Gate flags: alternate sprite 0x17 (pass right) / 0x18 (pass left),
    // x = -496 / -400, from 0x3c0 to just below the finish.
    let mut first = true;
    let mut y = 0x3c0i16;
    while y < 0x21c0 {
        let (sprite, x) = if first { (0x17, -496) } else { (0x18, -400) };
        gate(events, sprite, x, y);
        first = !first;
        y = y.wrapping_add(0x140);
    }

    // Finish posts.
    marker(events, 0x3b, -576, SLALOM_FINISH_Y);
    marker(events, 0x3c, -320, SLALOM_FINISH_Y);
}

/// Tree slalom (right strip): long timed course, gates plus a random tree
/// per gate row.
fn generate_tree_slalom(events: &mut Vec<SpawnEvent>, left: i16, top: i16, screen_w: i16, screen_h: i16, rng: &mut super::rng::Rng) {
    // "Tree Slalom ↘" banner: x = viewport_left + client_w - 0x3c, clamped to <= 0x140.
    let bx = (left + screen_w - 0x3c).min(0x140);
    marker(events, 0x3e, bx, banner_y(top, screen_h));

    // Start posts.
    marker(events, 0x39, 0x140, COURSE_START_Y);
    marker(events, 0x3a, 0x200, COURSE_START_Y);

    // Gate flags at x = 400 / 432, plus a random tree per row (and a
    // discarded rand(400) per row — RNG consumption order is faithful).
    let mut first = true;
    let mut y = 0x410i16;
    while y < 0x4100 {
        let (sprite, x) = if first { (0x17, 400) } else { (0x18, 432) };
        gate(events, sprite, x, y);
        first = !first;

        let tree_sprite = match rng.rand_range(8) {
            0 => 0x32,
            1 => 0x33,
            _ => 0x31,
        };
        let tree_x = rng.rand_range(0x20) as i16 + 400;
        rng.rand_range(400);
        events.push(SpawnEvent {
            actor_idx: None,
            event_type: 0x0d,
            state: 0,
            class: 0x0d,
            sprite: Some(tree_sprite),
            x: tree_x,
            y,
            z: 0,
            param: 0,
        });

        y = y.wrapping_add(400);
    }

    // Finish posts.
    marker(events, 0x3b, 0x140, LONG_FINISH_Y);
    marker(events, 0x3c, 0x200, LONG_FINISH_Y);
}

/// Freestyle (center strip): long style-scored course, no gates.
fn generate_freestyle(events: &mut Vec<SpawnEvent>, top: i16, screen_h: i16) {
    // "Free style ↓" banner.
    marker(events, 0x3f, 0, banner_y(top, screen_h));

    // Start / finish posts, no gates.
    marker(events, 0x39, -160, COURSE_START_Y);
    marker(events, 0x3a, 160, COURSE_START_Y);
    marker(events, 0x3b, -160, LONG_FINISH_Y);
    marker(events, 0x3c, 160, LONG_FINISH_Y);
}

/// Lift pole line (DAT_0040c738 records): class 0xd scenery with the pole
/// sprite 0x40 at x = -128, every 0x800 across the slope.
fn generate_lift_poles(events: &mut Vec<SpawnEvent>) {
    let mut y = -0x400i16;
    while y < 0x5c01 {
        events.push(SpawnEvent {
            actor_idx: None,
            event_type: 0x0d,
            state: 0,
            class: 0x0d,
            sprite: Some(0x40),
            x: -128,
            y,
            z: 0,
            param: 0,
        });
        y = y.wrapping_add(0x800);
    }
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
                actor_idx: None,
                event_type: 0x04,
                state: 0x27,
                class: 4,
                sprite: None,
                x: -112,
                y,
                z: 0x20,
                param: -2,
            });
        }
        // Down-drifting mob (all but the last y)
        if y < 0x5c00 {
            events.push(SpawnEvent {
                actor_idx: None,
                event_type: 0x04,
                state: 0x29,
                class: 4,
                sprite: None,
                x: -144,
                y,
                z: 0x20,
                param: 2,
            });
        }
        y = y.wrapping_add(0x800);
    }
}

fn generate_monster_spawns(events: &mut Vec<SpawnEvent>) {
    // Monster from top (type 7)
    events.push(SpawnEvent {
        actor_idx: None, event_type: 0x07, state: 0x2a, class: 7, sprite: None,
        x: -16060, y: 0, z: 0, param: 0,
    });
    // Monster from bottom (type 8)
    events.push(SpawnEvent {
        actor_idx: None, event_type: 0x08, state: 0x2a, class: 8, sprite: None,
        x: 16060, y: 0, z: 0, param: 0,
    });
    // Monster from left (type 5)
    events.push(SpawnEvent {
        actor_idx: None, event_type: 0x05, state: 0x2a, class: 5, sprite: None,
        x: 0, y: -2060, z: 0, param: 0,
    });
    // Monster from right (type 6)
    events.push(SpawnEvent {
        actor_idx: None, event_type: 0x06, state: 0x2a, class: 6, sprite: None,
        x: 0, y: 32060, z: 0, param: 0,
    });
}
