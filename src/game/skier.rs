use super::physics;
use super::state_table::*;
use super::types::*;

/// Skier state machine (FUN_004028e0)
/// States 0x00-0x15: skiing, jumping, crashing
pub fn update_skier(actor: &mut Actor, fast_forward: bool) {
    let state = actor.state;

    if state == 0x0b {
        // Crashed — decelerate to stop
        if actor.z == 0 && actor.vz == 0 {
            if actor.vx == 0 && actor.vy == 0 {
                physics::change_state(actor, 0x0c);
                return;
            }
            // Decelerate
            actor.vx = decelerate(actor.vx);
            actor.vy = decelerate(actor.vy);
        }
        physics::integrate(actor, fast_forward);
        return;
    }

    physics::integrate(actor, fast_forward);

    // Get physics params for current state
    if (state as usize) < SKIER_PHYSICS.len() {
        let entry = &SKIER_PHYSICS[state as usize];
        physics::apply_steering(
            actor,
            entry.accel,
            entry.max_speed,
            entry.decel,
            entry.friction_div,
            entry.direction,
        );
    }

    // State transitions for skiing states
    match state {
        7 | 9 => physics::change_state(actor, 3),
        8 | 10 => physics::change_state(actor, 6),
        0x0d..=0x15 => {
            // Jump states — check landing
            if actor.z == 0 {
                let landing = CRASH_LANDING[(state - 0x0d) as usize];
                physics::change_state(actor, landing);
            }
        }
        _ => {}
    }
}

/// Handle keyboard steering input (FUN_00406170)
pub fn steer_keyboard(actor: &mut Actor, left: bool, right: bool, up: bool, down: bool) {
    if actor.state == 0x0b || actor.state == 0x11 {
        return; // crashed or jumping
    }

    let current = actor.state as usize;
    if current >= DIRECTION_TRANSITION.len() {
        return;
    }

    if left {
        let new_dir = DIRECTION_TRANSITION[current] as usize;
        if new_dir < DIRECTION_TRANSITION.len() {
            physics::change_state(actor, DIRECTION_TRANSITION[new_dir]);
        }
    }
    if right {
        // Right turn: mirror of left
        let mirrored = mirror_direction(actor.state);
        physics::change_state(actor, mirrored);
    }
    if up {
        // Slow down / stop
        if actor.state == 3 || actor.state == 7 || actor.state == 0x0c {
            physics::change_state(actor, 9);
        } else if actor.state == 6 || actor.state == 8 {
            physics::change_state(actor, 10);
        }
    }
    if down {
        // Speed up / tuck
        if actor.state == 9 {
            physics::change_state(actor, 3);
        } else if actor.state == 10 {
            physics::change_state(actor, 6);
        }
    }
}

/// Handle mouse steering input (FUN_00406550 + FUN_004065e0)
pub fn steer_mouse(actor: &mut Actor, dx: i16, dy: i16) {
    if actor.state == 0x0b || actor.state == 0x11 {
        return;
    }
    let dir = mouse_direction(dx, dy);
    physics::change_state(actor, dir as u16);
}

fn decelerate(v: i16) -> i16 {
    if v > 0 {
        v - 1
    } else if v < 0 {
        v + 1
    } else {
        0
    }
}

fn mirror_direction(state: u16) -> u16 {
    match state {
        1 => 4,
        2 => 5,
        3 => 6,
        4 => 1,
        5 => 2,
        6 => 3,
        7 => 8,
        8 => 7,
        _ => state,
    }
}
