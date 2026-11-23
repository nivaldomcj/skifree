//! Dynamic obstacle populator (`FUN_004025c0` + friends).
//!
//! The original fills the slope procedurally: every time the camera scrolls
//! past a 0x3c boundary it spawns one actor at the entering screen edge. The
//! actor type is chosen by a zone test on the spawn position, then by a
//! per-zone probability table, and is skipped entirely when the live-actor
//! density (sum of sprite areas) exceeds a per-zone cap.

use super::course::{COURSE_START_Y, FREESTYLE_STRIP, LONG_FINISH_Y, SLALOM_FINISH_Y, SLALOM_STRIP, TREE_SLALOM_STRIP};
use super::rng::Rng;
use super::state_table::CLASS_INITIAL_STATE;
use super::types::{Actor, ActorClass, Camera, SpawnRequest};

/// Picker result meaning "spawn nothing" (0x12 in the original).
const NOTHING: u16 = 0x12;

/// Viewport padding used by the density metric: (w + 0xf0) * (h + 0xf0)
/// (`DAT_0040c748`, from the client rect inflated by 0x78 on every side).
pub fn padded_area(screen_w: i16, screen_h: i16) -> i32 {
    (i32::from(screen_w) + 0xf0) * (i32::from(screen_h) + 0xf0)
}

/// Live-actor density (`DAT_0040c6fc`): sum of sprite areas (width * height,
/// sprite-table entry +0xe) over all active actors.
pub fn sprite_weight<'a>(actors: impl Iterator<Item = &'a Actor>, sprite_dims: &[(i16, i16)]) -> i32 {
    actors
        .map(|a| {
            let (w, h) = sprite_dims.get(a.sprite_id as usize).copied().unwrap_or((16, 16));
            i32::from(w) * i32::from(h)
        })
        .sum()
}

/// Edge directions, matching `FUN_004024f0`'s parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Left = 0,
    Right = 1,
    Top = 2,
    Bottom = 3,
}

/// Spawn position on an entering edge (`FUN_004024f0`). The camera anchors
/// the player at half the width and a third of the height (`FUN_00406060`);
/// the original's window-frame offsets are 0 here. The RNG is consumed
/// exactly like the original: one `rand()` for the along-edge axis. The sums
/// wrap like the original's 16-bit stores (deep runs on tall windows pass
/// `i16::MAX`).
pub fn edge_position(edge: Edge, camera: &Camera, screen_w: i16, screen_h: i16, rng: &mut Rng) -> (i16, i16) {
    let left = camera.cx.wrapping_sub(screen_w / 2);
    let top = camera.cy.wrapping_sub(screen_h / 3);
    let along = |rng: &mut Rng, len: i16| rng.rand_range(len as u16) as i16;
    match edge {
        Edge::Left => (left.wrapping_sub(0x3c), top.wrapping_add(along(rng, screen_h))),
        Edge::Right => (left.wrapping_add(screen_w).wrapping_add(0x3c), top.wrapping_add(along(rng, screen_h))),
        Edge::Top => (left.wrapping_add(along(rng, screen_w)), top.wrapping_sub(0x3c)),
        Edge::Bottom => (left.wrapping_add(along(rng, screen_w)), top.wrapping_add(screen_h).wrapping_add(0x3c)),
    }
}

/// Default open-slope picker (`FUN_004026f0`).
fn pick_default(weight: i32, area: i32, rng: &mut Rng) -> u16 {
    if weight > area >> 5 {
        return NOTHING;
    }
    let r = rng.rand_range(1000);
    if r < 0x32 {
        10 // lift
    } else if r < 500 {
        0xd // tree
    } else if r < 700 {
        0xf // large rock
    } else if r < 0x2ee {
        0xb // snow bump
    } else if r < 0x3b6 {
        0xe // rock
    } else if r < 0x3ca {
        0x10 // banner
    } else if r < 0x3de {
        1 // dog
    } else {
        2 // snowboarder
    }
}

/// Slalom strip picker (`FUN_00402770`): always snow bumps, tighter cap.
fn pick_slalom(weight: i32, area: i32) -> u16 {
    if weight > area >> 6 {
        NOTHING
    } else {
        0xb
    }
}

/// Tree-slalom strip picker (`FUN_004027a0`): trees with a rare snowboarder.
fn pick_tree_slalom(weight: i32, area: i32, rng: &mut Rng) -> u16 {
    if weight > area >> 4 {
        return NOTHING;
    }
    if rng.rand_range(0x40) == 0 {
        2
    } else {
        0xd
    }
}

/// Freestyle strip picker (`FUN_004027e0`): the mixed table.
fn pick_freestyle(weight: i32, area: i32, rng: &mut Rng) -> u16 {
    if weight > area >> 5 {
        return NOTHING;
    }
    let r = rng.rand_range(100);
    if r < 2 {
        10
    } else if r < 0x14 {
        0xd
    } else if r < 0x32 {
        0xf
    } else if r < 0x3c {
        0xb
    } else if r < 0x50 {
        0xe
    } else {
        0x10
    }
}

