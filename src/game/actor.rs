use super::state_table::STATE_TO_SPRITE;
use super::types::*;

/// Fixed-size actor pool (100 slots, mirrors FUN_00404a00)
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
        let actors = vec![Actor::default(); 100];
        let free = (0..100).rev().collect();
        Self { actors, free, head: None }
    }

    pub fn alloc(&mut self, class: ActorClass, state: u16) -> Option<usize> {
        let idx = self.free.pop()?;
        let sprite_id = if (state as usize) < STATE_TO_SPRITE.len() { STATE_TO_SPRITE[state as usize] } else { 0 };
        self.actors[idx] = Actor { active: true, class, state, sprite_id, next: self.head, ..Default::default() };
        self.head = Some(idx);
        Some(idx)
    }

    /// Allocate an actor with a raw sprite id (FUN_004026a0), bypassing the
    /// state-to-sprite table. Used for class >= 0xb effect actors whose
    /// "state" field is really a sprite id (e.g. the snowboarder jump puff).
    pub fn alloc_with_sprite(&mut self, class: ActorClass, sprite_id: u16) -> Option<usize> {
        let idx = self.free.pop()?;
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

    pub fn active_count(&self) -> usize {
        self.active_indices().len()
    }
}
