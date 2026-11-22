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

/// Collision dispatch (FUN_00403a00)
/// Returns (score_delta, skier_eaten)
pub fn handle_collision(
    a: &mut Actor,
    b: &mut Actor,
    _player_idx: Option<usize>,
    a_idx: usize,
    b_idx: usize,
) -> CollisionResult {
    let mut result = CollisionResult::default();

    // Only the player (class 0) triggers most collisions
    if a.class == ActorClass::Skier || b.class == ActorClass::Skier {
        let (_skier, other) = if a.class == ActorClass::Skier { (&*a, &*b) } else { (&*b, &*a) };
        let _skier_idx = if a.class == ActorClass::Skier { a_idx } else { b_idx };

        match other.class {
            ActorClass::Obstacle => {
                // Hit tree/rock — crash
                result.score_delta = -0x20;
                result.skier_crash = true;
            }
            ActorClass::MonsterLeft | ActorClass::MonsterRight |
            ActorClass::MonsterTop | ActorClass::MonsterBottom => {
                // Eaten by monster
                result.skier_eaten = true;
                result.eaten_by = Some(if a.class == ActorClass::Skier { b_idx } else { a_idx });
            }
            ActorClass::Dog => {
                // Trip over dog
                result.score_delta = -0x10;
                result.skier_crash = true;
            }
            ActorClass::Snowboarder => {
                // Crash into snowboarder
                result.score_delta = -0x20;
                result.skier_crash = true;
            }
            ActorClass::Lift => {
                // Pass through lift
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
    pub skier_eaten: bool,
    pub eaten_by: Option<usize>,
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
            let result = handle_collision(&mut a_clone, &mut b_clone, player_idx, idx_a, idx_b);

            // Write back changes
            if let Some(a_mut) = pool.get_mut(idx_a) {
                *a_mut = a_clone;
            }
            if let Some(b_mut) = pool.get_mut(idx_b) {
                *b_mut = b_clone;
            }

            if result.score_delta != 0 || result.skier_crash || result.skier_eaten {
                results.push(result);
            }
        }
    }

    results
}
