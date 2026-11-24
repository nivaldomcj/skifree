//! What the original paints each frame, in its order.
//!
//! The original repaints only dirty rectangles (`FUN_00401060`): actors whose
//! boxes overlap are merged into one group, and each group is painted by
//! `FUN_00401540` back to front. Within a group it repeatedly picks the not-
//! yet-painted actor with the smallest key (a strict `<`, so on a tie the
//! earlier actor in the group's chain wins), where the key is
//!
//! - the actor's world y (`[actor+0x42]`), or
//! - `y - sprite height` when the actor's flag 0x40 is set — set on every
//!   sprite change from `FUN_00402310(sprite)`, which is true only for the two
//!   flat ground sprites 0x1b (mogul field) and 0x52 (small yellow patch), so
//!   anything standing on them is painted over them.
//!
//! Each sprite's box comes from `FUN_004014b0`: bottom-centre anchored at
//! (x, y - z), i.e. left = x - w/2, top = y - z - h in screen space.
//! White is the colour key (transparent), inside sprites too.
//!
//! Sorting the whole visible list by that key (stable, list order on ties)
//! gives the same picture: the order only matters between overlapping
//! sprites, and those always share a group. The one possible difference is
//! a tie between two overlapping sprites whose group chain was merged out of
//! list order.

use super::state::GameState;

/// One sprite to paint: its id and the top-left corner in client pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrawItem {
    pub sprite: u16,
    pub left: i16,
    pub top: i16,
}

/// The two flat ground sprites drawn under everything standing on them.
pub fn sorts_by_top(sprite: u16) -> bool {
    sprite == 0x1b || sprite == 0x52
}

/// The sprites of the current frame, in painting order.
pub fn draw_list(game: &GameState) -> Vec<DrawItem> {
    let mut items: Vec<(i16, DrawItem)> = Vec::new();
    for idx in game.pool.active_indices() {
        let Some(a) = game.pool.get(idx) else { continue };
        // A mob node without its live actor (only at the pool limit) isn't
        // drawn: the original has no actor to paint for it.
        if !a.is_live() {
            continue;
        }
        if a.sprite_id == 0 {
            continue;
        }
        let Some((w, h)) = game.sprites.lookup(a.sprite_id) else { continue };
        if w <= 0 || h <= 0 {
            continue;
        }
        let (sx, sy) = game.camera.world_to_screen(a.x, a.y, game.screen_w, game.screen_h);
        let left = sx.wrapping_sub(w / 2);
        let top = sy.wrapping_sub(a.z).wrapping_sub(h);
        if left >= game.screen_w || top >= game.screen_h || left + w <= 0 || top + h <= 0 {
            continue;
        }
        let key = if sorts_by_top(a.sprite_id) { a.y.wrapping_sub(h) } else { a.y };
        items.push((key, DrawItem { sprite: a.sprite_id, left, top }));
    }
    items.sort_by_key(|&(key, _)| key); // stable: list order on ties
    items.into_iter().map(|(_, item)| item).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::sprites::SpriteSizes;
    use crate::game::types::ActorClass;

    #[test]
    fn bottom_centre_anchor_and_y_order() {
        let mut sprites = SpriteSizes::placeholder();
        sprites.set(0x1b, (64, 32));
        let mut game = GameState::new_seeded(640, 480, 0, sprites);
        game.camera.cx = 0;
        game.camera.cy = 0;
        game.pool = crate::game::actor::ActorPool::new();
        let lower = game.pool.alloc(ActorClass::Effect, 0).unwrap();
        let upper = game.pool.alloc(ActorClass::Effect, 0).unwrap();
        let moguls = game.pool.alloc(ActorClass::Effect, 0).unwrap();
        for (idx, sprite, y) in [(lower, 1, 20), (upper, 2, 10), (moguls, 0x1b, 40)] {
            let a = game.pool.get_mut(idx).unwrap();
            a.sprite_id = sprite;
            a.x = 0;
            a.y = y;
        }
        let list = draw_list(&game);
        let order: Vec<u16> = list.iter().map(|d| d.sprite).collect();
        // moguls key = 40 - 32 = 8: under both; then y 10, then y 20.
        assert_eq!(order, vec![0x1b, 2, 1]);
        // (x, y) = (0, 20) is at screen (320, 160 + 20): box (312, 164).
        assert_eq!(list[2], DrawItem { sprite: 1, left: 312, top: 164 });
    }

    #[test]
    fn a_mob_node_without_its_actor_is_not_drawn() {
        let mut game = GameState::new_seeded(640, 480, 0, SpriteSizes::placeholder());
        game.pool = crate::game::actor::ActorPool::new();
        let node = game.pool.alloc_with_sprite(ActorClass::MonsterDownhill, 2).unwrap();
        assert!(draw_list(&game).is_empty(), "off screen: a bare node");
        game.pool.get_mut(node).unwrap().flags.on_screen = true;
        assert_eq!(draw_list(&game).len(), 1);
    }
}
