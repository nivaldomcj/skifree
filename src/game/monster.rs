use super::physics;
use super::rng::Rng;
use super::types::*;

/// Monster AI (FUN_00404350)
/// Four boundary guards: left (5), right (6), top (7), bottom (8)
/// Rubber-band chase: repositioned near player every tick
pub fn update_monster(actor: &mut Actor, player: Option<&Actor>, rng: &mut Rng, elapsed_ms: u32) {
    // Eating sequence: wall-clock state machine (works even while paused)
    if actor.state >= 0x32 && actor.state <= 0x37 {
        let elapsed = elapsed_ms.wrapping_sub(actor.timestamp);
        match actor.state {
            0x32 => physics::change_state(actor, 0x33),
            0x33 => {
                if elapsed > 500 {
                    physics::change_state(actor, 0x34);
                }
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
                if elapsed > 3000 {
                    physics::change_state(actor, 0x2a);
                }
            }
            _ => {}
        }
        return;
    }

    // Check if armed (player past boundary)
    let player = match player {
        Some(p) => p,
        None => return,
    };

    let armed = match actor.class {
        ActorClass::MonsterLeft => actor.y < -1999 && player.y < -2000,
        ActorClass::MonsterRight => actor.y > 31999 && player.y > 32000,
        ActorClass::MonsterTop => player.x < -16000,
        ActorClass::MonsterBottom => player.x > 16000,
        _ => false,
    };

    if !armed {
        // Dormant — sit at boundary
        return;
    }

    // Rubber-band chase
    let dx = player.x.wrapping_sub(actor.x);
    let dy = player.y.wrapping_sub(actor.y);

    // Reposition near player
    let step_x = 16i16;
    let step_y = 10i16;
    if dx.abs() > step_x {
        actor.x = player.x - dx.signum() * step_x;
    }
    if dy.abs() > step_y {
        actor.y = player.y - dy.signum() * step_y;
    }

    // Clamp velocity
    actor.vx = dx.clamp(-16, 16);
    actor.vy = dy.clamp(-10, 26);

    // 1-in-10 pause chance when close
    if dx.abs() < 1 && dy.abs() < 1 {
        if rng.rand_range(10) == 0 {
            physics::change_state(actor, 0x2b);
            actor.vz = 4; // pause for 4 frames
            return;
        }
    }

    // Select sprite from direction
    let state = direction_to_state(dx, dy);
    physics::change_state(actor, state);
}

/// Map chase direction to monster sprite state (0x2a-0x31)
fn direction_to_state(dx: i16, dy: i16) -> u16 {
    if dx == 0 && dy == 0 {
        return 0x2a;
    }
    let abs_dx = dx.abs() as i32;
    let abs_dy = dy.abs() as i32;

    if abs_dy > abs_dx * 2 {
        // Mostly vertical
        if dy < 0 { 0x30 } else { 0x31 }
    } else if abs_dx > abs_dy * 2 {
        // Mostly horizontal
        if dx < 0 { 0x2c } else { 0x2e }
    } else {
        // Diagonal
        if dx < 0 && dy < 0 { 0x2f }
        else if dx > 0 && dy < 0 { 0x31 }
        else if dx < 0 && dy > 0 { 0x2b }
        else { 0x2d }
    }
}

/// Monster catches the skier (FUN_00403a00 case 5-8)
pub fn eat_skier(monster: &mut Actor, skier_idx: usize, elapsed_ms: u32) -> usize {
    physics::change_state(monster, 0x32);
    monster.timestamp = elapsed_ms;
    monster.vx = 0;
    monster.vy = 0;
    monster.vz = 0;
    skier_idx // caller should free the skier actor
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monster_at(class: ActorClass, x: i16, y: i16) -> Actor {
        Actor {
            active: true,
            class,
            x,
            y,
            state: 0x2a,
            ..Default::default()
        }
    }

    fn player_at(x: i16, y: i16) -> Actor {
        Actor {
            active: true,
            class: ActorClass::Skier,
            x,
            y,
            ..Default::default()
        }
    }

    #[test]
    fn bottom_guard_arms_only_past_2000m() {
        let mut rng = Rng::new(1);
        // monster.y must be > 31999 AND player.y > 32000
        let mut m = monster_at(ActorClass::MonsterRight, 0, 32060);
        let p = player_at(0, 31999); // not past boundary
        let orig_x = m.x;
        update_monster(&mut m, Some(&p), &mut rng, 0);
        assert_eq!(m.x, orig_x, "should stay dormant before boundary");

        let p2 = player_at(0, 32001); // past boundary
        update_monster(&mut m, Some(&p2), &mut rng, 0);
        // Rubber-banded near player: x = player.x ± step (16)
        assert!((m.x - p2.x).abs() <= 16);
    }

    #[test]
    fn rubber_band_clamps_velocity() {
        let mut rng = Rng::new(1);
        let mut m = monster_at(ActorClass::MonsterRight, 0, 32060);
        let p = player_at(1000, 32100); // far away
        update_monster(&mut m, Some(&p), &mut rng, 0);
        assert!(m.vx >= -16 && m.vx <= 16, "vx clamped to ±16, got {}", m.vx);
        assert!(m.vy >= -10 && m.vy <= 26, "vy clamped to -10..26, got {}", m.vy);
    }

    #[test]
    fn rubber_band_repositions_near_player() {
        let mut rng = Rng::new(1);
        let mut m = monster_at(ActorClass::MonsterRight, 5000, 32060);
        let p = player_at(0, 32100);
        update_monster(&mut m, Some(&p), &mut rng, 0);
        // dx = 0 - 5000 = -5000, |dx| > 16, so x = player.x + 16
        assert_eq!(m.x, 16);
    }

    #[test]
    fn eating_sequence_timing() {
        let mut rng = Rng::new(1);
        let mut m = monster_at(ActorClass::MonsterRight, 0, 32060);
        eat_skier(&mut m, 42, 1000); // eaten at t=1000
        assert_eq!(m.state, 0x32);
        assert_eq!(m.timestamp, 1000);
        assert_eq!(m.vx, 0);

        update_monster(&mut m, None, &mut rng, 1000); // 0x32 → 0x33 immediately
        assert_eq!(m.state, 0x33);

        update_monster(&mut m, None, &mut rng, 1400); // 400ms elapsed: still 0x33
        assert_eq!(m.state, 0x33);
        update_monster(&mut m, None, &mut rng, 1501); // 501ms: → 0x34
        assert_eq!(m.state, 0x34);

        update_monster(&mut m, None, &mut rng, 2201); // 701ms: → 0x35
        assert_eq!(m.state, 0x35);
        update_monster(&mut m, None, &mut rng, 3201); // 1001ms: → 0x36
        assert_eq!(m.state, 0x36);

        update_monster(&mut m, None, &mut rng, 3201); // → 0x37 immediately
        assert_eq!(m.state, 0x37);
        update_monster(&mut m, None, &mut rng, 6000); // 3001ms: → 0x2a
        assert_eq!(m.state, 0x2a);
    }

    #[test]
    fn top_guard_arms_on_left_boundary() {
        let mut rng = Rng::new(1);
        let mut m = monster_at(ActorClass::MonsterTop, -16060, 0);
        let p = player_at(-16001, 0); // past -16000
        update_monster(&mut m, Some(&p), &mut rng, 0);
        assert!((m.x - p.x).abs() <= 16);
    }
}
