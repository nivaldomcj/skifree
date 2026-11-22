use super::types::*;

/// AABB intersection test (FUN_00401290)
pub fn intersects(l1: i16, t1: i16, r1: i16, b1: i16, l2: i16, t2: i16, r2: i16, b2: i16) -> bool {
    l1 < r2 && l2 < r1 && t1 < b2 && t2 < b1
}

/// Actor bounding box from sprite dimensions and position (FUN_004014b0)
pub fn actor_bounds(actor: &Actor, sprite_w: i16, sprite_h: i16, camera: &Camera, screen_w: i16, screen_h: i16) -> (i16, i16, i16, i16) {
    let (sx, sy) = camera.world_to_screen(actor.x, actor.y, screen_w, screen_h);
    let left = sx - sprite_w / 2;
    let top = sy - actor.z - sprite_h;
    (left, top, left + sprite_w, top + sprite_h)
}

/// Collision dispatch (FUN_00403a00, player side).
///
/// The original runs the switch once per ordered pair; the port calls this
/// once per unordered pair and resolves the player side internally. `a_dims`
/// / `b_dims` are the actors' current sprite dimensions, needed for the
/// z-overlap (flyover) tests.
pub fn handle_collision(
    a: &mut Actor,
    b: &mut Actor,
    _player_idx: Option<usize>,
    a_idx: usize,
    b_idx: usize,
    a_dims: (i16, i16),
    b_dims: (i16, i16),
) -> CollisionResult {
    let mut result = CollisionResult::default();

    // Only the player (class 0) triggers most collisions
    if a.class == ActorClass::Skier || b.class == ActorClass::Skier {
        let a_is_skier = a.class == ActorClass::Skier;
        let (skier, other) = if a_is_skier { (&mut *a, &mut *b) } else { (&mut *b, &mut *a) };
        let other_idx = if a_is_skier { b_idx } else { a_idx };
        let (skier_dims, other_dims) = if a_is_skier { (a_dims, b_dims) } else { (b_dims, a_dims) };

        // z-overlap: the skier flies over (or under) the other actor.
        let other_bottom = other.z + other_dims.1;
        let skier_bottom = skier.z + skier_dims.1;
        let flyover = other_bottom < skier.z || skier_bottom < other.z;

        match other.class {
            ActorClass::MonsterLeft | ActorClass::MonsterRight |
            ActorClass::MonsterTop | ActorClass::MonsterBottom => {
                // Eaten by monster
                result.skier_eaten = true;
                result.eaten_by = Some(other_idx);
            }
            ActorClass::SnowBump => {
                // Snow bump: launches a straight-downhill skier (state 0).
                if skier.state == 0 {
                    skier.vz = 1;
                    if skier.vy > 4 {
                        skier.vy /= 2;
                    }
                    result.skier_state = Some(0x0d);
                }
            }
            ActorClass::Tree | ActorClass::Dog | ActorClass::Obstacle |
            ActorClass::AmbientMob | ActorClass::Lift | ActorClass::Eating => {
                if flyover {
                    // Flew over (or under) it: style bonus, no crash.
                    if other.class == ActorClass::Eating {
                        result.score_delta += 1000;
                        other.class = ActorClass::Tree;
                        other.sprite_id = 0x32;
                    } else {
                        result.score_delta += 6;
                    }
                } else {
                    if skier.z == 0 && skier.vz == 0 {
                        result.skier_crash = true; // grounded -> state 0x0b
                    } else {
                        result.skier_state = Some(0x11); // crash-landing
                        if other.sprite_id == 0x32 {
                            // Breaking the dead tree: it becomes a breaking
                            // animation (class 9, state 0x38), +16 style.
                            other.class = ActorClass::Eating;
                            super::physics::change_state(other, 0x38);
                            result.score_delta += 0x10;
                            return result;
                        }
                    }
                    result.score_delta += -0x20;
                }
            }
            ActorClass::Rock => {
                if skier.z > 0 {
                    if other_bottom < skier.z {
                        // Flying above: a previously flipped rock (sprite 86)
                        // is collected for +100; otherwise sail over.
                        if other.sprite_id == 0x56 {
                            result.free_actor = Some(other_idx);
                            result.score_delta += 100;
                        }
                    } else {
                        // Bounce off mid-air.
                        skier.vz = skier.vy / 2;
                        result.score_delta += 1;
                    }
                } else if skier.vy < 0 && other.sprite_id == 0x2e {
                    // Uphill hit flips the small rock flat (sprite 86).
                    other.sprite_id = 0x56;
                } else {
                    result.skier_crash = true;
                    result.score_delta += -0x20;
                }
            }
            ActorClass::RockLarge => {
                if skier.z < 1 {
                    // Auto-hop over large rocks.
                    skier.vz = 4;
                    result.score_delta += 1;
                    result.skier_state = Some(0x0d);
                } else {
                    skier.vz = skier.vy / 2;
                    result.score_delta += 1;
                }
            }
            ActorClass::Banner => {
                // Rainbow freestyle ramp: launches the skier at its own speed.
                if (skier.z as i32) < (other_dims.1 as i32) / 2 && skier.vy > 0 {
                    skier.vz = skier.vy;
                    result.score_delta += 1;
                    result.skier_state = Some(0x0d);
                }
            }
            ActorClass::Snowboarder => {
                // Brush past: just slows the skier (no crash).
                skier.vy /= 2;
            }
            ActorClass::Effect => {
                // Skiing through a jump puff costs style and slows the skier.
                skier.vy /= 2;
                if other.sprite_id == 0x52 {
                    result.score_delta += -0x10;
                }
            }
            _ => {}
        }

        // Check for gate pass (event_type 0x0c = slalom gate)
        if other.class == ActorClass::Obstacle && other.event_idx.is_some() {
            result.score_delta = 100; // gate bonus
        }
    }

    // Monster eating sequence trigger
    if (a.class == ActorClass::MonsterLeft || a.class == ActorClass::MonsterRight ||
        a.class == ActorClass::MonsterTop || a.class == ActorClass::MonsterBottom) &&
       b.class == ActorClass::Skier {
        result.skier_eaten = true;
        result.eaten_by = Some(a_idx);
    }
    if (b.class == ActorClass::MonsterLeft || b.class == ActorClass::MonsterRight ||
        b.class == ActorClass::MonsterTop || b.class == ActorClass::MonsterBottom) &&
       a.class == ActorClass::Skier {
        result.skier_eaten = true;
        result.eaten_by = Some(b_idx);
    }

    result
}

