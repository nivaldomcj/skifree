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
