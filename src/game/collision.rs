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

/// Padding around the client rect for the original's "live" viewport
/// (DAT_0040c680 = client rect inflated by 0x78 on every side).
pub const VIEW_PAD: i16 = 0x78;

/// Whether a sprite at (x, y, z) is "on screen" in the original's sense: its
/// FUN_004014b0 draw box strictly overlaps the padded viewport DAT_0040c680
/// (FUN_00401290). Gates materialization (FUN_00404130), culling and the
/// populator's density sum.
pub fn in_padded_view(x: i16, y: i16, z: i16, sprite_w: i16, sprite_h: i16, camera: &Camera, screen_w: i16, screen_h: i16) -> bool {
    let (sx, sy) = camera.world_to_screen(x, y, screen_w, screen_h);
    let left = sx - sprite_w / 2;
    let top = sy - z - sprite_h;
    intersects(left, top, left + sprite_w, top + sprite_h, -VIEW_PAD, -VIEW_PAD, screen_w + VIEW_PAD, screen_h + VIEW_PAD)
}

/// The spawn-event y window scanned by FUN_004046e0 / FUN_004040a0: event y
/// on screen within [pad_top - 0x3c, pad_bottom + 0x3c).
pub fn in_event_window(y: i16, camera: &Camera, screen_w: i16, screen_h: i16) -> bool {
    let (_, sy) = camera.world_to_screen(0, y, screen_w, screen_h);
    sy >= -VIEW_PAD - 0x3c && sy < screen_h + VIEW_PAD + 0x3c
}

/// FUN_00403a00's crossing flag (local_4, a.k.a. bVar4). Compares the
/// skier's current/previous y against the other actor's y (FUN_00404070
/// gives a static actor itself as its "previous" copy, so its y is used for
/// both). Set when the skier's y reached or passed the other's y this tick,
/// i.e. exactly one tick per crossing. Mirrors the original's branch order,
/// including the equality edge cases.
pub fn crossed(y: i16, pre_y: i16, other_y: i16) -> bool {
    let (a, a0, b, b0) = (y, pre_y, other_y, other_y);
    let flips = (a >= b && a0 <= b0) || (a <= b && a0 >= b0);
    flips && !(a == b && a0 == b0)
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
        if std::env::var_os("SKIFREE_LOG_COLLISIONS").is_some() {
            let (s, o) = if a.class == ActorClass::Skier { (&*a, &*b) } else { (&*b, &*a) };
            eprintln!(
                "collision skier=({},{}) vy={} other=({:?},spr{},({},{})",
                s.x, s.y, s.vy, o.class, o.sprite_id, o.x, o.y
            );
        }
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
                } else if !crossed(skier.y, skier.y - skier.vy, other.y) {
                    // FUN_00403a00: overlapping but not on the crossing tick
                    // — nothing happens (the hit lands the tick the skier's
                    // y reaches the obstacle's base).
                } else if other.class == ActorClass::Tree
                    && (skier.x as i32 - other.x as i32).abs()
                        > (skier_dims.0.max(other_dims.0) as i32) / 2
                {
                    // Glancing a tree (class 0xd): off-centre by more than half
                    // the wider sprite — just halves vy, no crash.
                    skier.vy /= 2;
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
                } else if skier.z < other.z + other_dims.1 {
                    // FUN_00403a00 case 0xf: airborne but still below the
                    // rock's top (z + sprite height) — bounce off it. Above
                    // the top the skier clears it untouched.
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
            ActorClass::Snowboarder | ActorClass::Gate | ActorClass::Effect => {
                // FUN_00403a00 cases 2/0xc/0x11: a graze halves vy — but only
                // when the original's "moving apart" flag (bVar4) is set,
                // which happens on exactly one tick per crossing: the tick
                // the skier's y crosses the other's y (pre-update y on one
                // side, post-update y on the other). The collision runs
                // after the integrator, so pre-update y = y - vy.
                if crossed(skier.y, skier.y - skier.vy, other.y) {
                    skier.vy /= 2;
                }
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
        // The graze fires on the crossing tick: the skier's y just crossed
        // the other's y this update (pre-update y = y - vy = 0 <= 10 < 16).
        o.y = 10;
        s.y = 16;
        let r = collide(&mut s, &mut o, (16, 30), (26, 30));
        assert_eq!(s.vy, 8);
        assert!(!r.skier_crash);
        assert_eq!(r.score_delta, 0);
    }

    #[test]
    fn graze_does_not_slow_before_the_crossing_tick() {
        let mut s = skier();
        let mut o = scenery(ActorClass::Snowboarder, 37);
        // Still far ahead: no crossing this tick — no halving.
        o.y = 100;
        let r = collide(&mut s, &mut o, (16, 30), (26, 30));
        assert_eq!(s.vy, 16);
        assert!(!r.skier_crash);
        // Already past (crossed on an earlier tick): no halving either.
        let mut s2 = skier();
        let mut o2 = scenery(ActorClass::Snowboarder, 37);
        o2.y = -100;
        collide(&mut s2, &mut o2, (16, 30), (26, 30));
        assert_eq!(s2.vy, 16);
    }

    #[test]
    fn crossing_flag_fires_once_per_crossing() {
        // Approaching (both ticks above the obstacle): no.
        assert!(!crossed(3387, 3371, 3406));
        // Reached or passed this tick: yes.
        assert!(crossed(3419, 3403, 3406));
        assert!(crossed(3406, 3390, 3406));
        // Already past on the previous tick: no.
        assert!(!crossed(3435, 3419, 3406));
        // Moving uphill across it counts too.
        assert!(crossed(3400, 3410, 3406));
        // Standing exactly on it with no movement: no.
        assert!(!crossed(3406, 3406, 3406));
    }

    #[test]
    fn tree_hit_lands_only_on_the_crossing_tick() {
        let mut s = skier(); // vy = 16
        let mut o = scenery(ActorClass::Tree, 49);
        o.y = 3406;
        s.y = 3403; // pre-update 3387: still above the trunk base
        let r = collide(&mut s, &mut o, (16, 32), (28, 32));
        assert!(!r.skier_crash);
        assert_eq!(r.score_delta, 0);
        s.y = 3419; // pre-update 3403: crossed this tick
        let r = collide(&mut s, &mut o, (16, 32), (28, 32));
        assert!(r.skier_crash);
        assert_eq!(r.score_delta, -0x20);
    }

    #[test]
    fn tree_glance_halves_speed_without_crash() {
        let mut s = skier();
        let mut o = scenery(ActorClass::Tree, 49);
        o.y = 10;
        s.y = 16;
        o.x = 15; // |dx| 15 > max(16, 28) / 2 = 14
        let r = collide(&mut s, &mut o, (16, 32), (28, 32));
        assert!(!r.skier_crash);
        assert_eq!(s.vy, 8);
    }

    #[test]
    fn airborne_skier_above_large_rock_top_clears_it() {
        let mut s = skier();
        s.vy = 17;
        s.z = 4;
        s.vz = 3;
        let mut o = scenery(ActorClass::RockLarge, 47); // 16x4: top at z 4
        collide(&mut s, &mut o, (16, 32), (16, 4));
        assert_eq!(s.vz, 3, "no bounce at or above the rock's top");
        s.z = 3;
        collide(&mut s, &mut o, (16, 32), (16, 4));
        assert_eq!(s.vz, 8, "below the top: bounce at vy / 2");
    }
}
