use super::state_table::STATE_TO_SPRITE;
use super::types::Actor;

/// Physics integrator (`FUN_00402be0`)
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

/// Apply steering physics (`FUN_00403430`, faithful port)
///
/// vy is the downhill speed: it accelerates toward `max_speed` and decays by 2
/// when above it. vx is the lateral speed: it is derived from a friction term
/// that couples it to vy, clamped into a window around the previous lateral
/// magnitude, then re-applied along `direction`. All intermediate math is done
/// in i32 and truncated to i16 on store, mirroring the original's int promotion
/// and short stores.
pub fn apply_steering(actor: &mut Actor, accel: i16, max_speed: i16, decel: i16, friction_div: i16, direction: i16) {
    let vx = i32::from(actor.vx);
    let vy = i32::from(actor.vy);

    // Effective direction: table value, or the sign of current vx when the
    // table direction is 0 (skiing straight keeps lateral drift direction).
    let dir: i32 = if direction != 0 { i32::from(direction) } else { vx.signum() };

    // Signed magnitude of lateral velocity along the direction of travel.
    let s4 = dir * vx;

    // Friction term coupling lateral speed to downhill speed.
    let vy_clamped = if vy < 1 { 0 } else { vy };
    let friction = (i32::from(friction_div) * vy_clamped) / 2;

    // Clamp the friction term into the window [s4 - 2, s4 + decel].
    let s2 = if friction < s4 { friction.max(s4 - 2) } else { friction.min(s4 + i32::from(decel)) };

    // Downhill speed accelerates toward max_speed, decaying when above it.
    let new_vy = if i32::from(max_speed) < vy {
        (vy - 2).max(i32::from(max_speed))
    } else {
        (i32::from(accel) + vy).min(i32::from(max_speed))
    };

    actor.vy = new_vy as i16;
    actor.vx = (dir * s2) as i16;
}

/// Change actor state (`FUN_00402120`)
/// Updates sprite from state table
pub fn change_state(actor: &mut Actor, new_state: u16) {
    if actor.state == new_state {
        return;
    }
    if (new_state as usize) < STATE_TO_SPRITE.len() {
        actor.sprite_id = STATE_TO_SPRITE[new_state as usize];
    }
    actor.state = new_state;
}

/// Set actor position (`FUN_00402390`)
pub fn set_position(actor: &mut Actor, x: i16, y: i16, z: i16) {
    actor.x = x;
    actor.y = y;
    actor.z = z;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_actor() -> Actor {
        Actor { active: true, ..Default::default() }
    }

    #[test]
    fn integrate_applies_velocity() {
        let mut a = Actor { active: true, x: 10, y: 20, z: 0, vx: 3, vy: -4, vz: 0, ..Default::default() };
        integrate(&mut a, false);
        assert_eq!((a.x, a.y, a.z), (13, 16, 0));
    }

    #[test]
    fn integrate_fast_forward_doubles_velocity() {
        let mut a = Actor { active: true, x: 10, y: 20, z: 0, vx: 5, vy: 2, vz: 0, ..Default::default() };
        integrate(&mut a, true);
        assert_eq!((a.x, a.y), (20, 24));
    }

    #[test]
    fn gravity_decrements_vz_until_grounded() {
        let mut a = Actor { active: true, z: 10, vz: 3, ..Default::default() };
        integrate(&mut a, false);
        assert_eq!(a.z, 13);
        assert_eq!(a.vz, 2); // gravity pulled it down

        // Falling below ground: z + vz < 1 clamps to z=0, vz=0
        a.z = 0;
        a.vz = -1;
        integrate(&mut a, false);
        assert_eq!(a.z, 0);
        assert_eq!(a.vz, 0);
    }

    #[test]
    fn change_state_updates_sprite() {
        let mut a = test_actor();
        change_state(&mut a, 5);
        assert_eq!(a.state, 5);
        assert_eq!(a.sprite_id, super::super::state_table::STATE_TO_SPRITE[5]);
    }

    #[test]
    fn change_state_noop_when_same() {
        let mut a = test_actor();
        change_state(&mut a, 5);
        a.sprite_id = 99;
        change_state(&mut a, 5);
        assert_eq!(a.sprite_id, 99); // unchanged: same state, no sprite update
    }

    #[test]
    fn downhill_accelerates_toward_max_speed() {
        // State 0 (straight down): accel=1, max_speed=16, no friction.
        let mut a = Actor { active: true, vx: 0, vy: 0, ..Default::default() };
        for expected_vy in 1..=16 {
            apply_steering(&mut a, 1, 16, 0, 0, 0);
            assert_eq!(a.vy, expected_vy, "after {expected_vy} ticks");
            assert_eq!(a.vx, 0);
        }
        // Capped at max_speed.
        apply_steering(&mut a, 1, 16, 0, 0, 0);
        assert_eq!(a.vy, 16);
    }

    #[test]
    fn downhill_decays_by_two_when_above_max_speed() {
        let mut a = Actor { active: true, vx: 0, vy: 20, ..Default::default() };
        apply_steering(&mut a, 1, 16, 0, 0, 0);
        assert_eq!(a.vy, 18);
        apply_steering(&mut a, 1, 16, 0, 0, 0);
        assert_eq!(a.vy, 16); // clamps to max_speed once vy-2 < max_speed
    }

    #[test]
    fn full_turn_scrubs_downhill_speed_to_zero() {
        // State 3 (hard left): max_speed=0, direction=-1, friction_div=8.
        let mut a = Actor { active: true, vx: -10, vy: 8, ..Default::default() };
        apply_steering(&mut a, 1, 0, 1, 8, -1);
        assert_eq!(a.vy, 6); // vy - 2

        // Keep scrubbing until vy reaches 0.
        while a.vy > 0 {
            apply_steering(&mut a, 1, 0, 1, 8, -1);
        }
        assert_eq!(a.vy, 0);
    }

    #[test]
    fn lateral_velocity_keeps_direction_sign() {
        // State 6 (hard right): direction=+1.
        let mut a = Actor { active: true, vx: 10, vy: 8, ..Default::default() };
        apply_steering(&mut a, 1, 0, 1, 8, 1);
        assert!(a.vx >= 0, "right turn keeps vx non-negative, got {}", a.vx);
    }
}
