use super::physics;
use super::rng::Rng;
use super::sound::Sound;
use super::types::{Actor, ActorClass};

/// Monster AI (`FUN_004041c0` integration + `FUN_00404350` logic)
///
/// Four boundary guards: left (5), right (6), top (7), bottom (8). Each tick
/// the monster integrates its velocity into position, applies gravity (it can
/// hop), then either runs the eating state machine, chases the player
/// (rubber-banded to within one screen), holds position at its boundary, or
/// drifts back toward it. Every chasing tick plays the chase sound.
pub fn update_monster(
    actor: &mut Actor,
    player: Option<&Actor>,
    rng: &mut Rng,
    elapsed_ms: u32,
    screen_w: i16,
    screen_h: i16,
    sounds: &mut Vec<Sound>,
) {
    // 1. Integrate velocity into position (FUN_004041c0).
    actor.x = actor.x.wrapping_add(actor.vx);
    actor.y = actor.y.wrapping_add(actor.vy);
    actor.z = actor.z.wrapping_add(actor.vz);

    // 2. Gravity (FUN_00404350): clamp on the ground, vz -= 1 when airborne.
    if actor.z < 1 {
        actor.vz = 0;
        actor.z = 0;
    } else {
        actor.vz -= 1;
    }
    // Airborne (mid-hop): no chase/eat, keep the current pose.
    if actor.z != 0 {
        return;
    }

    // 3. Eating sequence (wall-clock state machine, works even while paused;
    //    FUN_00404350's switch at 0x4043be). Two chewing loops: 0x32 <-> 0x33
    //    until 500 ms, and 0x36 <-> 0x37 until 3000 ms (verified via Frida).
    if (0x32..=0x37).contains(&actor.state) {
        let elapsed = elapsed_ms.wrapping_sub(actor.timestamp);
        match actor.state {
            0x32 => physics::change_state(actor, 0x33),
            0x33 => {
                physics::change_state(actor, if elapsed >= 500 { 0x34 } else { 0x32 });
            }
            0x34 => {
                if elapsed > 700 {
                    physics::change_state(actor, 0x35);
                }
            }
            0x35 => {
                if elapsed > 1000 {
                    physics::change_state(actor, 0x36);
                }
            }
            0x36 => physics::change_state(actor, 0x37),
            0x37 => {
                physics::change_state(actor, if elapsed >= 3000 { 0x2a } else { 0x36 });
            }
            _ => {}
        }
        return;
    }

    // 4. Chase when inside the guard region AND the player crossed the
    //    trigger; hold position when in region but the player hasn't crossed;
    //    drift back toward the region otherwise.
    let class = actor.class;
    let in_region = match class {
        ActorClass::MonsterLeft => actor.y < -1999,
        ActorClass::MonsterRight => actor.y > 31999,
        ActorClass::MonsterTop => actor.x < -15999,
        ActorClass::MonsterBottom => actor.x > 15999,
        _ => false,
    };
    let triggered = match (class, player) {
        (ActorClass::MonsterLeft, Some(p)) => p.y < -2000,
        (ActorClass::MonsterRight, Some(p)) => p.y > 32000,
        (ActorClass::MonsterTop, Some(p)) => p.x < -16000,
        (ActorClass::MonsterBottom, Some(p)) => p.x > 16000,
        _ => false,
    };

    let mut vx: i16 = 0;
    let mut vy: i16 = 0;
    if let Some(p) = player.filter(|_| in_region && triggered) {
        let dx = i32::from(p.x) - i32::from(actor.x);
        let dy = i32::from(p.y) - i32::from(actor.y);
        // Rubber-band: keep within one screen of the player (window dims).
        let sw = i32::from(screen_w);
        let sh = i32::from(screen_h);
        if dx > sw {
            actor.x = (i32::from(p.x) - sw) as i16;
        } else if dx < -sw {
            actor.x = (i32::from(p.x) + sw) as i16;
        }
        if dy > sh {
            actor.y = (i32::from(p.y) - sh) as i16;
        } else if dy < -sh {
            actor.y = (i32::from(p.y) + sh) as i16;
        }
        // Velocity toward the player.
        vx = dx.clamp(-16, 16) as i16;
        vy = dy.clamp(-10, 26) as i16;
        // FUN_00402ba0 right after the clamps (0x4045d3): on every chasing
        // tick, so the sound restarts each tick.
        sounds.push(Sound::Chase);
    } else if !in_region {
        // Drift back into the guard region.
        match class {
            ActorClass::MonsterLeft => vy = -10,
            ActorClass::MonsterRight => vy = 26,
            ActorClass::MonsterTop => vx = -16,
            ActorClass::MonsterBottom => vx = 16,
            _ => {}
        }
    }
    actor.vx = vx;
    actor.vy = vy;
    // FUN_00404350's shared tail (0x4045d8-0x40461d): whenever the monster
    // moves (chasing or drifting back), it hops: vz = 1, so it is airborne
    // for the next two ticks and skips this logic (no chase, no chase
    // sound) until it lands. (The tail's vx/vy rescaling is overwritten
    // right after, so only the hop survives.) Verified via Frida.
    if vx != 0 || vy != 0 {
        actor.vz = 1;
    }

    // 5. Sprite state from movement direction (4-way, 2-frame animation).
    if vy < 0 {
        toggle_anim(actor, 0x30); // up
    } else if vx < 0 {
        toggle_anim(actor, 0x2c); // left
    } else if vx == 0 && vy == 0 {
        // Reached the player: 1-in-10 hop, else idle.
        if rng.rand_range(10) == 0 {
            actor.vz = 4;
            physics::change_state(actor, 0x2b);
        } else {
            physics::change_state(actor, 0x2a);
        }
    } else {
        toggle_anim(actor, 0x2e); // down / right
    }
}

