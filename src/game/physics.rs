use super::state_table::STATE_TO_SPRITE;
use super::types::*;

/// Physics integrator (FUN_00402be0)
/// position += velocity per axis; gravity decrements z
pub fn integrate(actor: &mut Actor, fast_forward: bool) {
    actor.x = actor.x.wrapping_add(actor.vx);
    actor.y = actor.y.wrapping_add(actor.vy);
    actor.z = actor.z.wrapping_add(actor.vz);
    if fast_forward {
        actor.x = actor.x.wrapping_add(actor.vx);
        actor.y = actor.y.wrapping_add(actor.vy);
        actor.z = actor.z.wrapping_add(actor.vz);
    }
    if actor.z > 0 {
        actor.vz -= 1; // gravity
    } else {
        actor.z = 0;
        actor.vz = 0;
    }
}

/// Apply steering physics (FUN_00403430)
/// entry: physics parameters for the current state
pub fn apply_steering(actor: &mut Actor, accel: i16, max_speed: i16, decel: i16, friction_div: i16, direction: i16) {
    let mut vx = actor.vx;
    let mut vy = actor.vy;

    // Apply direction-based acceleration
    if direction != 0 {
        vx = vx.wrapping_add(direction * accel);
    }

    // Clamp to max speed
    if max_speed > 0 {
        if vx > max_speed {
            vx = max_speed;
        } else if vx < -max_speed {
            vx = -max_speed;
        }
    }

    // Apply friction/deceleration
    if decel > 0 && friction_div > 0 {
        let friction = vy / friction_div;
        vy = vy.wrapping_sub(friction);
    }

    actor.vx = vx;
    actor.vy = vy;
}

/// Change actor state (FUN_00402120)
/// Updates sprite from state table
pub fn change_state(actor: &mut Actor, new_state: u16) {
    if actor.state == new_state {
        return;
    }
    if (new_state as usize) < STATE_TO_SPRITE.len() {
        actor.sprite_id = STATE_TO_SPRITE[new_state as usize];
    }
    actor.state = new_state;
    actor.flags.new_sprite = true;
}

/// Set actor position (FUN_00402390)
pub fn set_position(actor: &mut Actor, x: i16, y: i16, z: i16) {
    actor.x = x;
    actor.y = y;
    actor.z = z;
}
