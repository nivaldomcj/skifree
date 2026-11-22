use super::actor::ActorPool;
use super::state_table::STATE_TO_SPRITE;
use super::camera;
use super::types::Camera;
use super::collision;
use super::course;
use super::mob;
use super::monster;
use super::physics;
use super::populator;
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
    /// Freestyle, slalom, tree slalom run states (all three courses exist in
    /// the world at once, on their own x strips).
    pub courses: [CourseRun; 3],
    /// HUD clock (DAT_0040c944): runs only inside freestyle/slalom.
    pub course_clock_ms: u64,
    /// Set when a finish line is crossed; the platform layer saves scores.
    pub pending_score_save: bool,
    /// Top-10 tables (FUN_00402ec0), persisted by the platform layer.
    pub high_scores: crate::game::scoring::HighScores,
    /// The most recent finish, for the high-score display.
    pub last_finish: Option<FinishRecord>,
    pub elapsed_ms: u32,
    pub last_tick_ms: u32,
    pub screen_w: i16,
    pub screen_h: i16,
    pub sprite_dims: Vec<(i16, i16)>,
    /// Debug: log spawn materializations (SKIFREE_LOG_SPAWNS).
    pub log_spawns: bool,
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
            courses: [CourseRun::default(), CourseRun::default(), CourseRun::default()],
            course_clock_ms: 0,
            pending_score_save: false,
            high_scores: crate::game::scoring::HighScores::default(),
            last_finish: None,
            elapsed_ms: 0,
            last_tick_ms: 0,
            screen_w,
            screen_h,
            sprite_dims,
            log_spawns: std::env::var_os("SKIFREE_LOG_SPAWNS").is_some(),
        }
    }

    fn load_sprite_dims() -> Vec<(i16, i16)> {
        let meta_path = super::assets::asset_root().join("meta.json");
        let mut dims = vec![(16i16, 16i16); 90]; // default
        if let Ok(data) = std::fs::read_to_string(meta_path) {
            if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&data) {
                if let Some(resources) = meta.get("resources").and_then(|r| r.as_array()) {
                    for r in resources {
                        // Only bitmaps (RT_BITMAP = 2) are sprites: the icon
                        // resources (RT_ICON = 3, ids 1-6, 32x32) share ids
                        // with the skier sprites and must not overwrite them.
                        if r.get("type").and_then(|v| v.as_u64()) != Some(2) {
                            continue;
                        }
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
        self.courses = [CourseRun::default(), CourseRun::default(), CourseRun::default()];
        self.course_clock_ms = 0;
        self.pending_score_save = false;
        self.last_finish = None;
        self.camera = Camera::default();

        // Spawn skier at origin (FUN_00404a80: class 0, state 3, at 0,0,0)
        let idx = self.pool.alloc(ActorClass::Skier, 3);
        if let Some(idx) = idx {
            if let Some(actor) = self.pool.get_mut(idx) {
                actor.x = 0;
                actor.y = 0;
                actor.z = 0;
            }
            self.player_idx = Some(idx);
        }

        // Title splash (FUN_004051e0): logo, divider, copyright and version
        // as inert effect actors near the spawn point.
        self.spawn_title_splash();

        // Generate all three courses + poles + mobs + monsters (FUN_00404b50)
        self.spawn_events = course::generate_course(self.screen_w, self.screen_h, &mut self.rng);

        // Spawn initial actors from events
        self.consume_spawn_events();
    }

    /// Title splash actors (FUN_004051e0). Positions are derived from the
    /// sprite dimensions exactly like the original: logo top-left anchored,
    /// copyright/version right of it.
    fn spawn_title_splash(&mut self) {
        let dim = |id: usize| self.sprite_dims.get(id).copied().unwrap_or((16, 16));
        let (w53, h53) = dim(0x35);
        let (w55, h55) = dim(0x37);
        let (w56, h56) = dim(0x38);
        let lx = -0x28 - w53 / 2;
        let cx = w55.max(w56);
        let splash = [
            (0x35u16, lx, 0),
            (0x36, lx, h53 + 4),
            (0x37, cx, h55),
            (0x38, cx, h55 + h56 + 4),
        ];
        for (sprite, x, y) in splash {
            if let Some(idx) = self.pool.alloc_with_sprite(ActorClass::Effect, sprite) {
                if let Some(a) = self.pool.get_mut(idx) {
                    physics::set_position(a, x, y, 0);
                }
            }
        }
    }

    /// Main game tick (FUN_00401e50 + FUN_00401000)
    pub fn tick(&mut self, delta_ms: u32) {
        if self.paused {
            return;
        }

        self.elapsed_ms += delta_ms;
        self.last_tick_ms = delta_ms;

        // Player position before this tick's movement (course-line crossing
        // detection compares against it, like FUN_004028e0's saved x/y).
        let player_old_y = self.player_idx
            .and_then(|i| self.pool.get(i))
            .map(|a| a.y);

        // Actor loop (FUN_00401e50's first loop): update each actor, then
        // cull-test it against the padded viewport with the camera as it is
        // at that moment — actors ahead of the player in the list see the
        // pre-move view, since the camera follows the player only when the
        // player itself updates. Culled course actors keep their spawn-event
        // link until the end of the tick (the original frees them then), so
        // they cannot re-materialize within the same tick.
        let mut spawn_requests: Vec<SpawnRequest> = Vec::new();
        let mut unlinked_events: Vec<usize> = Vec::new();
        let indices = self.pool.active_indices();
        for idx in indices {
            let class = match self.pool.get(idx) {
                Some(a) => a.class,
                None => continue,
            };

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
                        if let Some(req) = mob::update_snowboarder(actor, &mut self.rng, self.fast_forward) {
                            spawn_requests.push(req);
                        }
                    }
                }
                ActorClass::Obstacle => {
                    if let Some(actor) = self.pool.get_mut(idx) {
                        mob::update_obstacle(actor, &mut self.rng, self.fast_forward);
                    }
                }
                // Proxies run in their own pass below (FUN_004040a0), in
                // mob-node order, after the actor loop.
                ActorClass::AmbientMob
                | ActorClass::MonsterLeft
                | ActorClass::MonsterRight
                | ActorClass::MonsterTop
                | ActorClass::MonsterBottom => {}
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
                // Classes >= 0xb get no behavior update in the original
                // (FUN_00401e50 only dispatches classes < 0xb): scenery and
                // effects just hold their sprite until they scroll away.
                ActorClass::SnowBump
                | ActorClass::Gate
                | ActorClass::Tree
                | ActorClass::Rock
                | ActorClass::RockLarge
                | ActorClass::Banner
                | ActorClass::Effect => {}
            }

            if Some(idx) == self.player_idx {
                // FUN_00402390 -> FUN_00402470: the camera follows the
                // player the moment it moves; scroll steps drain at the end.
                if let Some(player) = self.pool.get(idx) {
                    let (x, y) = (player.x, player.y);
                    self.camera.track(x, y);
                }
            } else {
                self.cull_check(idx, &mut unlinked_events);
            }
        }

        // Fulfill secondary-actor spawn requests (FUN_004020d0/0x26a0 + 0x2390).
        // A full pool drops the request, like the original's NULL return.
        for req in spawn_requests {
            self.fulfill_spawn(req);
        }

        // Course spawn-event lists (FUN_004046e0 x4), with this tick's view.
        self.consume_spawn_events();

        // Mob-node pass (FUN_004040a0): update each proxy, then materialize it
        // (FUN_00404130), in world-build node order.
        self.update_proxies();

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
            if let Some(state) = result.skier_state {
                if let Some(player_idx) = self.player_idx {
                    if let Some(actor) = self.pool.get_mut(player_idx) {
                        physics::change_state(actor, state);
                    }
                }
            }
            if let Some(idx) = result.free_actor {
                self.pool.free(idx);
            }
        }

        // Course start/finish/gate detection (FUN_00402c60 / FUN_00403250 /
        // FUN_00403180, called from the skier update in the original).
        if let Some(old_y) = player_old_y {
            self.check_course_crossings(old_y);
        }

        // Update camera to follow player; each 0x3c of travel spawns one
        // actor at the entering edge (FUN_004025c0), in the original's
        // order: bottom, top, right, left.
        let mut steps = camera::ScrollSteps::default();
        if let Some(player_idx) = self.player_idx {
            if let Some(player) = self.pool.get(player_idx) {
                steps = self.camera.follow(player.x, player.y);
            }
        }
        for (edge, count) in [
            (populator::Edge::Bottom, steps.down),
            (populator::Edge::Top, steps.up),
            (populator::Edge::Right, steps.right),
            (populator::Edge::Left, steps.left),
        ] {
            for _ in 0..count {
                // DAT_0040c6fc sums live actors only: off-screen proxies
                // are bare mob nodes in the original and do not count.
                let weight = populator::sprite_weight(
                    self.pool.active_indices().into_iter()
                        .filter(|&i| !self.is_proxy(i) || self.pool.get(i).is_some_and(|a| a.flags.on_screen))
                        .filter_map(|i| self.pool.get(i)),
                    &self.sprite_dims,
                );
                if let Some(req) = populator::pick_spawn(
                    edge,
                    &self.camera,
                    self.screen_w,
                    self.screen_h,
                    weight,
                    &mut self.rng,
                ) {
                    self.fulfill_spawn(req);
                }
            }
        }

        // Random crossing-skier spawn, 1-in-666 per tick (FUN_00401e50 tail):
        // class 3 state 0x1f entering above the viewport's top edge at a
        // random x across the window (FUN_00402350 direction 2), so it
        // overtakes the player from behind. The original's Win32 window-frame
        // offsets (DAT_0040c704/DAT_0040c5fc) are 0 in the port's camera
        // model, which tracks the viewport center.
        if self.rng.rand_range(0x29a) == 0 {
            let x = self.camera.cx - self.screen_w / 2 + self.rng.rand_range(self.screen_w as u16) as i16;
            let y = self.camera.cy - self.screen_h / 3 - 0x3c;
            self.fulfill_spawn(SpawnRequest {
                class: ActorClass::Obstacle,
                state: Some(0x1f),
                sprite: None,
                x,
                y,
                z: 0,
            });
        }

        // End of tick: culled course actors are gone; their spawn events
        // may re-materialize from the next tick on.
        for ev in unlinked_events {
            if let Some(event) = self.spawn_events.get_mut(ev) {
                event.actor_idx = None;
            }
        }
    }

    /// Cull test for one actor right after its update (FUN_00401e50 first
    /// loop: FUN_00401290 against the padded viewport DAT_0040c680). A
    /// course/secondary actor outside it is freed (its spawn-event index is
    /// queued for an end-of-tick link reset); a proxy keeps its node but its
    /// live actor is dropped (see `proxy_unlink`).
    fn cull_check(&mut self, idx: usize, unlinked_events: &mut Vec<usize>) {
        let a = match self.pool.get(idx) {
            Some(a) => a,
            None => return,
        };
        let proxy = self.is_proxy(idx);
        if proxy && !a.flags.on_screen {
            return; // bare mob node, no actor to cull
        }
        let (w, h) = self.sprite_dims.get(a.sprite_id as usize).copied().unwrap_or((16, 16));
        if collision::in_padded_view(a.x, a.y, a.z, w, h, &self.camera, self.screen_w, self.screen_h) {
            return;
        }
        if proxy {
            if let Some(a) = self.pool.get_mut(idx) {
                a.proxy_unlink = true;
            }
            return;
        }
        if let Some(ev) = a.event_idx {
            unlinked_events.push(ev);
        }
        self.pool.free(idx);
    }

    /// Course start/finish/gate detection (FUN_00402c60 freestyle,
    /// FUN_00403250 slalom, FUN_00403180 tree slalom). The original
    /// interpolates the exact crossing point; the port uses the
    /// post-tick position.
    fn check_course_crossings(&mut self, old_y: i16) {
        let Some(pi) = self.player_idx else { return };
        let Some(player) = self.pool.get(pi) else { return };
        let (px, py) = (player.x, player.y);

        const START_Y: i16 = 0x280;
        // courses[0] = slalom (left, timed, gated), courses[1] = tree slalom
        // (right, timed, gated), courses[2] = freestyle (center, style only).
        for ci in 0..3 {
            let ((xmin, xmax), finish, has_clock) = match ci {
                0 => (course::SLALOM_STRIP, course::SLALOM_FINISH_Y, true),
                1 => (course::TREE_SLALOM_STRIP, course::LONG_FINISH_Y, true),
                _ => (course::FREESTYLE_STRIP, course::LONG_FINISH_Y, false),
            };
            if !self.courses[ci].active {
                // Cross the start line downward inside the strip.
                if old_y <= START_Y && py > START_Y && px >= xmin && px <= xmax {
                    self.courses[ci].active = true;
                    self.courses[ci].start_ms = self.elapsed_ms as i64;
                    if has_clock {
                        self.course_clock_ms = 0;
                    }
                    if ci != 2 {
                        self.courses[ci].next_gate = self.first_gate_index(xmin, xmax);
                    }
                }
                continue;
            }

            if has_clock {
                self.course_clock_ms =
                    (self.elapsed_ms as i64 - self.courses[ci].start_ms).max(0) as u64;
            }
            if py > finish {
                self.courses[ci].active = false;
                self.courses[ci].finished = true;
                self.courses[ci].finish_ms = self.course_clock_ms;
                // High-score entry (FUN_00402ec0): time for slalom/giant
                // slalom, style for freestyle.
                let rank = if ci == 2 {
                    self.high_scores.insert_style(self.scoring.score)
                } else {
                    self.high_scores.insert_time(ci, self.course_clock_ms)
                };
                self.last_finish = Some(FinishRecord {
                    course: ci,
                    ms: self.course_clock_ms,
                    style: self.scoring.score,
                    rank,
                });
                self.pending_score_save = true;
                // The skier stands after finishing (FUN_00402e80), unless
                // crashed or mid-crash-landing.
                if let Some(pi) = self.player_idx {
                    if let Some(p) = self.pool.get_mut(pi) {
                        if p.state != 0x0b && p.state != 0x11 {
                            let state = if p.z < 1 { 3 } else { 0x0e };
                            physics::change_state(p, state);
                        }
                    }
                }
            } else if py <= START_Y {
                // Skied back above the start line: the run is abandoned.
                self.courses[ci].active = false;
            } else if ci != 2 {
                self.check_gate_pass(ci, px, py);
            }
        }

        // Style points accumulate only inside the freestyle course
        // (FUN_00403420 gates on DAT_0040c954, the freestyle flag).
        self.scoring.enabled = self.courses[2].active;
    }

    /// First gate record index within a course strip (DAT_0040c94c/c950).
    fn first_gate_index(&self, xmin: i16, xmax: i16) -> Option<usize> {
        self.spawn_events
            .iter()
            .position(|e| e.class == 0x0c && e.x >= xmin && e.x <= xmax)
    }

    /// Gate passing (FUN_00402c60 tail + FUN_00403130): the gate's flag turns
    /// sprite 0x19 (passed) or 0x1a (missed, with a 5 s clock penalty).
    fn check_gate_pass(&mut self, ci: usize, px: i16, py: i16) {
        let Some(gi) = self.courses[ci].next_gate else { return };
        let Some(gate) = self.spawn_events.get(gi) else { return };
        if gate.y >= py {
            return;
        }
        let (gx, gsprite, actor_idx) = (gate.x, gate.sprite, gate.actor_idx);
        let passed = match gsprite {
            Some(0x17) => gx < px, // sprite 0x17: pass on its right
            Some(0x18) => px < gx, // sprite 0x18: pass on its left
            _ => true,
        };
        let new_sprite = if passed { 0x19 } else { 0x1a };
        if !passed {
            self.courses[ci].start_ms -= 5000;
            self.course_clock_ms =
                (self.elapsed_ms as i64 - self.courses[ci].start_ms).max(0) as u64;
        }
        if let Some(ev) = self.spawn_events.get_mut(gi) {
            ev.sprite = Some(new_sprite);
        }
        if let Some(ai) = actor_idx {
            if let Some(a) = self.pool.get_mut(ai) {
                a.sprite_id = new_sprite;
            }
        }
        let (xmin, xmax) = if ci == 0 {
            course::SLALOM_STRIP
        } else {
            course::TREE_SLALOM_STRIP
        };
        self.courses[ci].next_gate = self
            .spawn_events
            .iter()
            .enumerate()
            .skip(gi + 1)
            .find(|(_, e)| e.class == 0x0c && e.x >= xmin && e.x <= xmax)
            .map(|(i, _)| i);
    }

    /// HUD Time value in ms (DAT_0040c944).
    pub fn hud_time_ms(&self) -> u64 {
        self.course_clock_ms
    }

    /// HUD Dist value in meters (FUN_00401b80): remaining inside a course,
    /// distance traveled when free-skiing, 0 without a player.
    pub fn hud_dist_m(&self) -> i32 {
        let Some(pi) = self.player_idx else { return 0 };
        let Some(p) = self.pool.get(pi) else { return 0 };
        let y = p.y as i32;
        let units = if self.courses[0].active {
            0x21c0 - y
        } else if self.courses[1].active || self.courses[2].active {
            0x4100 - y
        } else {
            y
        };
        units / 16
    }

    /// HUD Speed value in m/s (FUN_00401b80): downhill velocity scaled by
    /// the last frame delta (units are 1/16 m per tick).
    pub fn hud_speed(&self) -> i32 {
        let Some(pi) = self.player_idx else { return 0 };
        let Some(p) = self.pool.get(pi) else { return 0 };
        if self.last_tick_ms == 0 {
            return 0;
        }
        (p.vy as i32 * 1000) / ((self.last_tick_ms as i32) << 4)
    }

    /// Spawn a secondary actor from a behavior request (FUN_004020d0 via
    /// `state`, or FUN_004026a0 via `sprite`), placing it with FUN_00402390.
    fn fulfill_spawn(&mut self, req: SpawnRequest) {
        let idx = match (req.state, req.sprite) {
            (Some(state), None) => self.pool.alloc(req.class, state),
            (None, Some(sprite)) => self.pool.alloc_with_sprite(req.class, sprite),
            _ => None,
        };
        if let Some(idx) = idx {
            if let Some(actor) = self.pool.get_mut(idx) {
                physics::set_position(actor, req.x, req.y, req.z);
            }
        }
    }

    /// Consume spawn events that the player has scrolled past (FUN_004046e0)
    /// Proxy classes (4-8): ambient mobs and monsters, which the original
    /// runs as mob nodes with an actor only while on screen.
    fn is_proxy(&self, idx: usize) -> bool {
        self.pool.get(idx).is_some_and(|a| {
            matches!(
                a.class,
                ActorClass::AmbientMob
                    | ActorClass::MonsterLeft
                    | ActorClass::MonsterRight
                    | ActorClass::MonsterTop
                    | ActorClass::MonsterBottom
            )
        })
    }

    /// Mob-node pass (FUN_004040a0). Each node is updated (FUN_004041c0:
    /// class 4 via FUN_00404290, monsters via FUN_00404350) and then, if its
    /// y is inside the scan window, offered to FUN_00404130, which gives it
    /// a live actor when the node's point (x, y - z) lies strictly inside the
    /// padded viewport (nodes carry the empty sprite-table entry 0, 0x0 —
    /// verified via Frida). The live actor persists until the cull pass
    /// drops it by its real sprite box. Nodes are visited in world-build
    /// order: ambient mobs, then monsters.
    fn update_proxies(&mut self) {
        let nodes: Vec<usize> = self.spawn_events.iter()
            .filter(|e| (4..=8).contains(&e.class))
            .filter_map(|e| e.actor_idx)
            .collect();
        for idx in nodes {
            let player = self.player_idx.and_then(|pi| self.pool.get(pi).cloned());
            let mut request = None;
            if let Some(actor) = self.pool.get_mut(idx) {
                match actor.class {
                    // The original rolls only while the node has a live actor
                    // ([node+0] != 0 in FUN_00404290).
                    ActorClass::AmbientMob => {
                        let live = actor.flags.on_screen;
                        request = mob::update_ambient(actor, &mut self.rng, live);
                    }
                    ActorClass::MonsterLeft | ActorClass::MonsterRight
                    | ActorClass::MonsterTop | ActorClass::MonsterBottom => {
                        monster::update_monster(
                            actor,
                            player.as_ref(),
                            &mut self.rng,
                            self.elapsed_ms,
                            self.screen_w,
                            self.screen_h,
                        );
                    }
                    _ => continue,
                }
            }
            if let Some(req) = request {
                self.fulfill_spawn(req);
            }
            let (camera, sw, sh) = (self.camera.clone(), self.screen_w, self.screen_h);
            if let Some(a) = self.pool.get_mut(idx) {
                if a.proxy_unlink {
                    // Culled last cull pass: the link drops only now, and the
                    // node cannot re-materialize in the same pass.
                    a.proxy_unlink = false;
                    a.flags.on_screen = false;
                } else if !a.flags.on_screen
                    && collision::in_event_window(a.y, &camera, sw, sh)
                    && collision::in_padded_view(a.x, a.y, a.z, 0, 0, &camera, sw, sh)
                {
                    a.flags.on_screen = true;
                }
            }
        }
    }

    fn consume_spawn_events(&mut self) {
        for (event_pos, event) in self.spawn_events.iter_mut().enumerate() {
            if event.actor_idx.is_some() {
                continue; // already spawned
            }

            // Proxy classes (4-8: ambient mobs, monsters) are allocated from
            // world build: in the original their mob nodes update (drift,
            // wrap, roll RNG) globally wherever the player is; whether they
            // count as "on screen" is decided per tick by in_padded_view.
            // Course events materialize like FUN_004046e0 + FUN_00404130:
            // event y inside the scan window AND the sprite's draw box
            // overlapping the padded viewport. Culled actors (event link
            // reset) re-materialize here when scrolled back into range.
            let in_window = (4..=8).contains(&event.class) || {
                let sprite = event.sprite.unwrap_or_else(|| {
                    STATE_TO_SPRITE.get(event.state as usize).copied().unwrap_or(0)
                });
                let (w, h) = self.sprite_dims.get(sprite as usize).copied().unwrap_or((16, 16));
                collision::in_event_window(event.y, &self.camera, self.screen_w, self.screen_h)
                    // Event nodes carry z = 0 (verified via Frida); the
                    // actor's own z only applies once materialized.
                    && collision::in_padded_view(event.x, event.y, 0, w, h, &self.camera, self.screen_w, self.screen_h)
            };
            if in_window {
                let class = match ActorClass::from_u16(event.class) {
                    Some(c) => c,
                    None => continue,
                };

                let idx = match event.sprite {
                    Some(sprite) => self.pool.alloc_with_sprite(class, sprite),
                    None => self.pool.alloc(class, event.state),
                };
                if let Some(idx) = idx {
                    if let Some(actor) = self.pool.get_mut(idx) {
                        actor.x = event.x;
                        actor.y = event.y;
                        actor.z = event.z;
                        if class == ActorClass::AmbientMob {
                            // param carries the proxy's initial vy (-2/+2)
                            actor.vy = event.param;
                        }
                        actor.event_idx = Some(event_pos);
                    }
                    if self.log_spawns {
                        eprintln!(
                            "spawn ms={} cls={} spr={:?} x={} y={} rng={}",
                            self.elapsed_ms,
                            event.class,
                            event.sprite,
                            event.x,
                            event.y,
                            self.rng.call_count()
                        );
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

    /// Keyboard jump (Insert / numpad0): smaller hop that scrubs speed.
    pub fn jump_keyboard(&mut self) {
        if let Some(idx) = self.player_idx {
            if let Some(actor) = self.pool.get_mut(idx) {
                skier::jump_keyboard(actor);
            }
        }
    }

    /// Diagonal steering: set a skiing state directly (1/4 slight, 3/6 hard).
    pub fn steer_absolute(&mut self, target_state: u16) {
        if let Some(idx) = self.player_idx {
            if let Some(actor) = self.pool.get_mut(idx) {
                skier::steer_absolute(actor, target_state);
            }
        }
    }

    /// Whether the skier is still present (false once eaten).
    pub fn has_player(&self) -> bool {
        self.player_idx.is_some()
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
    fn sprite_dims_ignore_icon_resources() {
        // Icon resources 1-6 (32x32) share ids with the skier sprites; only
        // bitmaps may feed sprite_dims. Skipped when assets aren't extracted.
        if !super::super::assets::asset_root().join("meta.json").exists() {
            return;
        }
        let g = game_with_player();
        assert_eq!(g.sprite_dims[1], (16, 32));
        assert_eq!(g.sprite_dims[3], (24, 28));
    }

    #[test]
    fn course_events_materialize_only_inside_padded_view() {
        // 704 wide: padded view spans screen x -120..824, i.e. world x
        // -472..472 around the player. The slalom start marker at x = -576
        // stays a bare event; the freestyle ones at +-160 materialize once
        // the skier has descended far enough for y = 640 to enter the window.
        let mut g = GameState::new_seeded(704, 681, 0);
        g.new_game();
        g.steer_keyboard(false, false, false, true);
        for _ in 0..15 {
            g.tick(40);
        }
        let xs: Vec<i16> = g.pool.active_indices().into_iter()
            .filter_map(|i| g.pool.get(i))
            .filter(|a| a.sprite_id == 0x39 || a.sprite_id == 0x3a)
            .map(|a| a.x)
            .collect();
        assert!(xs.contains(&-160) && xs.contains(&160), "{xs:?}");
        assert!(!xs.contains(&-576), "{xs:?}");
    }

    #[test]
    fn offscreen_proxies_stay_unmaterialized() {
        let mut g = GameState::new_seeded(704, 681, 0);
        g.new_game();
        for _ in 0..3 {
            g.tick(40);
        }
        let monsters: Vec<bool> = g.pool.active_indices().into_iter()
            .filter_map(|i| g.pool.get(i))
            .filter(|a| matches!(a.class, ActorClass::MonsterLeft | ActorClass::MonsterRight
                | ActorClass::MonsterTop | ActorClass::MonsterBottom))
            .map(|a| a.flags.on_screen)
            .collect();
        assert_eq!(monsters.len(), 4, "monster nodes exist from world build");
        assert!(monsters.iter().all(|&live| !live), "far-away monsters have no live actor");
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

    #[test]
    fn keyboard_jump_on_ground_uses_vz2() {
        let mut g = game_with_player();
        g.jump_keyboard();
        let a = g.pool.get(g.player_idx.unwrap()).unwrap();
        assert_eq!(a.vz, 2);
        assert_eq!(a.state, 0x0d);
    }

    #[test]
    fn keyboard_jump_scrubs_downhill_speed() {
        let mut g = game_with_player();
        let idx = g.player_idx.unwrap();
        g.pool.get_mut(idx).unwrap().vy = 10;
        g.jump_keyboard();
        assert_eq!(g.pool.get(idx).unwrap().vy, 6); // 10 - 4
    }

    #[test]
    fn keyboard_jump_no_scrub_below_threshold() {
        let mut g = game_with_player();
        let idx = g.player_idx.unwrap();
        g.pool.get_mut(idx).unwrap().vy = 4;
        g.jump_keyboard();
        assert_eq!(g.pool.get(idx).unwrap().vy, 4); // unchanged (not > 4)
    }

    #[test]
    fn steer_absolute_sets_state_on_ground() {
        let mut g = game_with_player();
        g.steer_absolute(6);
        assert_eq!(g.pool.get(g.player_idx.unwrap()).unwrap().state, 6);
        g.steer_absolute(1);
        assert_eq!(g.pool.get(g.player_idx.unwrap()).unwrap().state, 1);
    }

    #[test]
    fn steer_absolute_ignored_in_air() {
        let mut g = game_with_player();
        let idx = g.player_idx.unwrap();
        let a = g.pool.get_mut(idx).unwrap();
        a.z = 5;
        physics::change_state(a, 0);
        g.steer_absolute(6);
        assert_eq!(g.pool.get(idx).unwrap().state, 0); // unchanged
    }

    fn seed_with_first_roll_zero(n: u16) -> u32 {
        (0..1_000_000u32)
            .find(|&s| {
                let mut r = Rng::new(s);
                r.rand_range(n) == 0
            })
            .expect("an LCG seed with first roll 0 exists")
    }

    #[test]
    fn fulfill_spawn_by_state_uses_sprite_table() {
        let mut g = GameState::new_seeded(640, 480, 1);
        g.fulfill_spawn(SpawnRequest {
            class: ActorClass::Obstacle,
            state: Some(0x21),
            sprite: None,
            x: -112,
            y: 498,
            z: 0x20,
        });
        let a = g.pool.get(g.pool.head.unwrap()).unwrap();
        assert_eq!(a.class, ActorClass::Obstacle);
        assert_eq!(a.state, 0x21);
        assert_eq!(a.sprite_id, crate::game::state_table::STATE_TO_SPRITE[0x21 as usize]);
        assert_eq!((a.x, a.y, a.z), (-112, 498, 0x20));
    }

    #[test]
    fn fulfill_spawn_by_sprite_sets_raw_sprite() {
        let mut g = GameState::new_seeded(640, 480, 1);
        g.fulfill_spawn(SpawnRequest {
            class: ActorClass::Effect,
            state: None,
            sprite: Some(0x52),
            x: 196,
            y: 298,
            z: 6,
        });
        let a = g.pool.get(g.pool.head.unwrap()).unwrap();
        assert_eq!(a.class, ActorClass::Effect);
        assert_eq!(a.sprite_id, 0x52);
        assert_eq!(a.state, 0);
        assert_eq!((a.x, a.y, a.z), (196, 298, 6));
    }

    #[test]
    fn tick_fulfills_snowboarder_jump_effect() {
        let seed = seed_with_first_roll_zero(100);
        let mut g = GameState::new_seeded(640, 480, seed);
        let idx = g.pool.alloc(ActorClass::Snowboarder, 0x1e).unwrap();
        {
            let a = g.pool.get_mut(idx).unwrap();
            a.x = 200;
            a.y = 300;
            a.z = 0;
        }
        g.tick(40);
        let effect = g.pool.active_indices().into_iter()
            .filter_map(|i| g.pool.get(i))
            .find(|a| a.class == ActorClass::Effect)
            .expect("jump effect actor spawned");
        assert_eq!(effect.sprite_id, 0x52);
        assert_eq!((effect.x, effect.y), (196, 298));
        assert_eq!(g.pool.get(idx).unwrap().state, 0x1b);
    }

    #[test]
    fn ambient_spawn_event_applies_initial_vy() {
        let mut g = GameState::new_seeded(640, 480, 1);
        g.spawn_events.push(SpawnEvent {
            actor_idx: None,
            event_type: 0x04,
            state: 0x27,
            class: 4,
            sprite: None,
            x: -112,
            y: 100,
            z: 0x20,
            param: -2,
        });
        g.tick(40);
        let mob = g.pool.active_indices().into_iter()
            .filter_map(|i| g.pool.get(i))
            .find(|a| a.class == ActorClass::AmbientMob)
            .expect("ambient mob materialized");
        assert_eq!(mob.vy, -2);
        assert_eq!(mob.z, 0x20);
        // It drifted up this tick (proxy integration, no gravity).
        assert_eq!(mob.y, 98);
    }
}

#[cfg(test)]
mod course_tests {
    use super::*;

    fn game() -> GameState {
        let mut g = GameState::new_seeded(640, 480, 1);
        g.new_game();
        g
    }

    fn place_player(g: &mut GameState, x: i16, y: i16, vy: i16) {
        let idx = g.player_idx.unwrap();
        let a = g.pool.get_mut(idx).unwrap();
        a.x = x;
        a.y = y;
        a.vy = vy;
        a.state = 0;
    }

    #[test]
    fn slalom_start_begins_clock() {
        let mut g = game();
        place_player(&mut g, -400, 0x280, 4);
        assert_eq!(g.hud_time_ms(), 0);
        g.tick(40);
        assert!(g.courses[0].active, "slalom should activate crossing 0x280");
        assert!(g.courses[1..].iter().all(|c| !c.active));
        g.tick(40);
        assert_eq!(g.hud_time_ms(), 40, "clock runs after the start line");
    }

    #[test]
    fn freestyle_runs_without_clock() {
        let mut g = game();
        place_player(&mut g, 0, 0x280, 4);
        g.tick(40);
        assert!(g.courses[2].active, "freestyle activates in the center strip");
        assert!(!g.courses[0].active && !g.courses[1].active);
        g.tick(40);
        assert_eq!(g.hud_time_ms(), 0, "freestyle has no clock (style-scored)");
    }

    #[test]
    fn skiing_back_above_start_abandons_run() {
        let mut g = game();
        place_player(&mut g, -400, 0x280, 4);
        g.tick(40);
        assert!(g.courses[0].active);
        place_player(&mut g, -400, 0x280, -2);
        g.tick(40);
        assert!(!g.courses[0].active, "run abandoned above the start line");
    }

    #[test]
    fn gate_pass_and_miss() {
        let mut g = game();
        place_player(&mut g, -400, 0x280, 4);
        g.tick(40);
        assert!(g.courses[0].active);
        let gi = g.courses[0].next_gate.expect("slalom has gates");
        let (gy, gsprite) = {
            let e = &g.spawn_events[gi];
            (e.y, e.sprite)
        };
        assert_eq!(gsprite, Some(0x17));

        // Pass on the correct side (sprite 0x17: gate.x < player.x).
        place_player(&mut g, -400, gy - 1, 4);
        g.tick(40);
        assert_eq!(g.spawn_events[gi].sprite, Some(0x19), "passed gate turns sprite 0x19");

        // Miss the next gate on the wrong side.
        let gi2 = g.courses[0].next_gate.unwrap();
        let gy2 = g.spawn_events[gi2].y;
        let clock_before = g.hud_time_ms();
        place_player(&mut g, -300, gy2 - 1, 4); // sprite 0x18: needs x < gate.x
        g.tick(40);
        assert_eq!(g.spawn_events[gi2].sprite, Some(0x1a), "missed gate turns sprite 0x1a");
        assert_eq!(
            g.hud_time_ms(),
            clock_before + 40 + 5000,
            "miss applies the 5s penalty on top of the tick"
        );
    }

    #[test]
    fn finish_records_time_and_flags_save() {
        let mut g = game();
        place_player(&mut g, -400, 0x280, 4);
        g.tick(40);
        assert!(g.courses[0].active);
        place_player(&mut g, -400, 0x21bf, 4);
        g.tick(40);
        assert!(!g.courses[0].active);
        assert!(g.courses[0].finished);
        assert!(g.pending_score_save);
        // High-score entry (FUN_00402ec0): slalom time recorded at rank 0,
        // and the skier stands (FUN_00402e80).
        let f = g.last_finish.expect("finish recorded");
        assert_eq!(f.course, 0);
        assert_eq!(f.rank, Some(0));
        assert_eq!(g.high_scores.slalom_ms.len(), 1);
        assert_eq!(g.pool.get(g.player_idx.unwrap()).unwrap().state, 3);
    }

    #[test]
    fn freestyle_finish_records_style_not_time() {
        let mut g = game();
        // Enter freestyle and earn style points.
        place_player(&mut g, 0, 0x280, 4);
        g.tick(40);
        assert!(g.courses[2].active);
        g.scoring.add(-22);
        place_player(&mut g, 0, 0x4100 - 1, 4);
        g.tick(40);
        let f = g.last_finish.expect("finish recorded");
        assert_eq!(f.course, 2);
        assert_eq!(f.style, -22);
        assert_eq!(g.high_scores.freestyle_style, vec![-22]);
        assert!(g.high_scores.slalom_ms.is_empty());
        assert!(g.high_scores.giant_slalom_ms.is_empty());
    }

    #[test]
    fn dist_counts_down_in_course_and_up_outside() {
        let mut g = game();
        // Free-skiing above the start: distance traveled.
        place_player(&mut g, 0x200, 0x100, 0);
        assert_eq!(g.hud_dist_m(), 0x100 / 16);
        // In freestyle: remaining.
        place_player(&mut g, -400, 0x280, 4);
        g.tick(40);
        assert_eq!(g.hud_dist_m(), (0x21c0 - 0x284) / 16);
    }
}

#[cfg(test)]
mod scoring_gate_tests {
    use super::*;

    #[test]
    fn style_only_accumulates_in_freestyle() {
        let mut g = GameState::new_seeded(640, 480, 1);
        g.new_game();
        let idx = g.player_idx.unwrap();
        // Outside any course: points are ignored (FUN_00403420 gate).
        g.scoring.add(100);
        assert_eq!(g.scoring.score, 0);
        // Enter freestyle: the gate opens.
        {
            let a = g.pool.get_mut(idx).unwrap();
            a.x = 0;
            a.y = 0x280;
            a.vy = 4;
            a.state = 0;
        }
        g.tick(40);
        assert!(g.courses[2].active);
        g.scoring.add(100);
        assert_eq!(g.scoring.score, 100);
    }
}
