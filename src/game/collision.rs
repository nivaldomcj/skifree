use super::sound::Sound;
use super::types::*;

/// AABB intersection test (FUN_00401290)
pub fn intersects(l1: i16, t1: i16, r1: i16, b1: i16, l2: i16, t2: i16, r2: i16, b2: i16) -> bool {
    l1 < r2 && l2 < r1 && t1 < b2 && t2 < b1
}

/// Actor bounding box from sprite dimensions and position (FUN_004014b0)
pub fn actor_bounds(
    actor: &Actor,
    sprite_w: i16,
    sprite_h: i16,
    camera: &Camera,
    screen_w: i16,
    screen_h: i16,
) -> (i16, i16, i16, i16) {
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
pub fn in_padded_view(
    x: i16,
    y: i16,
    z: i16,
    sprite_w: i16,
    sprite_h: i16,
    camera: &Camera,
    screen_w: i16,
    screen_h: i16,
) -> bool {
    let (sx, sy) = camera.world_to_screen(x, y, screen_w, screen_h);
    let left = sx - sprite_w / 2;
    let top = sy - z - sprite_h;
    intersects(
        left,
        top,
        left + sprite_w,
        top + sprite_h,
        -VIEW_PAD,
        -VIEW_PAD,
        screen_w + VIEW_PAD,
        screen_h + VIEW_PAD,
    )
}

/// The spawn-event y window scanned by FUN_004046e0 / FUN_004040a0: event y
/// on screen within [pad_top - 0x3c, pad_bottom + 0x3c).
pub fn in_event_window(y: i16, camera: &Camera, screen_w: i16, screen_h: i16) -> bool {
    let (_, sy) = camera.world_to_screen(0, y, screen_w, screen_h);
    sy >= -VIEW_PAD - 0x3c && sy < screen_h + VIEW_PAD + 0x3c
}

/// FUN_00403a00's crossing flag (local_4, a.k.a. bVar4). Compares both
/// actors' current y with their previous y (FUN_00404070: the previous-
/// position copy at `[actor+4]`, i.e. the y at the start of the tick, or the
/// actor itself when it has none). Set when their vertical order flipped or
/// they met this tick — exactly one tick per crossing. Mirrors the original's
/// branch order, including the equality edge cases.
pub fn crossed(y: i16, pre_y: i16, other_y: i16, other_pre_y: i16) -> bool {
    let (a, a0, b, b0) = (y, pre_y, other_y, other_pre_y);
    let flips = (a >= b && a0 <= b0) || (a <= b && a0 >= b0);
    flips && !(a == b && a0 == b0)
}

/// Collision handler for one *ordered* pair (FUN_00403a00): `a` is the first
/// actor, whose class picks the outer switch case; `b` is the other one.
/// `check_all_collisions` calls it for both orders, like the original.
/// `a_dims` / `b_dims` are the actors' current sprite dimensions, needed for
/// the z-overlap (flyover) tests.
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

    // First actor classes >= 0xb (scenery, effects) have no handler.
    if a.class != ActorClass::Skier {
        return other_first(a, b, a_idx, b_dims);
    }

    // Skier first (outer case 0).
    {
        if std::env::var_os("SKIFREE_LOG_COLLISIONS").is_some() {
            eprintln!(
                "collision skier=({},{}) vy={} other=({:?},spr{},({},{})",
                a.x, a.y, a.vy, b.class, b.sprite_id, b.x, b.y
            );
        }
        let (skier, other) = (&mut *a, &mut *b);
        let other_idx = b_idx;
        let (skier_dims, other_dims) = (a_dims, b_dims);

        // z-overlap: the skier flies over (or under) the other actor.
        let other_bottom = other.z + other_dims.1;
        let skier_bottom = skier.z + skier_dims.1;
        let flyover = other_bottom < skier.z || skier_bottom < other.z;

        match other.class {
            // Skier-first vs a monster does nothing (inner cases 5-8 fall
            // to the default); the eating runs monster-first.
            ActorClass::MonsterLeft | ActorClass::MonsterRight | ActorClass::MonsterTop | ActorClass::MonsterBottom => {
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
            ActorClass::Tree
            | ActorClass::Dog
            | ActorClass::Obstacle
            | ActorClass::AmbientMob
            | ActorClass::Lift
            | ActorClass::Eating => {
                solid_hit(skier, other, skier_dims, other_dims, flyover, &mut result);
            }
            ActorClass::Rock => {
                // Case 0xe: grounded -> the shared solid-obstacle path.
                // Airborne above the rock's top: a flipped rock (sprite 0x56)
                // is collected for +100. Airborne below the top: bounce at
                // vy / 2 (+1), on the crossing tick only.
                if skier.z <= 0 {
                    solid_hit(skier, other, skier_dims, other_dims, flyover, &mut result);
                } else if skier.z > other.z + other_dims.1 {
                    if other.sprite_id == 0x56 {
                        result.free_actor = Some(other_idx);
                        result.score_delta += 100;
                    }
                } else if crossed(skier.y, skier.prev_y.unwrap_or(skier.y), other.y, other.prev_y.unwrap_or(other.y)) {
                    skier.vz = skier.vy / 2;
                    result.score_delta += 1;
                    result.sound = Some(Sound::Launch);
                }
            }
            ActorClass::RockLarge => {
                if skier.z < 1 {
                    // Auto-hop over large rocks.
                    skier.vz = 4;
                    result.score_delta += 1;
                    result.skier_state = Some(0x0d);
                    result.sound = Some(Sound::Launch);
                } else if skier.z < other.z + other_dims.1 {
                    // FUN_00403a00 case 0xf: airborne but still below the
                    // rock's top (z + sprite height) — bounce off it. Above
                    // the top the skier clears it untouched.
                    skier.vz = skier.vy / 2;
                    result.score_delta += 1;
                    result.sound = Some(Sound::Launch);
                }
            }
            ActorClass::Banner => {
                // Rainbow freestyle ramp (case 0x10): on the crossing tick, a
                // skier below half the ramp's top launches at its own speed.
                if crossed(skier.y, skier.prev_y.unwrap_or(skier.y), other.y, other.prev_y.unwrap_or(other.y))
                    && (skier.z as i32) < (other.z as i32 + other_dims.1 as i32) / 2
                    && skier.vy > 0
                {
                    skier.vz = skier.vy;
                    result.score_delta += 1;
                    result.skier_state = Some(0x0d);
                    result.sound = Some(Sound::Launch);
                }
            }
            ActorClass::Snowboarder | ActorClass::Gate | ActorClass::Effect => {
                // FUN_00403a00 cases 2/0xc/0x11: a graze halves vy — but only
                // when the original's "moving apart" flag (bVar4) is set,
                // which happens on exactly one tick per crossing: the tick
                // the skier's y crosses the other's y (pre-update y on one
                // side, post-update y on the other). The collision runs
                // after the integrator, so pre-update y = y - vy.
                if crossed(skier.y, skier.prev_y.unwrap_or(skier.y), other.y, other.prev_y.unwrap_or(other.y)) {
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
    /// The sound this handler call played (FUN_00402ba0; at most one).
    pub sound: Option<Sound>,
}

/// FUN_00403a00's shared solid-obstacle path (skier first; inner cases
/// 1/3/4/9/10/13, and 14 for a grounded skier). In order: fly over/under
/// (+6, or +1000 for hurdling the eating monster, which leaves a dead tree);
/// nothing unless this is the crossing tick; a tree hit off-centre by more
/// than half the wider sprite only halves vy; otherwise the skier goes down —
/// state 0xb when grounded and still, else 0x11 (crash-landing; hitting the
/// dead tree 0x32 then breaks it for +16 instead). An uphill hit (vy < 0) on
/// the small rock (sprite 0x2e) flips it flat (0x56) with no penalty; any
/// other hit costs 32 style.
fn solid_hit(
    skier: &mut Actor,
    other: &mut Actor,
    skier_dims: (i16, i16),
    other_dims: (i16, i16),
    flyover: bool,
    result: &mut CollisionResult,
) {
    if flyover {
        if other.class == ActorClass::Eating {
            result.score_delta += 1000;
            other.class = ActorClass::Tree;
            other.sprite_id = 0x32;
        } else {
            result.score_delta += 6;
        }
        return;
    }
    if !crossed(skier.y, skier.prev_y.unwrap_or(skier.y), other.y, other.prev_y.unwrap_or(other.y)) {
        return;
    }
    if other.class == ActorClass::Tree
        && (skier.x as i32 - other.x as i32).abs() > (skier_dims.0.max(other_dims.0) as i32) / 2
    {
        skier.vy /= 2;
        return;
    }
    if skier.z == 0 && skier.vz == 0 {
        result.skier_crash = true; // state 0x0b
    } else {
        result.skier_state = Some(0x11);
        if other.sprite_id == 0x32 {
            // Breaking the dead tree: class 9 breaking animation, +16.
            other.class = ActorClass::Eating;
            super::physics::change_state(other, 0x38);
            result.score_delta += 0x10;
            return;
        }
    }
    if skier.vy < 0 && other.sprite_id == 0x2e {
        other.sprite_id = 0x56;
    } else {
        result.score_delta += -0x20;
        result.sound = Some(Sound::Crash);
    }
}

/// Outer cases of FUN_00403a00 for a non-skier first actor `a` against `b`.
/// `top` is the other actor's top (`b.z` + its sprite height); "below the
/// top" compares the first actor's z against it. Style points go through
/// `score_delta` (FUN_00403420); state changes apply immediately, as the
/// original's FUN_00402120 calls do.
fn other_first(a: &mut Actor, b: &mut Actor, a_idx: usize, b_dims: (i16, i16)) -> CollisionResult {
    let mut result = CollisionResult::default();
    let top = b.z + b_dims.1;
    match a.class {
        // Case 1, dog: knocked into its yelp/flee states 0x19 (other on the
        // ground) or 0x1a (other airborne); +20 style when it was the player.
        ActorClass::Dog => {
            if a.state < 0x19 {
                if b.class == ActorClass::Skier {
                    result.score_delta += 20;
                }
                result.sound = Some(Sound::Dog);
                super::physics::change_state(a, if b.z > 0 { 0x1a } else { 0x19 });
            }
        }
        // Case 2, snowboarder: hit by anything moving -> falls (state 0x1d);
        // +3 style when it was the player. Static obstacles don't count.
        ActorClass::Snowboarder => {
            // Mob-node actors (classes 4-8) carry no velocity in the original:
            // FUN_004041c0 moves the node and only copies position and state
            // to its actor, so a drifting chair or a monster reads as still.
            let node_actor = matches!(
                b.class,
                ActorClass::AmbientMob
                    | ActorClass::MonsterLeft
                    | ActorClass::MonsterRight
                    | ActorClass::MonsterTop
                    | ActorClass::MonsterBottom
            );
            let moving = !node_actor && (b.vx != 0 || b.vy != 0);
            if a.state < 0x1d && moving {
                if b.class == ActorClass::Skier {
                    result.score_delta += 3;
                }
                result.sound = Some(Sound::SnowboarderHit);
                super::physics::change_state(a, 0x1d);
            }
        }
        // Case 3, crossing skier: sub-case by the other's class (byte table
        // at 0x404054).
        ActorClass::Obstacle => match b.class {
            ActorClass::Skier | ActorClass::Dog | ActorClass::Obstacle | ActorClass::Tree | ActorClass::Rock => {
                if b.class == ActorClass::Skier {
                    result.score_delta += 20;
                }
                // Below the other's top -> falls (0x22), unless already down.
                if a.z < top && a.state != 0x22 {
                    super::physics::change_state(a, 0x22);
                }
            }
            ActorClass::RockLarge | ActorClass::Banner => {
                // Bump/ramp: launched at half its speed (state 0x21).
                if a.z < top {
                    a.vz = a.vy / 2;
                    result.sound = Some(Sound::SkierLaunch);
                    super::physics::change_state(a, 0x21);
                }
            }
            _ => {}
        },
        // Cases 5-8, monster: eats the player.
        ActorClass::MonsterLeft | ActorClass::MonsterRight | ActorClass::MonsterTop | ActorClass::MonsterBottom => {
            if b.class == ActorClass::Skier {
                result.skier_eaten = true;
                result.eaten_by = Some(a_idx);
                result.sound = Some(Sound::Eaten);
            }
        }
        // Case 10, lift chair: stops (vx = 0, state 0x3c).
        ActorClass::Lift => {
            a.vx = 0;
            super::physics::change_state(a, 0x3c);
        }
        _ => {}
    }
    result
}

/// `SKIFREE_LOG_PAIRS=1`: print every ordered collision-handler call (diff
/// against tools/vm/trace_pairs.js).
fn log_pairs() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("SKIFREE_LOG_PAIRS").is_some())
}

/// Collision pass (FUN_00401e50, after the mob nodes). Mirrors the original's
/// pair walk: for each actor `j` in list order, against each actor `i`
/// *before* it in the list (the inner loop stops at the outer actor, so every
/// unordered pair is visited once), test the pair when at least one of them
/// moved this tick (flag 0x20, set by FUN_00402390 on a position change) and
/// their boxes intersect; then run the handler for (later, earlier) and, if
/// neither got removed, for (earlier, later). `collidable` excludes actors the
/// original wouldn't have in its list (mob nodes without a live actor).
pub fn check_all_collisions(
    pool: &mut super::actor::ActorPool,
    player_idx: Option<usize>,
    camera: &Camera,
    screen_w: i16,
    screen_h: i16,
    sprite_dims: &[(i16, i16)],
    moved: &dyn Fn(usize, &Actor) -> bool,
    collidable: &dyn Fn(&Actor) -> bool,
) -> Vec<CollisionResult> {
    let mut results = Vec::new();
    let indices = pool.active_indices();
    let mut removed: Vec<usize> = Vec::new();

    for j in 0..indices.len() {
        for i in 0..j {
            let (later, earlier) = (indices[j], indices[i]);
            if removed.contains(&later) || removed.contains(&earlier) {
                continue;
            }
            let (a, b) = match (pool.get(later), pool.get(earlier)) {
                (Some(a), Some(b)) => (a.clone(), b.clone()),
                _ => continue,
            };
            if a.flags.dead || b.flags.dead || !collidable(&a) || !collidable(&b) {
                continue;
            }
            if !moved(later, &a) && !moved(earlier, &b) {
                continue;
            }

            let (aw, ah) = sprite_dims.get(a.sprite_id as usize).copied().unwrap_or((16, 16));
            let (bw, bh) = sprite_dims.get(b.sprite_id as usize).copied().unwrap_or((16, 16));
            let (al, at, ar, ab) = actor_bounds(&a, aw, ah, camera, screen_w, screen_h);
            let (bl, bt, br, bb) = actor_bounds(&b, bw, bh, camera, screen_w, screen_h);
            if !intersects(al, at, ar, ab, bl, bt, br, bb) {
                continue;
            }

            for (first, second, fd, sd) in [(later, earlier, (aw, ah), (bw, bh)), (earlier, later, (bw, bh), (aw, ah))]
            {
                if removed.contains(&first) || removed.contains(&second) {
                    break;
                }
                let (mut f, mut s) = match (pool.get(first), pool.get(second)) {
                    (Some(f), Some(s)) => (f.clone(), s.clone()),
                    _ => break,
                };
                if log_pairs() {
                    eprintln!(
                        "pair a=({:?},spr{},st{},({},{},{})) b=({:?},spr{},st{},({},{},{}))",
                        f.class, f.sprite_id, f.state, f.x, f.y, f.z, s.class, s.sprite_id, s.state, s.x, s.y, s.z
                    );
                }
                let result = handle_collision(&mut f, &mut s, player_idx, first, second, fd, sd);
                if let Some(m) = pool.get_mut(first) {
                    *m = f;
                }
                if let Some(m) = pool.get_mut(second) {
                    *m = s;
                }
                if let Some(idx) = result.free_actor {
                    removed.push(idx);
                }
                if result.skier_eaten {
                    if let Some(p) = player_idx {
                        removed.push(p);
                    }
                }
                if result.score_delta != 0
                    || result.skier_crash
                    || result.skier_eaten
                    || result.skier_state.is_some()
                    || result.free_actor.is_some()
                    || result.sound.is_some()
                {
                    results.push(result);
                }
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skier() -> Actor {
        Actor { active: true, class: ActorClass::Skier, state: 0, vy: 16, ..Default::default() }
    }

    fn scenery(class: ActorClass, sprite: u16) -> Actor {
        Actor { active: true, class, sprite_id: sprite, ..Default::default() }
    }

    fn collide(s: &mut Actor, o: &mut Actor, sd: (i16, i16), od: (i16, i16)) -> CollisionResult {
        // Unless a test sets it, the skier moved at constant speed this tick.
        if s.prev_y.is_none() {
            s.prev_y = Some(s.y - s.vy);
        }
        let r = handle_collision(s, o, Some(0), 0, 1, sd, od);
        s.prev_y = None;
        r
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
        // FUN_00403a00 0x403dba..0x403dcd: the rock flips flat (0x56) and the
        // grounded skier still goes down (state 0xb), but without the -32.
        assert_eq!(o.sprite_id, 0x56);
        assert!(r.skier_crash);
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
        assert!(!crossed(3387, 3371, 3406, 3406));
        // Reached or passed this tick: yes.
        assert!(crossed(3419, 3403, 3406, 3406));
        assert!(crossed(3406, 3390, 3406, 3406));
        // Already past on the previous tick: no.
        assert!(!crossed(3435, 3419, 3406, 3406));
        // Moving uphill across it counts too.
        assert!(crossed(3400, 3410, 3406, 3406));
        // Standing exactly on it with no movement: no.
        assert!(!crossed(3406, 3406, 3406, 3406));
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

    #[test]
    fn getting_up_next_to_an_obstacle_is_not_a_crossing() {
        // Crashed at 3942 just past a tree at 3941; getting up moves the
        // skier 3942 -> 3943 while vy grows to 2. The previous y is the
        // start-of-tick position (3942), not y - vy (3941): no new hit.
        let mut s = skier();
        s.y = 3943;
        s.vy = 2;
        s.prev_y = Some(3942);
        let mut o = scenery(ActorClass::Tree, 51);
        o.y = 3941;
        let r = collide(&mut s, &mut o, (16, 32), (32, 64));
        assert!(!r.skier_crash);
    }
}