/// Two-frame walk animation: alternate between `base` and `base + 1`.
fn toggle_anim(actor: &mut Actor, base: u16) {
    let next = if actor.state == base { base + 1 } else { base };
    physics::change_state(actor, next);
}

/// Monster catches the skier (`FUN_00403a00` case 5-8); the caller frees
/// the skier.
pub fn eat_skier(monster: &mut Actor, elapsed_ms: u32) {
    physics::change_state(monster, 0x32);
    monster.timestamp = elapsed_ms;
    monster.vx = 0;
    monster.vy = 0;
    monster.vz = 0;
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: i16 = 640;
    const H: i16 = 480;

    fn monster_at(class: ActorClass, x: i16, y: i16) -> Actor {
        Actor { active: true, class, x, y, state: 0x2a, ..Default::default() }
    }

    fn player_at(x: i16, y: i16) -> Actor {
        Actor { active: true, class: ActorClass::Skier, x, y, ..Default::default() }
    }

    #[test]
    fn moving_monster_hops_and_skips_chase_while_airborne() {
        // Frida, compare_monster tick 803-805: the chase tick sets vz = 1;
        // the next two ticks the monster is at z = 1 and doesn't chase.
        let mut rng = Rng::new(1);
        let mut m = monster_at(ActorClass::MonsterLeft, 0, -2060);
        let p = player_at(0, -2001);
        let mut sounds = Vec::new();
        update_monster(&mut m, Some(&p), &mut rng, 0, W, H, &mut sounds);
        assert_eq!((m.vy, m.vz), (26, 1));
        assert_eq!(sounds, vec![Sound::Chase]);
        for (y, z) in [(-2034, 1), (-2008, 1)] {
            update_monster(&mut m, Some(&p), &mut rng, 0, W, H, &mut sounds);
            assert_eq!((m.y, m.z), (y, z));
        }
        assert_eq!(sounds.len(), 1, "no chase while airborne");
        // Landed at y = -1982, outside its region (y < -1999): it drifts
        // back (vy = -10) without the chase sound, and hops again.
        update_monster(&mut m, Some(&p), &mut rng, 0, W, H, &mut sounds);
        assert_eq!((m.y, m.z, m.vy, m.vz), (-1982, 0, -10, 1));
        assert_eq!(sounds.len(), 1);
    }

    #[test]
    fn guard_holds_until_player_crosses_trigger() {
        let mut rng = Rng::new(1);
        let mut m = monster_at(ActorClass::MonsterRight, 0, 32060);
        // Player has not crossed y=32000 yet: monster holds (no velocity).
        let p = player_at(0, 31999);
        update_monster(&mut m, Some(&p), &mut rng, 0, W, H, &mut Vec::new());
        assert_eq!((m.vx, m.vy), (0, 0), "should hold before trigger");

        // Player crosses: monster now has velocity toward the player.
        let p2 = player_at(0, 32001);
        update_monster(&mut m, Some(&p2), &mut rng, 0, W, H, &mut Vec::new());
        assert!((m.vx, m.vy) != (0, 0), "should chase after trigger");
    }

    #[test]
    fn chase_velocity_is_clamped() {
        let mut rng = Rng::new(1);
        let mut m = monster_at(ActorClass::MonsterRight, 0, 32060);
        let p = player_at(1000, 32100); // far but within one screen
        update_monster(&mut m, Some(&p), &mut rng, 0, W, H, &mut Vec::new());
        assert!(m.vx >= -16 && m.vx <= 16, "vx clamped to ±16, got {}", m.vx);
        assert!(m.vy >= -10 && m.vy <= 26, "vy clamped to -10..26, got {}", m.vy);
    }

    #[test]
    fn rubber_band_uses_window_dimensions() {
        let mut rng = Rng::new(1);
        // Monster a full screen behind the player horizontally.
        let mut m = monster_at(ActorClass::MonsterRight, 0, 32060);
        let p = player_at(5000, 32100); // dx = 5000 > screen_w
        update_monster(&mut m, Some(&p), &mut rng, 0, W, H, &mut Vec::new());
        // Teleported to exactly one screen-width behind the player.
        assert_eq!(m.x, p.x - W);
    }

    #[test]
    fn monster_integrates_velocity_into_position() {
        let mut rng = Rng::new(1);
        let mut m = monster_at(ActorClass::MonsterRight, 0, 32060);
        m.vx = 5;
        m.vy = 7;
        // Player not triggering, so velocity is recomputed to 0 after moving.
        let p = player_at(0, 31999);
        update_monster(&mut m, Some(&p), &mut rng, 0, W, H, &mut Vec::new());
        assert_eq!((m.x, m.y), (5, 32067), "position integrates old velocity first");
    }

    #[test]
    fn drift_when_outside_region() {
        let mut rng = Rng::new(1);
        // MonsterRight belongs at y>31999; place it inside the map (out of region).
        let mut m = monster_at(ActorClass::MonsterRight, 0, 100);
        let p = player_at(0, 50);
        update_monster(&mut m, Some(&p), &mut rng, 0, W, H, &mut Vec::new());
        assert_eq!(m.vy, 26, "should drift back down toward its region");
    }

    #[test]
    fn eating_sequence_timing() {
        let mut rng = Rng::new(1);
        let mut m = monster_at(ActorClass::MonsterRight, 0, 32060);
        eat_skier(&mut m, 1000); // eaten at t=1000
        assert_eq!(m.state, 0x32);
        assert_eq!(m.timestamp, 1000);
        assert_eq!(m.vx, 0);

        update_monster(&mut m, None, &mut rng, 1000, W, H, &mut Vec::new()); // 0x32 → 0x33 immediately
        assert_eq!(m.state, 0x33);

        update_monster(&mut m, None, &mut rng, 1400, W, H, &mut Vec::new()); // 400ms: chew back to 0x32
        assert_eq!(m.state, 0x32);
        update_monster(&mut m, None, &mut rng, 1440, W, H, &mut Vec::new()); // 0x32 → 0x33
        assert_eq!(m.state, 0x33);
        update_monster(&mut m, None, &mut rng, 1500, W, H, &mut Vec::new()); // 500ms: → 0x34
        assert_eq!(m.state, 0x34);

        update_monster(&mut m, None, &mut rng, 2201, W, H, &mut Vec::new()); // 701ms: → 0x35
        assert_eq!(m.state, 0x35);
        update_monster(&mut m, None, &mut rng, 3201, W, H, &mut Vec::new()); // 1001ms: → 0x36
        assert_eq!(m.state, 0x36);

        update_monster(&mut m, None, &mut rng, 3201, W, H, &mut Vec::new()); // → 0x37 immediately
        assert_eq!(m.state, 0x37);
        update_monster(&mut m, None, &mut rng, 3999, W, H, &mut Vec::new()); // 2999ms: chew back to 0x36
        assert_eq!(m.state, 0x36);
        update_monster(&mut m, None, &mut rng, 3999, W, H, &mut Vec::new()); // 0x36 → 0x37
        assert_eq!(m.state, 0x37);
        update_monster(&mut m, None, &mut rng, 4000, W, H, &mut Vec::new()); // 3000ms: → 0x2a
        assert_eq!(m.state, 0x2a);
    }

    #[test]
    fn airborne_monster_does_not_chase() {
        let mut rng = Rng::new(1);
        let mut m = monster_at(ActorClass::MonsterRight, 0, 32060);
        m.vz = 4; // mid-hop
        let p = player_at(100, 32100);
        update_monster(&mut m, Some(&p), &mut rng, 0, W, H, &mut Vec::new());
        assert!(m.z > 0, "still airborne");
        assert_eq!((m.vx, m.vy), (0, 0), "no chase velocity while airborne");
    }
}
