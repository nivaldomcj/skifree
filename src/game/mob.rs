use super::physics;
use super::rng::Rng;
use super::sound::Sound;
use super::state_table::{DOG_PHYSICS, SNOWBOARDER_PHYSICS};
use super::types::{Actor, ActorClass, SpawnRequest};

/// Dog behavior (`FUN_00403540`, class 1)
pub fn update_dog(actor: &mut Actor, rng: &mut Rng, fast_forward: bool) {
    let state = actor.state;
    if state > 0x18 {
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

/// Snowboarder behavior (`FUN_00403610`, class 2)
///
/// State 0x1e can trigger the jump: a class-0x11 effect actor (sprite 0x52)
/// is spawned at (x-4, y-2, z) from the pre-integration position, and the
/// snowboarder continues in state 0x1b, with the jump sound (`FUN_00402ba0`).
pub fn update_snowboarder(
    actor: &mut Actor,
    rng: &mut Rng,
    fast_forward: bool,
    sounds: &mut Vec<Sound>,
) -> Option<SpawnRequest> {
    match actor.state {
        0x1b => {
            actor.vy = (rng.rand_range(3) as i16).wrapping_sub(1);
            physics::integrate(actor, fast_forward);
            physics::change_state(actor, 0x1c);
        }
        0x1c => {
            actor.vx = 4;
            physics::integrate(actor, fast_forward);
            physics::change_state(actor, 0x1b);
        }
        0x1d => {
            actor.vx = 0;
            actor.vy = 0;
            physics::integrate(actor, fast_forward);
            let r = rng.rand_range(0x20);
            physics::change_state(actor, if r != 0 { 0x1e } else { 0x1b });
        }
        0x1e => {
            if rng.rand_range(100) != 0 {
                physics::integrate(actor, fast_forward);
                physics::change_state(actor, 0x1d);
                return None;
            }
            // Jump — spawn the effect from the pre-integration position.
            let req = SpawnRequest {
                class: ActorClass::Effect,
                state: None,
                sprite: Some(0x52),
                x: actor.x.wrapping_sub(4),
                y: actor.y.wrapping_sub(2),
                z: actor.z,
            };
            sounds.push(Sound::SnowboarderJump);
            physics::integrate(actor, fast_forward);
            physics::change_state(actor, 0x1b);
            return Some(req);
        }
        _ => {
            physics::integrate(actor, fast_forward);
        }
    }
    None
}

/// Ambient mob behavior (`FUN_004041c0` pure integration + `FUN_00404290`, class 4)
///
/// The ambient mob is a spawner that drifts vertically just left of the slope
/// center (x = -112 heading up in state 0x27, x = -144 heading down in 0x29),
/// wrapping at the course ends. It floats at z = 0x20: proxy integration has
/// no gravity and is not doubled by fast-forward.
///
/// In state 0x27 there is a 1-in-1000 chance per tick to drop a crossing
/// skier (class 3, state 0x21) at the mob's current position — including its
/// z = 0x20, so the skier visibly drops in from above — and flip to state
/// 0x28. The original gates the roll on the mob node having a live visible
/// actor ([node+0x00] != 0 in `FUN_00404290`); `materialized` carries that
/// state from the caller (see `GameState::update_proxies`). The integration
/// and wrap run regardless, like the original's mob nodes.
pub fn update_ambient(actor: &mut Actor, rng: &mut Rng, materialized: bool) -> Option<SpawnRequest> {
    // Proxy integration (FUN_004041c0): pure velocity add, no gravity.
    actor.x = actor.x.wrapping_add(actor.vx);
    actor.y = actor.y.wrapping_add(actor.vy);
    actor.z = actor.z.wrapping_add(actor.vz);

    // Wrap at y boundaries (FUN_00404290: resets x and vertical velocity)
    if actor.y < -1023 {
        physics::change_state(actor, 0x29);
        actor.vy = 2;
        actor.x = -144; // 0xff70 as i16
        return None;
    }
    if actor.y > 23551 {
        physics::change_state(actor, 0x27);
        actor.vy = -2; // 0xfffe as i16
        actor.x = -112; // 0xff90 as i16
        return None;
    }

    // 1-in-1000 chance to drop a crossing skier at the mob's position —
    // rolled only while the mob is inside the viewport window.
    if materialized && actor.state == 0x27 && rng.rand_range(1000) == 0 {
        let req = SpawnRequest {
            class: ActorClass::Obstacle,
            state: Some(0x21),
            sprite: None,
            x: actor.x,
            y: actor.y,
            z: actor.z,
        };
        physics::change_state(actor, 0x28);
        return Some(req);
    }
    None
}

/// Lift chair behavior (`FUN_004037b0`, class 10)
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

/// Obstacle behavior (`FUN_00403910`, class 3)
pub fn update_obstacle(actor: &mut Actor, rng: &mut Rng, fast_forward: bool) {
    let state = actor.state;
    if !(0x1f..=0x26).contains(&state) {
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
    } else if (0x22..=0x26).contains(&state) {
        let next = state + 1;
        if next == 0x27 {
            physics::change_state(actor, 0x20);
        } else {
            physics::change_state(actor, next);
        }
    }
}

/// Eating behavior (`FUN_00403750`, class 9)
pub fn update_eating(actor: &mut Actor, _fast_forward: bool) {
    let next = actor.state + 1;
    if next > 0x3b {
        physics::change_state(actor, 0x38);
    } else {
        physics::change_state(actor, next);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Find a seed whose first `rand_range(n)` roll is 0 (for the rare paths).
    fn seed_with_first_roll_zero(n: u16) -> u32 {
        (0..1_000_000u32)
            .find(|&s| {
                let mut r = Rng::new(s);
                r.rand_range(n) == 0
            })
            .expect("an LCG seed with first roll 0 exists")
    }

    fn ambient(x: i16, y: i16, z: i16, state: u16, vy: i16) -> Actor {
        Actor { active: true, class: ActorClass::AmbientMob, x, y, z, vy, state, ..Default::default() }
    }

    #[test]
    fn ambient_integrates_without_gravity() {
        // Proxy integration is pure: z = 0x20 floats forever, no fast-forward.
        let mut rng = Rng::new(7);
        let mut a = ambient(-112, 100, 0x20, 0x27, -2);
        update_ambient(&mut a, &mut rng, true);
        assert_eq!((a.x, a.y, a.z), (-112, 98, 0x20));
        assert_eq!(a.vz, 0);
    }

    #[test]
    fn ambient_wraps_at_course_ends() {
        let mut rng = Rng::new(7);
        let mut a = ambient(-112, 23552, 0x20, 0x27, -2);
        // y + vy = 23550 is in range first; force the wrap directly.
        a.y = 23560;
        update_ambient(&mut a, &mut rng, true);
        assert_eq!(a.state, 0x27);
        assert_eq!((a.x, a.vy), (-112, -2));

        let mut b = ambient(-144, -1030, 0x20, 0x29, 2);
        update_ambient(&mut b, &mut rng, true);
        assert_eq!(b.state, 0x29);
        assert_eq!((b.x, b.vy), (-144, 2));
    }

    #[test]
    fn ambient_drops_crossing_skier_on_rare_roll() {
        let seed = seed_with_first_roll_zero(1000);
        let mut rng = Rng::new(seed);
        let mut a = ambient(-112, 500, 0x20, 0x27, -2);
        let req = update_ambient(&mut a, &mut rng, true).expect("should request a spawn");
        // Mob integrated first (y 500 -> 498), then spawned at its position.
        assert_eq!(req.class, ActorClass::Obstacle);
        assert_eq!(req.state, Some(0x21));
        assert_eq!(req.sprite, None);
        assert_eq!((req.x, req.y, req.z), (-112, 498, 0x20));
        assert_eq!(a.state, 0x28);
    }

    #[test]
    fn ambient_no_spawn_on_common_roll() {
        // Seed 1: first rand_range(1000) is 41 != 0.
        let mut rng = Rng::new(1);
        let mut a = ambient(-112, 500, 0x20, 0x27, -2);
        assert_eq!(update_ambient(&mut a, &mut rng, true), None);
        assert_eq!(a.state, 0x27);
    }

    #[test]
    fn ambient_skips_roll_outside_viewport_window() {
        // A winning seed, but not materialized: no roll, no spawn, and the
        // RNG stream is untouched (calls stay at 0).
        let seed = seed_with_first_roll_zero(1000);
        let mut rng = Rng::new(seed);
        let mut a = ambient(-112, 500, 0x20, 0x27, -2);
        assert_eq!(update_ambient(&mut a, &mut rng, false), None);
        assert_eq!(a.state, 0x27);
        assert_eq!(rng.call_count(), 0);
    }

    fn snowboarder(x: i16, y: i16, z: i16, state: u16) -> Actor {
        Actor { active: true, class: ActorClass::Snowboarder, x, y, z, state, ..Default::default() }
    }

    #[test]
    fn snowboarder_jump_spawns_effect_and_resumes() {
        let seed = seed_with_first_roll_zero(100);
        let mut rng = Rng::new(seed);
        let mut a = snowboarder(200, 300, 6, 0x1e);
        let req = update_snowboarder(&mut a, &mut rng, false, &mut Vec::new()).expect("jump effect");
        assert_eq!(req.class, ActorClass::Effect);
        assert_eq!(req.sprite, Some(0x52));
        assert_eq!(req.state, None);
        // Effect spawns from the pre-integration position at (x-4, y-2, z).
        assert_eq!((req.x, req.y, req.z), (196, 298, 6));
        // The snowboarder integrates and continues moving (state 0x1b).
        assert_eq!(a.state, 0x1b);
    }

    #[test]
    fn snowboarder_no_jump_goes_back_to_stopped() {
        // Seed 1: first rand_range(100) is 41 != 0.
        let mut rng = Rng::new(1);
        let mut a = snowboarder(200, 300, 0, 0x1e);
        assert_eq!(update_snowboarder(&mut a, &mut rng, false, &mut Vec::new()), None);
        assert_eq!(a.state, 0x1d);
    }
}