/// Zone test + pick for a spawn position (`FUN_004025c0`'s region dispatch):
/// each course strip, from its start line to its finish line, has its own
/// picker (docs/course-populator.md, "Zones").
fn pick_class(x: i16, y: i16, weight: i32, area: i32, rng: &mut Rng) -> u16 {
    let within = |(lo, hi): (i16, i16), finish: i16| (lo..=hi).contains(&x) && (COURSE_START_Y..=finish).contains(&y);
    if within(SLALOM_STRIP, SLALOM_FINISH_Y) {
        pick_slalom(weight, area)
    } else if within(TREE_SLALOM_STRIP, LONG_FINISH_Y) {
        pick_tree_slalom(weight, area, rng)
    } else if within(FREESTYLE_STRIP, LONG_FINISH_Y) {
        pick_freestyle(weight, area, rng)
    } else {
        pick_default(weight, area, rng)
    }
}

/// Raw sprite for classes >= 0xb (`FUN_00402850`).
fn sprite_for_class(class: u16, rng: &mut Rng) -> u16 {
    match class {
        0xb => 0x1b,
        0xd => match rng.rand_range(8) {
            0 => 0x32,
            1 => 0x33,
            _ => 0x31,
        },
        0xe => {
            if rng.rand_range(4) == 0 {
                0x2e
            } else {
                0x2d
            }
        }
        0xf => {
            if rng.rand_range(3) == 0 {
                0x30
            } else {
                0x2f
            }
        }
        0x10 => 0x34,
        _ => 0,
    }
}

/// Pick one actor to spawn at an entering edge (`FUN_004025c0`), or None when
/// the density cap or the picker says nothing.
pub fn pick_spawn(
    edge: Edge,
    camera: &Camera,
    screen_w: i16,
    screen_h: i16,
    weight: i32,
    rng: &mut Rng,
) -> Option<SpawnRequest> {
    let (x, y) = edge_position(edge, camera, screen_w, screen_h, rng);
    let class = pick_class(x, y, weight, padded_area(screen_w, screen_h), rng);
    if class == NOTHING {
        return None;
    }
    let (state, sprite) = if class < 0xb {
        (Some(CLASS_INITIAL_STATE[class as usize]), None)
    } else {
        (None, Some(sprite_for_class(class, rng)))
    };
    Some(SpawnRequest { class: ActorClass::from_u16(class)?, state, sprite, x, y, z: 0 })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cam() -> Camera {
        Camera { cx: 0, cy: 0, ..Default::default() }
    }

    #[test]
    fn edge_positions_are_offscreen() {
        let mut rng = Rng::new(1);
        let (x, _) = populator_edge(Edge::Left, &mut rng);
        assert_eq!(x, -320 - 0x3c);
        let (x, _) = populator_edge(Edge::Right, &mut rng);
        assert_eq!(x, 320 + 0x3c);
        let (_, y) = populator_edge(Edge::Top, &mut rng);
        assert_eq!(y, -160 - 0x3c); // top = cy - h/3
        let (_, y) = populator_edge(Edge::Bottom, &mut rng);
        assert_eq!(y, -160 + 480 + 0x3c);
    }

    fn populator_edge(e: Edge, rng: &mut Rng) -> (i16, i16) {
        edge_position(e, &cam(), 640, 480, rng)
    }

    #[test]
    fn density_cap_blocks_default_picks() {
        let area = padded_area(640, 480);
        let mut rng = Rng::new(1);
        // weight above area/32 -> nothing regardless of roll
        assert_eq!(pick_default((area >> 5) + 1, area, &mut rng), NOTHING);
        assert_eq!(pick_slalom((area >> 6) + 1, area), NOTHING);
        assert_eq!(pick_tree_slalom((area >> 4) + 1, area, &mut rng), NOTHING);
        assert_eq!(pick_freestyle((area >> 5) + 1, area, &mut rng), NOTHING);
    }

    #[test]
    fn zone_a_is_all_snow_bumps() {
        for w in 0..3 {
            assert_eq!(pick_slalom(w, padded_area(640, 480)), 0xb);
        }
    }

    #[test]
    fn default_picker_distribution_boundaries() {
        let area = padded_area(640, 480);
        // Seed 1 sequence: 41, 18467, 6334, ... -> rand_range(1000) = 41 -> lift
        let mut rng = Rng::new(1);
        assert_eq!(pick_default(0, area, &mut rng), 10);
    }

    #[test]
    fn sprite_table_matches_original() {
        let mut rng = Rng::new(1);
        assert_eq!(sprite_for_class(0xb, &mut rng), 0x1b);
        assert_eq!(sprite_for_class(0x10, &mut rng), 0x34);
        // seed 1: rand_range(8) = 1 -> big pine
        assert_eq!(sprite_for_class(0xd, &mut rng), 0x33);
        // seed 1: rand_range(4) = 1 -> 0x2d
        assert_eq!(sprite_for_class(0xe, &mut rng), 0x2d);
        // seed 1: rand_range(3) = 2 -> 0x2f
        assert_eq!(sprite_for_class(0xf, &mut rng), 0x2f);
    }

    #[test]
    fn zone_dispatch_uses_spawn_position() {
        let area = padded_area(640, 480);
        let mut rng = Rng::new(1);
        // Inside the slalom strip -> snow bump
        assert_eq!(pick_class(-0x200, 0x1000, 0, area, &mut rng), 0xb);
        // Inside the tree-slalom strip -> tree or snowboarder (seed 1: rand(64)=41 -> tree)
        assert_eq!(pick_class(0x180, 0x1000, 0, area, &mut rng), 0xd);
        // Outside everything -> default table (fresh seed 1: 41 -> lift)
        assert_eq!(pick_class(0, 0, 0, area, &mut Rng::new(1)), 10);
    }
}
