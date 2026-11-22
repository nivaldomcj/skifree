use super::actor::ActorPool;
use super::types::Camera;
use super::collision;
use super::course;
use super::mob;
use super::monster;
use super::physics;
use super::rng::Rng;
use super::scoring::Scoring;
use super::skier;
use super::types::*;

/// Central game state — replaces all the original's global variables
pub struct GameState {
    pub pool: ActorPool,
    pub camera: Camera,
    pub player_idx: Option<usize>,
    pub rng: Rng,
    pub scoring: Scoring,
    pub fast_forward: bool,
    pub paused: bool,
    pub course_type: CourseType,
    pub spawn_events: Vec<SpawnEvent>,
    pub elapsed_ms: u32,
    pub last_tick_ms: u32,
    pub screen_w: i16,
    pub screen_h: i16,
    pub sprite_dims: Vec<(i16, i16)>,
}

impl GameState {
    pub fn new(screen_w: i16, screen_h: i16) -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u32;
        Self::new_seeded(screen_w, screen_h, seed)
    }

    pub fn new_seeded(screen_w: i16, screen_h: i16, seed: u32) -> Self {
        // Load sprite dimensions from meta.json
        let sprite_dims = Self::load_sprite_dims();

        Self {
            pool: ActorPool::new(),
            camera: Camera::default(),
            player_idx: None,
            rng: Rng::new(seed),
            scoring: Scoring::new(),
            fast_forward: false,
            paused: false,
            course_type: CourseType::Freestyle,
            spawn_events: Vec::new(),
            elapsed_ms: 0,
            last_tick_ms: 0,
            screen_w,
            screen_h,
            sprite_dims,
        }
    }

    fn load_sprite_dims() -> Vec<(i16, i16)> {
        let meta_path = std::path::Path::new("assets/meta.json");
        let mut dims = vec![(16i16, 16i16); 90]; // default
        if let Ok(data) = std::fs::read_to_string(meta_path) {
            if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&data) {
                if let Some(resources) = meta.get("resources").and_then(|r| r.as_array()) {
                    for r in resources {
                        if let (Some(id), Some(w), Some(h)) = (
                            r.get("id").and_then(|v| v.as_u64()),
                            r.get("width").and_then(|v| v.as_i64()),
                            r.get("height").and_then(|v| v.as_i64()),
                        ) {
                            let id = id as usize;
                            if id < dims.len() {
                                dims[id] = (w as i16, h as i16);
                            }
                        }
                    }
                }
            }
        }
        dims
    }

    /// Start a new game (FUN_00404970 + FUN_00404a80)
    pub fn new_game(&mut self) {
        self.pool = ActorPool::new();
        self.scoring = Scoring::new();
        self.fast_forward = false;
        self.paused = false;
        self.elapsed_ms = 0;

        // Spawn skier at origin
        let idx = self.pool.alloc(ActorClass::Skier, 3);
        if let Some(idx) = idx {
            if let Some(actor) = self.pool.get_mut(idx) {
                actor.x = 0;
                actor.y = 0;
                actor.z = 0;
            }
            self.player_idx = Some(idx);
        }

        // Generate course
        self.spawn_events = course::generate_course(
            self.course_type,
            self.screen_w,
            self.screen_h,
            0,
            0,
        );

        // Spawn initial actors from events
        self.consume_spawn_events();
    }

    /// Main game tick (FUN_00401e50 + FUN_00401000)
    pub fn tick(&mut self, delta_ms: u32) {
        if self.paused {
            return;
        }

        self.elapsed_ms += delta_ms;
        self.last_tick_ms = delta_ms;

        // Consume spawn events as player scrolls
        self.consume_spawn_events();

        // Update all actors
        let indices = self.pool.active_indices();
        for idx in indices {
            let class = match self.pool.get(idx) {
                Some(a) => a.class,
                None => continue,
            };

            let player = self.player_idx.and_then(|pi| self.pool.get(pi).cloned());

            match class {
                ActorClass::Skier => {
                    if let Some(actor) = self.pool.get_mut(idx) {
                        skier::update_skier(actor, self.fast_forward);
                    }
                }
                ActorClass::Dog => {
                    if let Some(actor) = self.pool.get_mut(idx) {
                        mob::update_dog(actor, &mut self.rng, self.fast_forward);
                    }
                }
                ActorClass::Snowboarder => {
                    if let Some(actor) = self.pool.get_mut(idx) {
                        mob::update_snowboarder(actor, &mut self.rng, self.fast_forward);
                    }
                }
                ActorClass::Obstacle => {
                    if let Some(actor) = self.pool.get_mut(idx) {
                        mob::update_obstacle(actor, &mut self.rng, self.fast_forward);
                    }
                }
                ActorClass::AmbientMob => {
                    if let Some(actor) = self.pool.get_mut(idx) {
                        mob::update_ambient(actor, &mut self.rng, self.fast_forward);
                    }
                }
                ActorClass::MonsterLeft | ActorClass::MonsterRight |
                ActorClass::MonsterTop | ActorClass::MonsterBottom => {
                    if let Some(actor) = self.pool.get_mut(idx) {
                        monster::update_monster(actor, player.as_ref(), &mut self.rng, self.elapsed_ms);
                    }
                }
                ActorClass::Eating => {
                    if let Some(actor) = self.pool.get_mut(idx) {
                        mob::update_eating(actor, self.fast_forward);
                    }
                }
                ActorClass::Lift => {
                    if let Some(actor) = self.pool.get_mut(idx) {
                        mob::update_lift(actor, &mut self.rng, self.fast_forward);
                    }
                }
            }
        }

        // Check collisions
        let results = collision::check_all_collisions(
            &mut self.pool,
            self.player_idx,
            &self.camera,
            self.screen_w,
            self.screen_h,
            &self.sprite_dims,
        );

        for result in results {
            if result.score_delta != 0 {
                self.scoring.add(result.score_delta);
            }
            if result.skier_eaten {
                if let (Some(player_idx), Some(monster_idx)) = (self.player_idx, result.eaten_by) {
                    if let Some(monster) = self.pool.get_mut(monster_idx) {
                        monster::eat_skier(monster, player_idx, self.elapsed_ms);
                    }
                    self.pool.free(player_idx);
                    self.player_idx = None;
                }
            }
            if result.skier_crash {
                if let Some(player_idx) = self.player_idx {
                    if let Some(actor) = self.pool.get_mut(player_idx) {
                        physics::change_state(actor, 0x0b);
                    }
                }
            }
        }

        // Update camera to follow player
        if let Some(player_idx) = self.player_idx {
            if let Some(player) = self.pool.get(player_idx) {
                self.camera.follow(player.x, player.y);
            }
        }

        // Random ambient spawn (1-in-666)
        if self.rng.rand_range(0x29a) == 0 {
            // Spawn a random mob
            let _ = self.pool.alloc(ActorClass::AmbientMob, 0x1f);
        }
    }

    /// Consume spawn events that the player has scrolled past (FUN_004046e0)
    fn consume_spawn_events(&mut self) {
        let player_y = self.player_idx
            .and_then(|idx| self.pool.get(idx))
            .map(|a| a.y)
            .unwrap_or(0);

        for event in &mut self.spawn_events {
            if event.actor_idx.is_some() {
                continue; // already spawned
            }

            // Spawn when player is within screen height of the event
            let spawn_threshold = player_y + self.screen_h;
            if event.y <= spawn_threshold {
                let class = match ActorClass::from_u16(event.class) {
                    Some(c) => c,
                    None => continue,
                };

                if let Some(idx) = self.pool.alloc(class, event.state) {
                    if let Some(actor) = self.pool.get_mut(idx) {
                        actor.x = event.x;
                        actor.y = event.y;
                        actor.z = event.z;
                        actor.event_idx = Some(event.actor_idx.unwrap_or(0));
                    }
                    event.actor_idx = Some(idx);
                }
            }
        }
    }

    /// Handle keyboard steering
    pub fn steer_keyboard(&mut self, left: bool, right: bool, up: bool, down: bool) {
        if let Some(idx) = self.player_idx {
            if let Some(actor) = self.pool.get_mut(idx) {
                skier::steer_keyboard(actor, left, right, up, down);
            }
        }
    }

    /// Handle mouse steering
    pub fn steer_mouse(&mut self, dx: i16, dy: i16) {
        if let Some(idx) = self.player_idx {
            if let Some(actor) = self.pool.get_mut(idx) {
                skier::steer_mouse(actor, dx, dy);
            }
        }
    }

    /// Handle click (FUN_004066d0): jump on the ground, cycle tricks mid-air.
    pub fn click(&mut self) {
        if let Some(idx) = self.player_idx {
            if let Some(actor) = self.pool.get_mut(idx) {
                if actor.state == 0x0b {
                    return; // crashed
                }
                if actor.z == 0 {
                    actor.vz = 4; // jump (mouse uses vz=4, unlike keyboard's 2)
                    physics::change_state(actor, 0x0d);
                } else if actor.state != 0x11 {
                    let next = match actor.state {
                        0x0d => 0x12,
                        0x0e => 0x14,
                        0x0f => 0x15,
                        0x12 => 0x13,
                        0x13 => 0x0d,
                        _ => actor.state,
                    };
                    physics::change_state(actor, next);
                }
            }
        }
    }

    /// Get player position for HUD
    pub fn player_pos(&self) -> Option<(i16, i16, i16)> {
        self.player_idx
            .and_then(|idx| self.pool.get(idx))
            .map(|a| (a.x, a.y, a.z))
    }

    /// Get player velocity for HUD
    pub fn player_velocity(&self) -> Option<(i16, i16, i16)> {
        self.player_idx
            .and_then(|idx| self.pool.get(idx))
            .map(|a| (a.vx, a.vy, a.vz))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game_with_player() -> GameState {
        let mut g = GameState::new_seeded(640, 480, 1);
        g.new_game();
        g
    }

    #[test]
    fn click_on_ground_jumps_with_vz4() {
        let mut g = game_with_player();
        g.click();
        let a = g.pool.get(g.player_idx.unwrap()).unwrap();
        assert_eq!(a.vz, 4);
        assert_eq!(a.state, 0x0d);
    }

    #[test]
    fn click_mid_air_cycles_tricks() {
        let mut g = game_with_player();
        let idx = g.player_idx.unwrap();
        // Force the skier airborne in the base jump pose.
        {
            let a = g.pool.get_mut(idx).unwrap();
            a.z = 5;
            physics::change_state(a, 0x0d);
        }
        g.click();
        assert_eq!(g.pool.get(idx).unwrap().state, 0x12);
        g.click();
        assert_eq!(g.pool.get(idx).unwrap().state, 0x13);
        g.click();
        assert_eq!(g.pool.get(idx).unwrap().state, 0x0d);
    }

    #[test]
    fn click_crashed_does_nothing() {
        let mut g = game_with_player();
        let idx = g.player_idx.unwrap();
        {
            let a = g.pool.get_mut(idx).unwrap();
            physics::change_state(a, 0x0b);
        }
        g.click();
        let a = g.pool.get(idx).unwrap();
        assert_eq!(a.state, 0x0b);
        assert_eq!(a.vz, 0);
    }
}