#[derive(Default)]
pub struct CollisionResult {
    pub score_delta: i32,
    pub skier_crash: bool,
    pub skier_state: Option<u16>,
    pub skier_eaten: bool,
    pub eaten_by: Option<usize>,
    pub free_actor: Option<usize>,
}

/// Check collisions between all actor pairs (FUN_00401e50 inner loop)
pub fn check_all_collisions(
    pool: &mut super::actor::ActorPool,
    player_idx: Option<usize>,
    camera: &Camera,
    screen_w: i16,
    screen_h: i16,
    sprite_dims: &[(i16, i16)],
) -> Vec<CollisionResult> {
    let mut results = Vec::new();
    let indices = pool.active_indices();

    for i in 0..indices.len() {
        for j in (i + 1)..indices.len() {
            let idx_a = indices[i];
            let idx_b = indices[j];

            let (a, b) = match (pool.get(idx_a), pool.get(idx_b)) {
                (Some(a), Some(b)) => (a.clone(), b.clone()),
                _ => continue,
            };

            // Skip dead/invisible actors
            if a.flags.dead || b.flags.dead {
                continue;
            }

            // Get bounding boxes
            let (aw, ah) = sprite_dims.get(a.sprite_id as usize).copied().unwrap_or((16, 16));
            let (bw, bh) = sprite_dims.get(b.sprite_id as usize).copied().unwrap_or((16, 16));

            let (al, at, ar, ab) = actor_bounds(&a, aw, ah, camera, screen_w, screen_h);
            let (bl, bt, br, bb) = actor_bounds(&b, bw, bh, camera, screen_w, screen_h);

            if !intersects(al, at, ar, ab, bl, bt, br, bb) {
                continue;
            }

            // Handle collision — clone actors to avoid double borrow
            let mut a_clone = a.clone();
            let mut b_clone = b.clone();
            let result = handle_collision(
                &mut a_clone,
                &mut b_clone,
                player_idx,
                idx_a,
                idx_b,
                (aw, ah),
                (bw, bh),
            );

            // Write back changes
            if let Some(a_mut) = pool.get_mut(idx_a) {
                *a_mut = a_clone;
            }
            if let Some(b_mut) = pool.get_mut(idx_b) {
                *b_mut = b_clone;
            }

            if result.score_delta != 0 || result.skier_crash || result.skier_eaten
                || result.skier_state.is_some() || result.free_actor.is_some() {
                results.push(result);
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skier() -> Actor {
        Actor {
            active: true,
            class: ActorClass::Skier,
            state: 0,
            vy: 16,
            ..Default::default()
        }
    }

    fn scenery(class: ActorClass, sprite: u16) -> Actor {
        Actor {
            active: true,
            class,
            sprite_id: sprite,
            ..Default::default()
        }
    }

    fn collide(s: &mut Actor, o: &mut Actor, sd: (i16, i16), od: (i16, i16)) -> CollisionResult {
        handle_collision(s, o, Some(0), 0, 1, sd, od)
    }

    #[test]
    fn snow_bump_launches_straight_skier() {
        let mut s = skier();
        let mut o = scenery(ActorClass::SnowBump, 27);
        let r = collide(&mut s, &mut o, (16, 30), (64, 32));
        assert_eq!(s.vz, 1);
        assert_eq!(s.vy, 8); // 16 halved
        assert_eq!(r.skier_state, Some(0x0d));
        assert!(!r.skier_crash);
    }

    #[test]
    fn snow_bump_ignored_when_turning() {
        let mut s = skier();
        s.state = 3;
        let mut o = scenery(ActorClass::SnowBump, 27);
        let r = collide(&mut s, &mut o, (16, 30), (64, 32));
        assert_eq!(r.skier_state, None);
        assert_eq!(s.vz, 0);
    }

    #[test]
    fn tree_crash_grounded() {
        let mut s = skier();
        let mut o = scenery(ActorClass::Tree, 49);
        let r = collide(&mut s, &mut o, (16, 30), (28, 32));
        assert!(r.skier_crash);
        assert_eq!(r.score_delta, -0x20);
    }

    #[test]
    fn tree_flyover_bonus() {
        let mut s = skier();
        s.z = 40; // airborne above the tree top (32)
        let mut o = scenery(ActorClass::Tree, 49);
        let r = collide(&mut s, &mut o, (16, 30), (28, 32));
        assert!(!r.skier_crash);
        assert_eq!(r.score_delta, 6);
    }

    #[test]
    fn dead_tree_breaks_for_bonus() {
        let mut s = skier();
        s.z = 5; // airborne but overlapping the tree
        let mut o = scenery(ActorClass::Tree, 50); // dead tree sprite 0x32
        let r = collide(&mut s, &mut o, (16, 30), (22, 27));
        assert_eq!(r.skier_state, Some(0x11)); // crash-landing
        assert_eq!(r.score_delta, 0x10);
        assert_eq!(o.class, ActorClass::Eating);
        assert_eq!(o.state, 0x38);
    }

    #[test]
    fn rock_flips_on_uphill_hit() {
        let mut s = skier();
        s.vy = -2; // moving uphill
        let mut o = scenery(ActorClass::Rock, 46);
        let r = collide(&mut s, &mut o, (16, 30), (16, 11));
        assert_eq!(o.sprite_id, 0x56);
        assert!(!r.skier_crash);
        assert_eq!(r.score_delta, 0);
    }

    #[test]
    fn flipped_rock_collected_midair() {
        let mut s = skier();
        s.z = 20;
        let mut o = scenery(ActorClass::Rock, 0x56);
        let r = collide(&mut s, &mut o, (16, 30), (8, 11));
        assert_eq!(r.free_actor, Some(1));
        assert_eq!(r.score_delta, 100);
    }

    #[test]
    fn large_rock_auto_hop() {
        let mut s = skier();
        let mut o = scenery(ActorClass::RockLarge, 48);
        let r = collide(&mut s, &mut o, (16, 30), (24, 8));
        assert_eq!(s.vz, 4);
        assert_eq!(r.skier_state, Some(0x0d));
        assert_eq!(r.score_delta, 1);
    }

    #[test]
    fn banner_ramp_launches_at_own_speed() {
        let mut s = skier();
        let mut o = scenery(ActorClass::Banner, 52);
        let r = collide(&mut s, &mut o, (16, 30), (32, 8));
        assert_eq!(s.vz, 16); // vz = vy
        assert_eq!(r.skier_state, Some(0x0d));
        assert_eq!(r.score_delta, 1);
    }

    #[test]
    fn snowboarder_brush_slows_without_crash() {
        let mut s = skier();
        let mut o = scenery(ActorClass::Snowboarder, 37);
        let r = collide(&mut s, &mut o, (16, 30), (26, 30));
        assert_eq!(s.vy, 8);
        assert!(!r.skier_crash);
        assert_eq!(r.score_delta, 0);
    }
}
