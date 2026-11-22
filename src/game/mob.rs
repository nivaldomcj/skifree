use super::physics;
use super::rng::Rng;
use super::state_table::*;
use super::types::*;

/// Dog behavior (FUN_00403540, class 1)
pub fn update_dog(actor: &mut Actor, rng: &mut Rng, fast_forward: bool) {
    let state = actor.state;
    if state > 0x1a {
        return;
    }

    physics::integrate(actor, fast_forward);

    if (state as usize) >= 0x16 && (state as usize) < 0x16 + DOG_PHYSICS.len() {
        let entry = &DOG_PHYSICS[state as usize - 0x16];
        physics::apply_steering(actor, entry.accel, entry.max_speed, entry.decel, entry.friction_div, entry.direction);
    }

    // Random direction change (1-in-12)
    if rng.rand_range(12) == 0 {
        match rng.rand_range(3) {
            0 => physics::change_state(actor, 0x16),
            1 => physics::change_state(actor, 0x17),
            _ => physics::change_state(actor, 0x18),
        }
    }
}

/// Snowboarder behavior (FUN_00403610, class 2)
pub fn update_snowboarder(actor: &mut Actor, rng: &mut Rng, fast_forward: bool) {
    match actor.state {
        0x1b => {
            actor.vx = (rng.rand_range(3) as i16).wrapping_sub(1);
            physics::integrate(actor, fast_forward);
            physics::change_state(actor, 0x1c);
        }
        0x1c => {
            actor.vy = 4;
            physics::integrate(actor, fast_forward);
            physics::change_state(actor, 0x1b);
        }
        0x1d => {
            actor.vx = 0;
            actor.vy = 0;
            physics::integrate(actor, fast_forward);
            let r = rng.rand_range(0x20);
            physics::change_state(actor, if r != 0 { 0x1b } else { 0x1e });
        }
        0x1e => {
            if rng.rand_range(100) != 0 {
                physics::integrate(actor, fast_forward);
                physics::change_state(actor, 0x1d);
                return;
            }
            // Jump — spawn a jump effect actor
            physics::integrate(actor, fast_forward);
        }
        _ => {
            physics::integrate(actor, fast_forward);
        }
    }
}

/// Ambient mob behavior (FUN_00404290, class 4)
/// Crosses the slope horizontally, wraps at boundaries
pub fn update_ambient(actor: &mut Actor, rng: &mut Rng, fast_forward: bool) {
    // Wrap at y boundaries
    if actor.y < -1024 {
        physics::change_state(actor, 0x29);
        actor.vx = 2;
        actor.x = -112; // 0xff90 as i16
        return;
    }
    if actor.y > 23551 {
        physics::change_state(actor, 0x27);
        actor.vx = -2; // 0xfffe as i16
        actor.x = -128; // 0xff80 as i16
        return;
    }

    physics::integrate(actor, fast_forward);

    // 1-in-1000 chance to spawn a secondary actor behind
    if actor.next.is_some() && actor.state == 0x27 {
        if rng.rand_range(1000) == 0 {
            physics::change_state(actor, 0x28);
        }
    }
}

/// Lift chair behavior (FUN_004037b0, class 10)
pub fn update_lift(actor: &mut Actor, rng: &mut Rng, fast_forward: bool) {
    match actor.state {
        0x3c => {
            if actor.vx != 0 {
                physics::integrate(actor, fast_forward);
                return;
            }
            if rng.rand_range(100) == 0 {
                actor.vx = (rng.rand_range(2) as i16) * 2 - 1;
                physics::integrate(actor, fast_forward);
                physics::change_state(actor, 0x3d);
                return;
            }
            physics::integrate(actor, fast_forward);
        }
        0x3d => {
            if actor.vx == 0 {
                physics::integrate(actor, fast_forward);
                return;
            }
            if rng.rand_range(10) == 0 {
                actor.vx = 0;
                physics::integrate(actor, fast_forward);
                physics::change_state(actor, 0x3c);
                return;
            }
            physics::integrate(actor, fast_forward);
            physics::change_state(actor, if actor.vx > 0 { 0x3f } else { 0x3e });
        }
        0x3e | 0x3f => {
            physics::integrate(actor, fast_forward);
            physics::change_state(actor, 0x3d);
        }
        _ => {
            physics::integrate(actor, fast_forward);
        }
    }
}

/// Obstacle behavior (FUN_00403910, class 3)
pub fn update_obstacle(actor: &mut Actor, rng: &mut Rng, fast_forward: bool) {
    let state = actor.state;
    if state < 0x1f || state > 0x26 {
        return;
    }

    physics::integrate(actor, fast_forward);

    if (state as usize) >= 0x1f && (state as usize) < 0x1f + SNOWBOARDER_PHYSICS.len() {
        let entry = &SNOWBOARDER_PHYSICS[state as usize - 0x1f];
        physics::apply_steering(actor, entry.accel, entry.max_speed, entry.decel, entry.friction_div, entry.direction);
    }

    // State transitions
    if state == 0x1f {
        if rng.rand_range(10) == 0 {
            physics::change_state(actor, 0x20);
        }
    } else if state == 0x20 {
        if rng.rand_range(10) == 0 {
            physics::change_state(actor, 0x1f);
        }
    } else if state == 0x21 {
        if actor.z == 0 {
            physics::change_state(actor, 0x20);
        }
    } else if state >= 0x22 && state <= 0x26 {
        let next = state + 1;
        if next == 0x27 {
            physics::change_state(actor, 0x20);
        } else {
            physics::change_state(actor, next);
        }
    }
}

/// Eating behavior (FUN_00403750, class 9)
pub fn update_eating(actor: &mut Actor, _fast_forward: bool) {
    let next = actor.state + 1;
    if next > 0x3b {
        physics::change_state(actor, 0x38);
    } else {
        physics::change_state(actor, next);
    }
}
