use super::physics;
use super::sound::Sound;
use super::state_table::{
    mouse_air_direction, mouse_direction, CRASH_LANDING, LEFT_TRANSITION, RIGHT_TRANSITION, SKIER_PHYSICS,
};
use super::types::Actor;

/// Skier state machine (`FUN_004028e0`)
/// States 0x00-0x15: skiing, jumping, crashing.
///
/// Returns the style points earned this tick (the caller adds them through
/// `Scoring::add`, which only counts inside freestyle, like `FUN_00403420`).
/// A landing pushes its sound to `sounds`.
pub fn update_skier(actor: &mut Actor, fast_forward: bool, sounds: &mut Vec<Sound>) -> i32 {
    let state = actor.state;

    if state == 0x0b {
        // Crashed (FUN_004028e0, state 0xb): the velocities step one unit
        // toward zero each tick, but the integrator is NOT run — the skier
        // stays put while "decelerating" — and once both are zero the state
        // becomes 0xc (sitting in the snow). The original asserts z == 0 and
        // vz == 0 here; integrate defensively if a crash ever starts airborne.
        if actor.z == 0 && actor.vz == 0 {
            if actor.vx == 0 && actor.vy == 0 {
                physics::change_state(actor, 0x0c);
                return 0;
            }
            actor.vx = decelerate(actor.vx);
            actor.vy = decelerate(actor.vy);
        } else {
            physics::integrate(actor, fast_forward);
        }
        return 0;
    }

    physics::integrate(actor, fast_forward);

    // Get physics params for current state
    if (state as usize) < SKIER_PHYSICS.len() {
        let entry = &SKIER_PHYSICS[state as usize];
        physics::apply_steering(actor, entry.accel, entry.max_speed, entry.decel, entry.friction_div, entry.direction);
    }

    // State transitions for skiing states
    let mut points = 0;
    match state {
        7 | 9 => physics::change_state(actor, 3),
        8 | 10 => physics::change_state(actor, 6),
        // Jump states — check landing
        0x0d..=0x15 if actor.z == 0 => {
            let landing = CRASH_LANDING[(state - 0x0d) as usize];
            // The original's branch, kept as is: its landing table has no
            // 0x11 entry, so it never runs (every landing plays sound 4).
            if landing == 0x11 {
                points -= 64; // botched landing (0x402ad5)
                sounds.push(Sound::Crash);
            } else {
                sounds.push(Sound::Land);
            }
            physics::change_state(actor, landing);
        }
        _ => {}
    }

    // Style for the pose held this tick, by the new state (byte table at
    // 0x402b88, verified via Frida): tricks score every tick in the air.
    points += match actor.state {
        7..=10 => -1,
        0x10 => 2,
        0x12 | 0x13 => 4,
        0x14 | 0x15 => 8,
        _ => 0,
    };
    points
}

/// A cardinal steering key (arrows or numpad 2/4/6/8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrowKey {
    Left,
    Right,
    Up,
    Down,
}

/// Handle one keyboard steering key-down (`FUN_00406170`, faithful port)
///
/// The original handles each cardinal key as a discrete event:
/// - DOWN (ground): straight down (state 0) — the fastest descent.
/// - UP (ground): snowplow brake into states 9/10 with vy = -4.
/// - LEFT/RIGHT: step through the left/right transition fan (0→1→2→3→7 and
///   0→4→5→6→8); the hard-carve states 7/8 also kick lateral velocity to ±8.
/// - In the air, UP/DOWN cycle the jump-trick poses instead of steering.
pub fn steer_keyboard(actor: &mut Actor, key: ArrowKey) {
    if actor.state == 0x0b || actor.state == 0x11 {
        return; // crashed or jumping
    }

    match key {
        ArrowKey::Down => steer_down(actor),
        ArrowKey::Up => steer_up(actor),
        ArrowKey::Left => steer_left(actor),
        ArrowKey::Right => steer_right(actor),
    }
}

/// DOWN: straight down on the ground; in the air, the next trick pose.
fn steer_down(actor: &mut Actor) {
    if actor.z == 0 {
        physics::change_state(actor, 0);
    } else {
        let next = match actor.state {
            0x0d => 0x13,
            0x12 => 0x0d,
            0x13 => 0x12,
            0x14 => 0x0e,
            0x15 => 0x0f,
            _ => actor.state,
        };
        physics::change_state(actor, next);
    }
}

