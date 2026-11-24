use super::state_table::STATE_TO_SPRITE;
use super::types::{Actor, ActorClass};

/// The original's actor pool size (`FUN_00404a00`): at most this many live
/// actors.
pub const LIVE_ACTORS: usize = 100;
/// Slots for the mob nodes (chairlift chairs and monsters, classes 4-8; a world
/// has 28). The original keeps them outside the actor pool and gives a node
/// a pool actor only while it is on screen; the port merges node and actor,
/// so it reserves their slots here and counts a node as live only while
/// `flags.on_screen` (docs/porting-guide.md, "Monster/mob proxy vs actor").
const NODE_SLOTS: usize = 32;

/// Fixed-size actor pool: `LIVE_ACTORS` live actors, like the original's,
/// plus the mob nodes' slots.
pub struct ActorPool {
    actors: Vec<Actor>,
    free: Vec<usize>,
    pub head: Option<usize>, // active linked list head
}

impl Default for ActorPool {
    fn default() -> Self {
        Self::new()
    }
}

impl ActorPool {
    pub fn new() -> Self {
        let slots = LIVE_ACTORS + NODE_SLOTS;
        let actors = vec![Actor::default(); slots];
        let free = (0..slots).rev().collect();
        Self { actors, free, head: None }
    }

    /// A free slot for an actor of `class`: a mob node always gets one (it
    /// isn't live yet); any other actor only below `LIVE_ACTORS` live ones,
    /// where the original's allocation fails.
    fn take_slot(&mut self, class: ActorClass) -> Option<usize> {
        if !class.is_mob_node() && !self.has_live_room() {
            return None;
        }
        self.free.pop()
    }

    /// Whether one more actor can go live (a new actor, or a mob node coming
    /// on screen).
    pub fn has_live_room(&self) -> bool {
        self.live_count() < LIVE_ACTORS
    }

    /// Live actors as the original counts them: every actor except mob
    /// nodes that are off screen.
    pub fn live_count(&self) -> usize {
        self.active_indices().into_iter().filter(|&i| self.actors[i].is_live()).count()
    }

    pub fn alloc(&mut self, class: ActorClass, state: u16) -> Option<usize> {
        let idx = self.take_slot(class)?;
        let sprite_id = if (state as usize) < STATE_TO_SPRITE.len() { STATE_TO_SPRITE[state as usize] } else { 0 };
        self.actors[idx] = Actor { active: true, class, state, sprite_id, next: self.head, ..Default::default() };
        self.head = Some(idx);
        Some(idx)
    }

    /// Allocate an actor with a raw sprite id (`FUN_004026a0`), bypassing the
    /// state-to-sprite table. Used for class >= 0xb effect actors whose
    /// "state" field is really a sprite id (e.g. the snowboarder jump puff).
    pub fn alloc_with_sprite(&mut self, class: ActorClass, sprite_id: u16) -> Option<usize> {
        let idx = self.take_slot(class)?;
        self.actors[idx] = Actor { active: true, class, state: 0, sprite_id, next: self.head, ..Default::default() };
        self.head = Some(idx);
        Some(idx)
    }

    pub fn free(&mut self, idx: usize) {
        if idx >= self.actors.len() || !self.actors[idx].active {
            return;
        }
        // Unlink from active list
        if self.head == Some(idx) {
            self.head = self.actors[idx].next;
        } else {
            let mut prev = self.head;
            while let Some(p) = prev {
                if self.actors[p].next == Some(idx) {
                    self.actors[p].next = self.actors[idx].next;
                    break;
                }
                prev = self.actors[p].next;
            }
        }
        self.actors[idx].active = false;
        self.actors[idx].next = None;
        self.free.push(idx);
    }

    pub fn get(&self, idx: usize) -> Option<&Actor> {
        if idx < self.actors.len() && self.actors[idx].active {
            Some(&self.actors[idx])
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, idx: usize) -> Option<&mut Actor> {
        if idx < self.actors.len() && self.actors[idx].active {
            Some(&mut self.actors[idx])
        } else {
            None
        }
    }

    /// Iterate over active actor indices
    pub fn active_indices(&self) -> Vec<usize> {
        let mut result = Vec::new();
        let mut cur = self.head;
        while let Some(idx) = cur {
            if self.actors[idx].active {
                result.push(idx);
                cur = self.actors[idx].next;
            } else {
                break;
            }
        }
        result
    }

    /// Every slot, free or not (the live actors' and the mob nodes').
    pub fn slots(&self) -> usize {
        self.actors.len()
    }

    pub fn active_count(&self) -> usize {
        self.active_indices().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A world's 28 mob nodes, all off screen.
    fn pool_with_nodes() -> ActorPool {
        let mut pool = ActorPool::new();
        for _ in 0..24 {
            pool.alloc(ActorClass::LiftChair, 0x27).unwrap();
        }
        for class in [
            ActorClass::MonsterUphill,
            ActorClass::MonsterDownhill,
            ActorClass::MonsterLeftEdge,
            ActorClass::MonsterRightEdge,
        ] {
            pool.alloc(class, 0x2a).unwrap();
        }
        pool
    }

    #[test]
    fn off_screen_nodes_leave_the_original_100_live_actors() {
        let mut pool = pool_with_nodes();
        assert_eq!(pool.live_count(), 0);
        for _ in 0..LIVE_ACTORS {
            assert!(pool.alloc(ActorClass::Tree, 0).is_some());
        }
        assert_eq!(pool.live_count(), LIVE_ACTORS);
        assert_eq!(pool.alloc(ActorClass::Tree, 0), None, "the 101st live actor fails, like the original");
        assert!(!pool.has_live_room(), "and no node can come on screen");
    }

    #[test]
    fn on_screen_nodes_count_as_live() {
        let mut pool = pool_with_nodes();
        let nodes = pool.active_indices();
        for &i in &nodes[..10] {
            pool.get_mut(i).unwrap().flags.on_screen = true;
        }
        assert_eq!(pool.live_count(), 10);
        for _ in 0..LIVE_ACTORS - 10 {
            assert!(pool.alloc(ActorClass::Tree, 0).is_some());
        }
        assert_eq!(pool.alloc(ActorClass::Tree, 0), None);
    }
}