/// UP: the snowplow brake from a hard turn; in the air, the next trick pose.
fn steer_up(actor: &mut Actor) {
    match actor.state {
        3 | 7 | 0x0c => {
            if actor.vy == 0 {
                physics::change_state(actor, 9);
                actor.vy = -4;
            }
        }
        6 | 8 => {
            if actor.vy == 0 {
                physics::change_state(actor, 10);
                actor.vy = -4;
            }
        }
        0x0d => physics::change_state(actor, 0x12),
        0x0e => physics::change_state(actor, 0x14),
        0x0f => physics::change_state(actor, 0x15),
        0x12 => physics::change_state(actor, 0x13),
        0x13 => physics::change_state(actor, 0x0d),
        _ => {}
    }
}

/// LEFT: one step through the left fan; the hard carve (7) kicks vx left.
fn steer_left(actor: &mut Actor) {
    if let Some(&next) = LEFT_TRANSITION.get(actor.state as usize) {
        physics::change_state(actor, next);
        if next == 7 {
            actor.vx = (actor.vx - 8).max(-8);
        }
    }
}

/// RIGHT: one step through the right fan; the hard carve (8) kicks vx right.
fn steer_right(actor: &mut Actor) {
    if let Some(&next) = RIGHT_TRANSITION.get(actor.state as usize) {
        physics::change_state(actor, next);
        if next == 8 {
            actor.vx = (actor.vx + 8).min(8);
        }
    }
}

/// Keyboard jump (Insert / numpad0, `FUN_00406170` case 0x2d/0x60).
/// Smaller hop than the mouse (vz=2) that also scrubs downhill speed.
pub fn jump_keyboard(actor: &mut Actor) {
    if actor.state == 0x0b || actor.state == 0x11 {
        return; // crashed or mid-flip
    }
    if actor.z == 0 {
        actor.vz = 2;
        physics::change_state(actor, 0x0d);
        if actor.vy > 4 {
            actor.vy -= 4;
        }
    }
}

/// Diagonal steering (`FUN_00406170`): set a skiing state directly on the ground.
pub fn steer_absolute(actor: &mut Actor, target_state: u16) {
    if actor.state == 0x0b || actor.state == 0x11 {
        return;
    }
    if actor.z == 0 {
        physics::change_state(actor, target_state);
    }
}

/// Mouse steering (`FUN_00406550`): `dx`/`dy` are the cursor's offset from
/// the skier's screen anchor (client w/2, h/3). Ignored while crashed (0xb)
/// or crash-landing (0x11); on the ground the slope picks a skiing
/// direction (`FUN_004065e0`), in the air a jump pose (`FUN_00406670`).
pub fn steer_mouse(actor: &mut Actor, dx: i16, dy: i16) {
    if actor.state == 0x0b || actor.state == 0x11 {
        return;
    }
    let dir = if actor.z == 0 { mouse_direction(dx, dy) } else { mouse_air_direction(dx, dy) };
    physics::change_state(actor, dir);
}

/// One step toward 0.
/// Click (`FUN_004066d0`): jump on the ground, cycle the trick poses in the
/// air; nothing while crashed or after a botched landing.
pub fn click(actor: &mut Actor) {
    if actor.state == 0x0b {
        return; // crashed
    }
    if actor.z == 0 {
        actor.vz = 4; // jump (mouse uses vz=4, unlike keyboard's 2)
        physics::change_state(actor, 0x0d);
    } else if actor.state != 0x11 {
        let next = match actor.state {
            0x0d => 0x12,
            0x0e => 0x14,
            0x0f => 0x15,
            0x12 => 0x13,
            0x13 => 0x0d,
            _ => actor.state,
        };
        physics::change_state(actor, next);
    }
}

fn decelerate(v: i16) -> i16 {
    v - v.signum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crashed_skier_decelerates_in_place_then_sits() {
        let mut a = Actor { active: true, state: 0x0b, x: 5, y: 100, vx: 1, vy: 2, ..Default::default() };
        update_skier(&mut a, false, &mut Vec::new());
        assert_eq!((a.x, a.y, a.vx, a.vy, a.state), (5, 100, 0, 1, 0x0b));
        update_skier(&mut a, false, &mut Vec::new());
        assert_eq!((a.x, a.y, a.vx, a.vy, a.state), (5, 100, 0, 0, 0x0b));
        update_skier(&mut a, false, &mut Vec::new());
        assert_eq!((a.y, a.state), (100, 0x0c));
    }
}
