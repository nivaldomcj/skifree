use super::actor::ActorPool;
use super::collision;
use super::course;
use super::course::Course;
use super::mob;
use super::monster;
use super::physics;
use super::populator;
use super::rng::Rng;
use super::scoring::Scoring;
use super::skier;
use super::sound::Sound;
use super::sprites::SpriteSizes;
use super::state_table::STATE_TO_SPRITE;
use super::types::Camera;
use super::types::{Actor, ActorClass, CourseRun, FinishRecord, SpawnEvent, SpawnRequest};

/// What the collision pass's "moved this tick" test compares: position,
/// class and sprite at the start of the tick.
type MovedSnapshot = (i16, i16, i16, ActorClass, u16);

/// What the collision pass's "moved this tick" test compares (the original's
/// flag 0x20: set on a position change and on a sprite change).
fn moved_snapshot(a: &Actor) -> MovedSnapshot {
    (a.x, a.y, a.z, a.class, a.sprite_id)
}

/// Central game state — replaces all the original's global variables
#[expect(clippy::struct_excessive_bools, reason = "the original's global flags, one field each")]
pub struct GameState {
    pub pool: ActorPool,
    pub camera: Camera,
    pub player_idx: Option<usize>,
    pub rng: Rng,
    pub scoring: Scoring,
    pub fast_forward: bool,
    /// F3 pause (`DAT_0040c650`).
    pub paused: bool,
    /// The game timer runs (`DAT_0040c6d0`): ticks only advance the game while
    /// it does. Stopped by F3 and by losing focus / minimizing.
    pub timer_running: bool,
    /// Keyboard and mouse input are processed (`DAT_0040c67c`): only while the
    /// window is active and not minimized (`FUN_00405a10`).
    pub input_enabled: bool,
    pub spawn_events: Vec<SpawnEvent>,
    /// Freestyle, slalom, tree slalom run states (all three courses exist in
    /// the world at once, on their own x strips).
    pub courses: [CourseRun; 3],
    /// Course clock `DAT_0040c944` (ms, signed: it is `start − now` on the
    /// start tick). It runs only inside the timed courses (slalom and tree
    /// slalom); freestyle has none.
    pub course_clock_ms: i64,
    /// Set when a finish line is crossed; the platform layer saves scores.
    pub pending_score_save: bool,
    /// Top-10 tables (`FUN_00402ec0`), persisted by the platform layer.
    pub high_scores: crate::game::scoring::HighScores,
    /// The most recent finish, for the high-score display.
    pub last_finish: Option<FinishRecord>,
    /// Every finish since the scores were last saved (`rank_finish_in`):
    /// the original saves each one (`FUN_00402ec0` runs once per finish).
    unsaved_finishes: Vec<FinishRecord>,
    /// Sprite area the original's density sum `DAT_0040c6fc` keeps after a
    /// collision removes an actor (a collected rock, the eaten skier):
    /// `FUN_00401350` never subtracts it, only a new game resets the sum.
    pub removed_area: i32,
    /// Game time when the timer last stopped (`DAT_0040c600`), to take the
    /// T steps made while stopped back off the course clock at the restart.
    pause_ref_ms: u32,
    /// A finish just happened: the platform shows the high-score message
    /// box (`FUN_00402ec0`'s `MessageBoxA`) and clears this.
    pub show_high_scores: bool,
    pub elapsed_ms: u32,
    /// Wall clock: like `elapsed_ms` but keeps running while paused. The
    /// original's `DAT_0040c698` is wall time; only the course clock is
    /// compensated for pauses (`FUN_00404ad0` shifts the course start), so the
    /// monster's meal keeps progressing during a pause.
    pub wall_ms: u32,
    /// Status display values as last painted (`FUN_00401b80`): Time, Dist,
    /// Speed, Style. Repainted only by `paint_status`.
    pub status_values: [String; 4],
    /// Wall time since the status display was last painted (the original's
    /// `DAT_0040c698 - DAT_0040c5dc`). Never reset by a new game: F2
    /// neither repaints the display nor restarts its schedule.
    pub status_age_ms: u32,
    /// The status display was painted since the replay runner last looked
    /// (it clears this after each tick's record).
    pub status_painted: bool,
    pub last_tick_ms: u32,
    pub screen_w: i16,
    pub screen_h: i16,
    /// Sprite sizes by id (the original's sprite table).
    pub sprites: SpriteSizes,
    /// Debug: log spawn materializations (`SKIFREE_LOG_SPAWNS`).
    pub log_spawns: bool,
    /// `SKIFREE_LOG_UPDATES`: print each actor-loop update (tick, class,
    /// state, position) to stderr, to diff against `tools/vm/trace_updates.js`.
    pub log_updates: bool,
    /// Seed every new game starts from: fixed for seeded games (replays, the
    /// patched original's srand(0)); `None` re-seeds from the clock, like the
    /// original's srand(GetTickCount()) in `FUN_00404970`.
    pub fixed_seed: Option<u32>,
    /// Last mouse position seen (`DAT_0040c700/DAT_0040c70c`; `None` until the
    /// first mouse message, `DAT_0040c760`). Survives new games, like the
    /// original's globals.
    pub mouse_last: Option<(i16, i16)>,
    /// Main-tick counter (port tick N = the original's `FUN_00401e50` call N).
    pub tick_count: u32,
    /// Sounds played since the platform layer (or the replay runner) last
    /// took them, in the original's call order (`FUN_00402ba0`). Each one
    /// cuts off the one before, so of one tick's sounds only the last is
    /// heard.
    pub sounds: Vec<Sound>,
}

impl GameState {
    /// A game whose every new game seeds from the clock.
    pub fn new(screen_w: i16, screen_h: i16, sprites: SpriteSizes) -> Self {
        let mut g = Self::new_seeded(screen_w, screen_h, clock_seed(), sprites);
        g.fixed_seed = None;
        g
    }

    /// A game whose every new game starts from `seed`, with the loaded
    /// sprites' sizes.
    pub fn new_seeded(screen_w: i16, screen_h: i16, seed: u32, sprites: SpriteSizes) -> Self {
        Self {
            pool: ActorPool::new(),
            camera: Camera::default(),
            player_idx: None,
            rng: Rng::new(seed),
            fixed_seed: Some(seed),
            scoring: Scoring::new(),
            fast_forward: false,
            paused: false,
            timer_running: true,
            input_enabled: true,
            spawn_events: Vec::new(),
            courses: [CourseRun::default(), CourseRun::default(), CourseRun::default()],
            course_clock_ms: 0,
            pending_score_save: false,
            high_scores: crate::game::scoring::HighScores::default(),
            last_finish: None,
            removed_area: 0,
            pause_ref_ms: 0,
            unsaved_finishes: Vec::new(),
            show_high_scores: false,
            elapsed_ms: 0,
            wall_ms: 0,
            status_values: Default::default(),
            status_age_ms: 0,
            status_painted: false,
            last_tick_ms: 0,
            screen_w,
            screen_h,
            sprites,
            log_spawns: std::env::var_os("SKIFREE_LOG_SPAWNS").is_some(),
            log_updates: std::env::var_os("SKIFREE_LOG_UPDATES").is_some(),
            mouse_last: None,
            tick_count: 0,
            sounds: Vec::new(),
        }
    }

    /// Start a new game (`FUN_00404970` + `FUN_00404a80`)
    pub fn new_game(&mut self) {
        // FUN_00404970 re-seeds on every new game, F2 restarts included.
        self.rng = Rng::new(self.fixed_seed.unwrap_or_else(clock_seed));
        self.pool = ActorPool::new();
        self.removed_area = 0; // 0x404998
        self.scoring = Scoring::new();
        self.fast_forward = false;
        // F2 unpauses (FUN_00406500 -> FUN_00405760) and the new game starts
        // the timer (FUN_00404a80 -> FUN_00404ad0).
        self.paused = false;
        self.resume_timer();
        self.elapsed_ms = 0;
        self.wall_ms = 0;
        self.tick_count = 0;
        self.courses = [CourseRun::default(), CourseRun::default(), CourseRun::default()];
        self.course_clock_ms = 0;
        self.pending_score_save = false;
        self.last_finish = None;
        self.unsaved_finishes.clear();
        self.show_high_scores = false;
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

        // The status display is first painted when the window shows
        // (WM_PAINT -> FUN_00406970 -> FUN_00401b80), after the first game
        // is set up. Later new games (F2) leave it alone.
        if self.status_values[0].is_empty() {
            self.paint_status();
        }
    }

    /// The high-score message box text for the last finish (caption
    /// `scoring::HIGH_SCORES_CAPTION`).
    pub fn high_score_text(&self) -> Option<String> {
        let f = self.last_finish?;
        Some(self.high_scores.finish_text(f.course, f.ms, f.style, f.rank))
    }

    /// Wall time that passed while the game was blocked (a modal message
    /// box): the original's `GetTickCount` keeps running, so the monster's
    /// meal and the status display's repaint schedule see it.
    pub fn add_wall_time(&mut self, ms: u32) {
        self.wall_ms = self.wall_ms.wrapping_add(ms);
        self.status_age_ms = self.status_age_ms.saturating_add(ms);
    }

    /// The client area changed size (`WM_SIZE` -> `FUN_00405fa0`): the view
    /// follows the new size (anchor at w/2, h/3), and the status window is
    /// moved to the new top-right corner with `MoveWindow`(..., bRepaint =
    /// TRUE) (`FUN_00406890`), which repaints it before the next tick.
    pub fn resize(&mut self, w: i16, h: i16) {
        if (w, h) == (self.screen_w, self.screen_h) {
            return;
        }
        self.screen_w = w;
        self.screen_h = h;
        self.paint_status();
    }

    /// Repaint the status display (`FUN_00401b80`): take the current values
    /// and restart the 327 ms repaint schedule.
    pub fn paint_status(&mut self) {
        self.status_values = [
            self.scoring.format_time(self.hud_time_ms()),
            self.scoring.format_dist(self.hud_dist_m()),
            self.scoring.format_speed(self.hud_speed()),
            self.scoring.format_style(),
        ];
        self.status_age_ms = 0;
        self.status_painted = true;
    }

    /// Title splash actors (`FUN_004051e0`). Positions are derived from the
    /// sprite dimensions exactly like the original: logo top-left anchored,
    /// copyright/version right of it.
    fn spawn_title_splash(&mut self) {
        let dim = |id: u16| self.sprites.get(id);
        // FUN_004051e0 reads the sprite table: the divider (0x36) sits its
        // *own* height + 4 below the logo's top (sprite-table +0x36c = h54;
        // verified via Frida: (-86, 14)), not the logo's height.
        let (w53, _) = dim(0x35);
        let (_, h54) = dim(0x36);
        let (w55, h55) = dim(0x37);
        let (w56, h56) = dim(0x38);
        let lx = -0x28 - w53 / 2;
        let cx = w55.max(w56);
        let splash = [(0x35u16, lx, 0), (0x36, lx, h54 + 4), (0x37, cx, h55), (0x38, cx, h55 + h56 + 4)];
        for (sprite, x, y) in splash {
            if let Some(idx) = self.pool.alloc_with_sprite(ActorClass::Effect, sprite) {
                if let Some(a) = self.pool.get_mut(idx) {
                    physics::set_position(a, x, y, 0);
                }
            }
        }
    }

    /// Main game tick (`FUN_00401e50` + `FUN_00401000`)
    #[expect(clippy::too_many_lines, reason = "one pass in the original's order; split, the order is harder to check")]
    pub fn tick(&mut self, delta_ms: u32) {
        // Timer stopped (F3, or the window lost focus / was minimized):
        // nothing updates, but wall time still passes.
        self.wall_ms = self.wall_ms.wrapping_add(delta_ms);
        self.status_age_ms = self.status_age_ms.saturating_add(delta_ms);
        if !self.timer_running {
            return;
        }

        self.elapsed_ms += delta_ms;
        self.last_tick_ms = delta_ms;

        // Player position before this tick's movement (course-line crossing
        // detection compares against it, like FUN_004028e0's saved x/y).
        let player_old = self.player_idx.and_then(|i| self.pool.get(i)).map(|a| (a.x, a.y));

        // Actor loop (FUN_00401e50's first loop): update each actor, then
        // cull-test it against the padded viewport with the camera as it is
        // at that moment — actors ahead of the player in the list see the
        // pre-move view, since the camera follows the player only when the
        // player itself updates. Culled course actors keep their spawn-event
        // link until the end of the tick (the original frees them then), so
        // they cannot re-materialize within the same tick.
        let mut spawn_requests: Vec<SpawnRequest> = Vec::new();
        let mut unlinked_events: Vec<usize> = Vec::new();
        // Positions and sprites at the start of the tick, for the collision
        // pass's "moved this tick" test: the original's flag 0x20, cleared in
        // this loop and set by FUN_00402390 on a position change and by
        // FUN_00402180 on a sprite change. Actors created during the tick
        // count as moved. Every pool slot, node slots included.
        let start_pos: Vec<Option<MovedSnapshot>> =
            (0..self.pool.slots()).map(|i| self.pool.get(i).map(moved_snapshot)).collect();
        // Previous-position copies for the collision crossing test.
        for idx in self.pool.active_indices() {
            if let Some(a) = self.pool.get_mut(idx) {
                a.prev_y = Some(a.y);
            }
        }
        let indices = self.pool.active_indices();
        for idx in indices {
            let class = match self.pool.get(idx) {
                Some(a) => a.class,
                None => continue,
            };
            if self.log_updates {
                if let Some(a) = self.pool.get(idx) {
                    eprintln!(
                        "upd tick={} cls={} st={} spr={} x={} y={} z={}",
                        self.tick_count, a.class as u16, a.state, a.sprite_id, a.x, a.y, a.z
                    );
                }
            }

            match class {
                ActorClass::Skier => {
                    if let Some(actor) = self.pool.get_mut(idx) {
                        let points = skier::update_skier(actor, self.fast_forward, &mut self.sounds);
                        if points != 0 {
                            self.scoring.add(points);
                        }
                    }
                    // Course start/finish/gate detectors (FUN_00402c60 slalom,
                    // FUN_00403180 freestyle, FUN_00403250 tree slalom) run at
                    // the end of the player's update, with its saved
                    // pre-update position.
                    if Some(idx) == self.player_idx {
                        if let Some((ox, oy)) = player_old {
                            self.check_course_crossings(ox, oy, delta_ms);
                        }
                    }
                }
                ActorClass::Dog => {
                    if let Some(actor) = self.pool.get_mut(idx) {
                        mob::update_dog(actor, &mut self.rng, self.fast_forward);
                    }
                }
                ActorClass::Snowboarder => {
                    if let Some(actor) = self.pool.get_mut(idx) {
                        if let Some(req) =
                            mob::update_snowboarder(actor, &mut self.rng, self.fast_forward, &mut self.sounds)
                        {
                            spawn_requests.push(req);
                        }
                    }
                }
                ActorClass::CrossingSkier => {
                    if let Some(actor) = self.pool.get_mut(idx) {
                        mob::update_crossing_skier(actor, &mut self.rng, self.fast_forward);
                    }
                }
                // Proxies run in their own pass below (FUN_004040a0), in
                // mob-node order, after the actor loop.
                #[expect(
                    clippy::match_same_arms,
                    reason = "proxies update in their own pass below; classes >= 0xb never do"
                )]
                ActorClass::LiftChair
                | ActorClass::MonsterUphill
                | ActorClass::MonsterDownhill
                | ActorClass::MonsterLeftEdge
                | ActorClass::MonsterRightEdge => {}
                ActorClass::BurningTree => {
                    if let Some(actor) = self.pool.get_mut(idx) {
                        mob::update_burning_tree(actor, self.fast_forward);
                    }
                }
                ActorClass::WalkingTree => {
                    if let Some(actor) = self.pool.get_mut(idx) {
                        mob::update_walking_tree(actor, &mut self.rng, self.fast_forward);
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
        if self.log_updates || collision::log_pairs() {
            eprintln!("collisions tick={}", self.tick_count);
        }
        let results = collision::check_all_collisions(
            &mut self.pool,
            self.player_idx,
            &self.camera,
            self.screen_w,
            self.screen_h,
            &self.sprites,
            &|idx, a| start_pos.get(idx).copied().flatten() != Some(moved_snapshot(a)),
            &|a| !a.class.is_mob_node() || a.flags.on_screen,
        );

        for result in results {
            if let Some(sound) = result.sound {
                self.sounds.push(sound);
            }
            if result.score_delta != 0 {
                self.scoring.add(result.score_delta);
            }
            if result.skier_eaten {
                if let (Some(player_idx), Some(monster_idx)) = (self.player_idx, result.eaten_by) {
                    if let Some(monster) = self.pool.get_mut(monster_idx) {
                        monster::eat_skier(monster, self.wall_ms);
                    }
                    self.leak_area(player_idx);
                    self.pool.free(player_idx);
                    self.player_idx = None;
                }
            }
            if let Some(idx) = result.free_actor {
                self.leak_area(idx);
                self.pool.free(idx);
            }
        }

        // The camera moved with the player during its update (track); each
        // 0x3c of that travel spawns one actor at the entering edge
        // (FUN_004025c0), in the original's order: bottom, top, right, left.
        // Drained every tick, even the one the skier was eaten in.
        let steps = self.camera.drain_steps();
        for (edge, count) in [
            (populator::Edge::Bottom, steps.down),
            (populator::Edge::Top, steps.up),
            (populator::Edge::Right, steps.right),
            (populator::Edge::Left, steps.left),
        ] {
            for _ in 0..count {
                // DAT_0040c6fc: live actors (off-screen proxies are bare mob
                // nodes in the original and don't count), plus the areas the
                // original never subtracts (`removed_area`).
                let weight = self.removed_area
                    + populator::sprite_weight(
                        self.pool
                            .active_indices()
                            .into_iter()
                            .filter(|&i| !self.is_proxy(i) || self.pool.get(i).is_some_and(|a| a.flags.on_screen))
                            .filter_map(|i| self.pool.get(i)),
                        &self.sprites,
                    );
                if let Some(req) =
                    populator::pick_spawn(edge, &self.camera, self.screen_w, self.screen_h, weight, &mut self.rng)
                {
                    self.fulfill_spawn(req);
                }
            }
        }

        // Random crossing-skier spawn, 1-in-666 per tick (FUN_00401e50 tail):
        // class 3 state 0x1f entering above the viewport's top edge at a
        // random x across the window (FUN_004024f0 direction 2, the populator's
        // edge position), so it overtakes the player from behind. The view
        // anchor DAT_0040c704/DAT_0040c5fc is (w/2, h/3) from FUN_00406060,
        // as in the port's camera; the client-rect origin
        // DAT_0040c6b0/DAT_0040c6b4 it adds is always 0.
        if self.rng.rand_range(0x29a) == 0 {
            let (x, y) = populator::edge_position(
                populator::Edge::Top,
                &self.camera,
                self.screen_w,
                self.screen_h,
                &mut self.rng,
            );
            self.fulfill_spawn(SpawnRequest {
                class: ActorClass::CrossingSkier,
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
        self.tick_count += 1;

        // FUN_00401000: after the update and the repaint, the status display
        // is repainted only if more than 327 ms have passed since its last
        // paint (every 7th tick at the real 46.875 ms, every 9th at 40 ms).
        if self.status_age_ms > 0x147 {
            self.paint_status();
        }
    }

    /// Cull test for one actor right after its update (`FUN_00401e50` first
    /// loop: `FUN_00401290` against the padded viewport `DAT_0040c680`). A
    /// course/secondary actor outside it is freed (its spawn-event index is
    /// queued for an end-of-tick link reset); a proxy keeps its node but its
    /// live actor is dropped (see `proxy_unlink`).
    fn cull_check(&mut self, idx: usize, unlinked_events: &mut Vec<usize>) {
        let Some(a) = self.pool.get(idx) else { return };
        let proxy = self.is_proxy(idx);
        if proxy && !a.flags.on_screen {
            return; // bare mob node, no actor to cull
        }
        let (w, h) = self.sprites.get(a.sprite_id);
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

    /// Course start/finish/gate detection (`FUN_00402c60` slalom,
    /// `FUN_00403250` tree slalom, `FUN_00403180` freestyle). Like the
    /// original, it interpolates the crossing point and time between the
    /// pre- and post-tick positions (`course::interpolate`, `FUN_00402e30`).
    fn check_course_crossings(&mut self, old_x: i16, old_y: i16, delta_ms: u32) {
        const START_Y: i16 = 0x280;
        let Some(pi) = self.player_idx else { return };
        let Some(player) = self.pool.get(pi) else { return };
        let (px, py) = (player.x, player.y);
        // DAT_0040c698 (this tick) and DAT_0040c708 (previous tick).
        let now = i64::from(self.elapsed_ms);
        let prev = now - i64::from(delta_ms);
        // Crossing point of a line at `line_y`, interpolated between the
        // pre- and post-update positions (FUN_00402e30).
        let cross = |a: i64, b: i64, line_y: i16| course::interpolate(a, b, py, old_y, line_y);

        for c in Course::ALL {
            let ci = c.index();
            let ((xmin, xmax), finish, has_clock) = (c.strip(), c.finish_y(), c.is_timed());
            if !self.courses[ci].active {
                // Start: crossing y = 0x280 downward, with the crossing x
                // (interpolated) inside the strip.
                if old_y <= START_Y && py > START_Y {
                    let cx = cross(i64::from(px), i64::from(old_x), START_Y) as i16;
                    if cx >= xmin && cx <= xmax {
                        self.courses[ci].active = true;
                        if has_clock {
                            let start = cross(now, prev, START_Y);
                            self.courses[ci].start_ms = start;
                            self.course_clock_ms = start - now;
                            self.courses[ci].next_gate = self.first_gate_index(xmin, xmax);
                        }
                    }
                }
                continue;
            }

            if has_clock {
                self.course_clock_ms = now - self.courses[ci].start_ms;
            }
            if py > finish {
                self.courses[ci].active = false;
                if has_clock {
                    self.course_clock_ms = cross(now, prev, finish) - self.courses[ci].start_ms;
                }
                let clock = self.course_clock_ms.max(0) as u64;
                // High-score entry (FUN_00402ec0): time for slalom/giant
                // slalom, style for freestyle.
                let rank = self.high_scores.insert(c, clock, self.scoring.score);
                let finish = FinishRecord { course: c, ms: clock, style: self.scoring.score, rank };
                self.last_finish = Some(finish);
                self.unsaved_finishes.push(finish);
                self.pending_score_save = true;
                self.show_high_scores = true;
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
                // ... and then repaints the status display at once.
                self.paint_status();
            } else if py <= START_Y {
                // Skied back above the start line: the run is abandoned.
                self.courses[ci].active = false;
            } else if has_clock {
                self.check_gate_pass(c, px, old_x, py, old_y);
            }
        }

        // Style points accumulate only inside the freestyle course
        // (FUN_00403420 gates on DAT_0040c954, the freestyle flag).
        self.scoring.enabled = self.courses[Course::Freestyle.index()].active;
    }

    /// First gate record index within a course strip (`DAT_0040c94c/c950`).
    fn first_gate_index(&self, xmin: i16, xmax: i16) -> Option<usize> {
        self.spawn_events.iter().position(|e| e.class == ActorClass::Gate as u16 && e.x >= xmin && e.x <= xmax)
    }

    /// Gate passing (`FUN_00402c60` / `FUN_00403250` tail + `FUN_00403130`): once
    /// the player's y passes the next gate's y, the crossing x (interpolated
    /// at the gate's y) decides: sprite 0x17 is missed when that x is right
    /// of the gate's (x > gate x), sprite 0x18 when it is left of it
    /// (x < gate x). The flag turns sprite 0x19 (passed) or 0x1a (missed, and
    /// the course start moves back 5 s). One gate per tick.
    fn check_gate_pass(&mut self, c: Course, px: i16, old_x: i16, py: i16, old_y: i16) {
        let ci = c.index();
        let Some(gi) = self.courses[ci].next_gate else { return };
        let Some(gate) = self.spawn_events.get(gi) else { return };
        if py <= gate.y {
            return;
        }
        let (gx, gsprite, actor_idx) = (gate.x, gate.sprite, gate.actor_idx);
        let cx = course::interpolate(i64::from(px), i64::from(old_x), py, old_y, gate.y) as i16;
        let missed = match gsprite {
            Some(0x17) => cx > gx,
            Some(0x18) => cx < gx,
            _ => false,
        };
        let new_sprite = if missed { 0x1a } else { 0x19 };
        if missed {
            self.courses[ci].start_ms -= 5000;
        }
        if let Some(ev) = self.spawn_events.get_mut(gi) {
            ev.sprite = Some(new_sprite);
        }
        if let Some(ai) = actor_idx {
            if let Some(a) = self.pool.get_mut(ai) {
                a.sprite_id = new_sprite;
            }
        }
        let (xmin, xmax) = c.strip();
        self.courses[ci].next_gate = self
            .spawn_events
            .iter()
            .enumerate()
            .skip(gi + 1)
            .find(|(_, e)| e.class == ActorClass::Gate as u16 && e.x >= xmin && e.x <= xmax)
            .map(|(i, _)| i);
    }

    /// HUD Time value in ms (`DAT_0040c944`), signed like the original.
    pub fn hud_time_ms(&self) -> i64 {
        self.course_clock_ms
    }

    /// HUD Dist value in meters (`FUN_00401b80`): remaining inside a course,
    /// distance traveled when free-skiing, 0 without a player.
    pub fn hud_dist_m(&self) -> i32 {
        let Some(pi) = self.player_idx else { return 0 };
        let Some(p) = self.pool.get(pi) else { return 0 };
        // 16-bit arithmetic like the original (`movsx eax, di`).
        let units = match Course::ALL.into_iter().find(|c| self.courses[c.index()].active) {
            Some(c) => c.finish_y().wrapping_sub(p.y),
            None => p.y,
        };
        i32::from(units) / 16
    }

    /// HUD Speed value in m/s (`FUN_00401b80`): downhill velocity scaled by
    /// the last frame delta (units are 1/16 m per tick).
    pub fn hud_speed(&self) -> i32 {
        let Some(pi) = self.player_idx else { return 0 };
        let Some(p) = self.pool.get(pi) else { return 0 };
        if self.last_tick_ms == 0 {
            return 0;
        }
        (i32::from(p.vy) * 1000) / ((self.last_tick_ms as i32) << 4)
    }

    /// Spawn a secondary actor from a behavior request (`FUN_004020d0` via
    /// `state`, or `FUN_004026a0` via `sprite`), placing it with `FUN_00402390`.
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

    /// Proxy classes (4-8): chairlift chairs and monsters, which the original
    /// runs as mob nodes with an actor only while on screen.
    fn is_proxy(&self, idx: usize) -> bool {
        self.pool.get(idx).is_some_and(|a| a.class.is_mob_node())
    }

    /// Window activation changed (`WM_ACTIVATE` / `WM_SIZE` -> `FUN_00405a10`):
    /// input is processed only while active; losing focus or minimizing
    /// stops the game timer, regaining it restarts it — unless F3-paused.
    pub fn set_active(&mut self, active: bool) {
        self.input_enabled = active;
        if active {
            self.resume_timer();
        } else {
            self.stop_timer();
        }
    }

    /// T cheat (`WM_CHAR` -> `FUN_00406780` case 'T' -> `FUN_00401000`): run one
    /// tick directly, whether or not the timer is running (F3 / focus pause).
    pub fn step_tick(&mut self, delta_ms: u32) {
        let running = self.timer_running;
        self.timer_running = true;
        self.tick(delta_ms);
        self.timer_running = running;
    }

    /// F3 (`FUN_00405760`): pause if the timer runs, otherwise unpause and
    /// restart it.
    pub fn toggle_pause(&mut self) {
        self.paused = self.timer_running;
        if self.paused {
            self.stop_timer();
        } else {
            self.resume_timer();
        }
    }

    /// `FUN_004057c0`: kill the game timer, noting the game time it stopped
    /// at (`DAT_0040c600`).
    fn stop_timer(&mut self) {
        if self.timer_running {
            self.pause_ref_ms = self.elapsed_ms;
        }
        self.timer_running = false;
    }

    /// `FUN_00404ad0`: restart the game timer, unless it runs or F3 paused it.
    /// `elapsed_ms` stands still while stopped, so the course clock skips the
    /// pause by itself; the original also shifts the course start
    /// (`DAT_0040c948`) by the time the T steps added while stopped, so the
    /// clock drops back to where it stood at the pause.
    fn resume_timer(&mut self) {
        if !self.timer_running && !self.paused {
            self.timer_running = true;
            let stepped = i64::from(self.elapsed_ms.wrapping_sub(self.pause_ref_ms));
            for c in Course::ALL {
                if c.is_timed() && self.courses[c.index()].active {
                    self.courses[c.index()].start_ms += stepped;
                }
            }
            self.pause_ref_ms = self.elapsed_ms;
        }
    }

    /// Mob-node pass (`FUN_004040a0`). Each node is updated (`FUN_004041c0`:
    /// class 4 via `FUN_00404290`, monsters via `FUN_00404350`) and then, if its
    /// y is inside the scan window, offered to `FUN_00404130`, which gives it
    /// a live actor when the node's point (x, y - z) lies strictly inside the
    /// padded viewport (nodes carry the empty sprite-table entry 0, 0x0 —
    /// verified via Frida). The live actor persists until the cull pass
    /// drops it by its real sprite box. Nodes are visited in world-build
    /// order: chairlift chairs, then monsters.
    fn update_proxies(&mut self) {
        let nodes: Vec<usize> =
            self.spawn_events.iter().filter(|e| (4..=8).contains(&e.class)).filter_map(|e| e.actor_idx).collect();
        for idx in nodes {
            let player = self.player_idx.and_then(|pi| self.pool.get(pi).cloned());
            let mut request = None;
            if let Some(actor) = self.pool.get_mut(idx) {
                match actor.class {
                    // The original rolls only while the node has a live actor
                    // ([node+0] != 0 in FUN_00404290).
                    ActorClass::LiftChair => {
                        let live = actor.flags.on_screen;
                        request = mob::update_lift_chair(actor, &mut self.rng, live);
                    }
                    ActorClass::MonsterUphill
                    | ActorClass::MonsterDownhill
                    | ActorClass::MonsterLeftEdge
                    | ActorClass::MonsterRightEdge => {
                        monster::update_monster(
                            actor,
                            player.as_ref(),
                            &mut self.rng,
                            self.wall_ms,
                            self.screen_w,
                            self.screen_h,
                            &mut self.sounds,
                        );
                    }
                    _ => continue,
                }
            }
            if let Some(req) = request {
                self.fulfill_spawn(req);
            }
            let (camera, sw, sh) = (self.camera.clone(), self.screen_w, self.screen_h);
            // Coming on screen takes a live actor: none when the original's
            // 100 are in use (its allocation fails and the node stays bare).
            let room = self.pool.has_live_room();
            if let Some(a) = self.pool.get_mut(idx) {
                if a.proxy_unlink {
                    // Culled last cull pass: the link drops only now, and the
                    // node cannot re-materialize in the same pass.
                    a.proxy_unlink = false;
                    a.flags.on_screen = false;
                } else if !a.flags.on_screen
                    && room
                    && collision::in_event_window(a.y, &camera, sw, sh)
                    && collision::in_padded_view(a.x, a.y, a.z, 0, 0, &camera, sw, sh)
                {
                    a.flags.on_screen = true;
                }
            }
        }
    }

    /// Consume spawn events that the player has scrolled past (`FUN_004046e0`).
    fn consume_spawn_events(&mut self) {
        for (event_pos, event) in self.spawn_events.iter_mut().enumerate() {
            if event.actor_idx.is_some() {
                continue; // already spawned
            }

            // Proxy classes (4-8: chairlift chairs, monsters) are allocated from
            // world build: in the original their mob nodes update (drift,
            // wrap, roll RNG) globally wherever the player is; whether they
            // count as "on screen" is decided per tick by in_padded_view.
            // Course events materialize like FUN_004046e0 + FUN_00404130:
            // event y inside the scan window AND the sprite's draw box
            // overlapping the padded viewport. Culled actors (event link
            // reset) re-materialize here when scrolled back into range.
            let in_window = (4..=8).contains(&event.class) || {
                let sprite =
                    event.sprite.unwrap_or_else(|| STATE_TO_SPRITE.get(event.state as usize).copied().unwrap_or(0));
                let (w, h) = self.sprites.get(sprite);
                collision::in_event_window(event.y, &self.camera, self.screen_w, self.screen_h)
                    // Event nodes carry z = 0 (verified via Frida); the
                    // actor's own z only applies once materialized.
                    && collision::in_padded_view(event.x, event.y, 0, w, h, &self.camera, self.screen_w, self.screen_h)
            };
            if in_window {
                let Some(class) = ActorClass::from_u16(event.class) else { continue };

                let idx = match event.sprite {
                    Some(sprite) => self.pool.alloc_with_sprite(class, sprite),
                    None => self.pool.alloc(class, event.state),
                };
                if let Some(idx) = idx {
                    if let Some(actor) = self.pool.get_mut(idx) {
                        actor.x = event.x;
                        actor.y = event.y;
                        actor.z = event.z;
                        if class == ActorClass::LiftChair {
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

    /// `FUN_00402ec0` re-reads the stored high scores at every finish
    /// (0x402f09) and ranks the finish in that table, not in one kept from
    /// startup, so another instance's scores survive. `stored` is that
    /// table; the platform layer reads it and saves the result. Every finish
    /// since the last save goes in, in order (two can land in one frame),
    /// and the last one's rank is the one the box shows.
    pub fn rank_finish_in(&mut self, stored: crate::game::scoring::HighScores) {
        if self.unsaved_finishes.is_empty() {
            return;
        }
        self.high_scores = stored;
        let mut rank = None;
        for f in std::mem::take(&mut self.unsaved_finishes) {
            rank = self.high_scores.insert(f.course, f.ms, f.style);
        }
        if let Some(last) = self.last_finish.as_mut() {
            last.rank = rank;
        }
    }

    /// Keep actor `idx`'s area in the density sum: the original removes it
    /// from the world but not from `DAT_0040c6fc`.
    fn leak_area(&mut self, idx: usize) {
        if let Some(a) = self.pool.get(idx) {
            self.removed_area += populator::sprite_weight(std::iter::once(a), &self.sprites);
        }
    }

    /// The player's actor, while the skier is present.
    fn player_mut(&mut self) -> Option<&mut Actor> {
        self.player_idx.and_then(|idx| self.pool.get_mut(idx))
    }

    /// Handle keyboard steering
    #[expect(clippy::fn_params_excessive_bools, reason = "the four arrow keys, held or not")]
    pub fn steer_keyboard(&mut self, left: bool, right: bool, up: bool, down: bool) {
        if let Some(actor) = self.player_mut() {
            skier::steer_keyboard(actor, left, right, up, down);
        }
    }

    /// Mouse moved to client position (x, y) (`WM_MOUSEMOVE` -> `FUN_00406550`).
    /// Like the original, the very first mouse message only records the
    /// position, and a message at the unchanged position is ignored; the
    /// offset is taken from the skier's screen anchor (w/2, h/3).
    pub fn mouse_move(&mut self, x: i16, y: i16) {
        let last = self.mouse_last.replace((x, y));
        if last.is_none() || last == Some((x, y)) {
            return;
        }
        let (dx, dy) = (x - self.screen_w / 2, y - self.screen_h / 3);
        if let Some(actor) = self.player_mut() {
            skier::steer_mouse(actor, dx, dy);
        }
    }

    /// Keyboard jump (Insert / numpad0): smaller hop that scrubs speed.
    pub fn jump_keyboard(&mut self) {
        if let Some(actor) = self.player_mut() {
            skier::jump_keyboard(actor);
        }
    }

    /// Diagonal steering: set a skiing state directly (1/4 slight, 3/6 hard).
    pub fn steer_absolute(&mut self, target_state: u16) {
        if let Some(actor) = self.player_mut() {
            skier::steer_absolute(actor, target_state);
        }
    }

    /// Whether the skier is still present (false once eaten).
    pub fn has_player(&self) -> bool {
        self.player_idx.is_some()
    }

    /// Handle click (`FUN_004066d0`): with no skier (eaten) it starts a new
    /// game like Enter (it jumps to `FUN_00406500`); otherwise it jumps on
    /// the ground and cycles tricks mid-air.
    pub fn click(&mut self) {
        match self.player_mut() {
            Some(actor) => skier::click(actor),
            None => self.new_game(),
        }
    }

    /// A cheat key (`FUN_00406780`). `step_ms` gives the time a `t` step
    /// runs for, read only for a step. Returns whether it stepped.
    pub fn cheat(&mut self, cheat: super::cheat::Cheat, step_ms: impl FnOnce() -> u32) -> bool {
        use super::cheat::Cheat;
        match cheat {
            Cheat::Teleport(dx, dy) => self.teleport(dx, dy),
            Cheat::FastForward => self.fast_forward = !self.fast_forward,
            // The port repaints every frame.
            Cheat::Repaint => {}
            Cheat::Step => {
                self.step_tick(step_ms());
                return true;
            }
        }
        false
    }

    /// The x/X/y/Y teleport cheat: move the skier by (dx, dy), wrapping like
    /// the original's 16-bit position.
    pub fn teleport(&mut self, dx: i16, dy: i16) {
        if let Some(actor) = self.player_mut() {
            actor.x = actor.x.wrapping_add(dx);
            actor.y = actor.y.wrapping_add(dy);
            let (x, y) = (actor.x, actor.y);
            // FUN_00402390 -> FUN_00402470: the camera jumps with the skier
            // between ticks, outside FUN_00401e50's scroll accounting, so a
            // teleport spawns nothing.
            self.camera.cx = x;
            self.camera.cy = y;
        }
    }
}

static CLOCK_SOURCE: std::sync::OnceLock<fn() -> u32> = std::sync::OnceLock::new();

/// The clock `clock_seed` reads, installed once by the platform layer
/// (wasm32-unknown-unknown has no `SystemTime`). Without one it reads
/// `SystemTime`, as the replay tools and tests do.
pub fn set_clock_source(clock_ms: fn() -> u32) {
    let _ = CLOCK_SOURCE.set(clock_ms);
}

/// Clock-derived seed for unseeded games (the original seeds with
/// GetTickCount()): milliseconds since the Unix epoch, truncated to 32 bits.
fn clock_seed() -> u32 {
    if let Some(clock_ms) = CLOCK_SOURCE.get() {
        return clock_ms();
    }
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game_with_player() -> GameState {
        let mut g = GameState::new_seeded(640, 480, 1, SpriteSizes::placeholder());
        g.new_game();
        g
    }

    #[test]
    fn course_events_materialize_only_inside_padded_view() {
        // 704 wide: padded view spans screen x -120..824, i.e. world x
        // -472..472 around the player. The slalom start marker at x = -576
        // stays a bare event; the freestyle ones at +-160 materialize once
        // the skier has descended far enough for y = 640 to enter the window.
        let mut g = GameState::new_seeded(704, 681, 0, SpriteSizes::placeholder());
        g.new_game();
        g.steer_keyboard(false, false, false, true);
        for _ in 0..15 {
            g.tick(40);
        }
        let xs: Vec<i16> = g
            .pool
            .active_indices()
            .into_iter()
            .filter_map(|i| g.pool.get(i))
            .filter(|a| a.sprite_id == 0x39 || a.sprite_id == 0x3a)
            .map(|a| a.x)
            .collect();
        assert!(xs.contains(&-160) && xs.contains(&160), "{xs:?}");
        assert!(!xs.contains(&-576), "{xs:?}");
    }

    #[test]
    fn offscreen_proxies_stay_unmaterialized() {
        let mut g = GameState::new_seeded(704, 681, 0, SpriteSizes::placeholder());
        g.new_game();
        for _ in 0..3 {
            g.tick(40);
        }
        let monsters: Vec<bool> = g
            .pool
            .active_indices()
            .into_iter()
            .filter_map(|i| g.pool.get(i))
            .filter(|a| a.class.is_monster())
            .map(|a| a.flags.on_screen)
            .collect();
        assert_eq!(monsters.len(), 4, "monster nodes exist from world build");
        assert!(monsters.iter().all(|&live| !live), "far-away monsters have no live actor");
    }

    #[test]
    fn losing_focus_stops_the_game_and_input() {
        let mut g = game_with_player();
        g.steer_keyboard(false, false, false, true);
        g.tick(40);
        let y = g.pool.get(g.player_idx.unwrap()).unwrap().y;
        g.set_active(false);
        assert!(!g.input_enabled && !g.timer_running);
        for _ in 0..5 {
            g.tick(40);
        }
        assert_eq!(g.pool.get(g.player_idx.unwrap()).unwrap().y, y, "frozen while inactive");
        g.set_active(true);
        assert!(g.input_enabled && g.timer_running);
    }

    #[test]
    fn regaining_focus_does_not_undo_f3() {
        let mut g = game_with_player();
        g.toggle_pause();
        assert!(g.paused && !g.timer_running);
        g.set_active(false);
        g.set_active(true);
        assert!(g.paused && !g.timer_running, "F3 pause survives a focus change");
        g.toggle_pause();
        assert!(!g.paused && g.timer_running);
    }

    #[test]
    fn meal_wall_clock_runs_while_paused() {
        // The original keeps DAT_0040c698 on wall time; only the course clock
        // is compensated for pauses. Pausing must not freeze the meal timer.
        let mut g = game_with_player();
        g.tick(40);
        g.toggle_pause();
        let (w0, e0) = (g.wall_ms, g.elapsed_ms);
        for _ in 0..10 {
            g.tick(40);
        }
        assert_eq!(g.wall_ms, w0 + 400);
        assert_eq!(g.elapsed_ms, e0, "game clock (course timing) stays frozen");
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
    fn a_finish_is_ranked_in_the_stored_table() {
        // FUN_00402ec0 ranks against the table it re-reads, so another
        // instance's better time pushes this one down.
        let mut g = game_with_player();
        let first = FinishRecord { course: Course::Freestyle, ms: 0, style: 120, rank: Some(0) };
        let last = FinishRecord { course: Course::TreeSlalom, ms: 5000, style: 120, rank: Some(0) };
        g.unsaved_finishes = vec![first, last];
        g.last_finish = Some(last);
        let mut stored = crate::game::scoring::HighScores::default();
        stored.insert_time(Course::TreeSlalom, 4000);
        g.rank_finish_in(stored);
        assert_eq!(g.last_finish.map(|f| f.rank), Some(Some(1)));
        assert_eq!(g.high_scores.giant_slalom_ms, vec![4000, 5000]);
        // Two finishes before a save (cheats can do it): both are kept.
        assert_eq!(g.high_scores.freestyle_style, vec![120]);
        assert_eq!(g.unsaved_finishes, []);
    }

    #[test]
    fn click_without_a_skier_starts_a_new_game() {
        // FUN_004066d0 jumps to FUN_00406500 when the skier is gone.
        let mut g = game_with_player();
        for _ in 0..5 {
            g.tick(40);
        }
        let idx = g.player_idx.take().unwrap();
        g.pool.free(idx);
        g.click();
        assert!(g.has_player());
        assert_eq!(g.tick_count, 0);
    }

    #[test]
    fn teleport_moves_the_camera_without_scrolling() {
        // FUN_00402390 -> FUN_00402470: the camera jumps with the skier,
        // outside the scroll accounting, so the jump spawns nothing.
        let mut g = game_with_player();
        let scroll = (g.camera.scroll_x, g.camera.scroll_y);
        for _ in 0..31 {
            g.teleport(2, 2);
        }
        let p = g.pool.get(g.player_idx.unwrap()).unwrap();
        assert_eq!((g.camera.cx, g.camera.cy), (p.x, p.y));
        assert_eq!((g.camera.scroll_x, g.camera.scroll_y), scroll);
    }

    #[test]
    fn removed_actors_stay_in_the_density_sum_until_a_new_game() {
        let mut g = game_with_player();
        g.sprites = SpriteSizes::new(vec![(8, 11); 100]);
        let rock = g.pool.alloc_with_sprite(ActorClass::Rock, 0x56).unwrap();
        g.leak_area(rock);
        g.pool.free(rock);
        assert_eq!(g.removed_area, 88);
        g.new_game();
        assert_eq!(g.removed_area, 0, "0x404998 resets the sum");
    }

    #[test]
    fn t_steps_during_a_pause_come_off_the_course_clock() {
        // FUN_00404ad0 shifts the course start by the time since the pause
        // (DAT_0040c600), T steps included.
        let mut g = game_with_player();
        g.tick(40);
        let p = g.player_idx.unwrap();
        g.pool.get_mut(p).unwrap().y = 0x300; // past the start line
        g.courses[0].active = true;
        g.courses[0].start_ms = i64::from(g.elapsed_ms);
        g.tick(40);
        assert_eq!(g.course_clock_ms, 40);
        g.toggle_pause();
        g.step_tick(5000);
        assert_eq!(g.course_clock_ms, 5040, "the step shows while paused");
        g.toggle_pause();
        g.tick(47);
        assert_eq!(g.course_clock_ms, 87, "the restart takes the step back off");
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
        let mut g = GameState::new_seeded(640, 480, 1, SpriteSizes::placeholder());
        g.fulfill_spawn(SpawnRequest {
            class: ActorClass::CrossingSkier,
            state: Some(0x21),
            sprite: None,
            x: -112,
            y: 498,
            z: 0x20,
        });
        let a = g.pool.get(g.pool.head.unwrap()).unwrap();
        assert_eq!(a.class, ActorClass::CrossingSkier);
        assert_eq!(a.state, 0x21);
        assert_eq!(a.sprite_id, crate::game::state_table::STATE_TO_SPRITE[0x21]);
        assert_eq!((a.x, a.y, a.z), (-112, 498, 0x20));
    }

    #[test]
    fn fulfill_spawn_by_sprite_sets_raw_sprite() {
        let mut g = GameState::new_seeded(640, 480, 1, SpriteSizes::placeholder());
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
        let mut g = GameState::new_seeded(640, 480, seed, SpriteSizes::placeholder());
        let idx = g.pool.alloc(ActorClass::Snowboarder, 0x1e).unwrap();
        {
            let a = g.pool.get_mut(idx).unwrap();
            a.x = 200;
            a.y = 300;
            a.z = 0;
        }
        g.tick(40);
        let effect = g
            .pool
            .active_indices()
            .into_iter()
            .filter_map(|i| g.pool.get(i))
            .find(|a| a.class == ActorClass::Effect)
            .expect("jump effect actor spawned");
        assert_eq!(effect.sprite_id, 0x52);
        assert_eq!((effect.x, effect.y), (196, 298));
        assert_eq!(g.pool.get(idx).unwrap().state, 0x1b);
    }

    #[test]
    fn lift_chair_spawn_event_applies_initial_vy() {
        let mut g = GameState::new_seeded(640, 480, 1, SpriteSizes::placeholder());
        g.spawn_events.push(SpawnEvent {
            actor_idx: None,
            state: 0x27,
            class: 4,
            sprite: None,
            x: -112,
            y: 100,
            z: 0x20,
            param: -2,
        });
        g.tick(40);
        let mob = g
            .pool
            .active_indices()
            .into_iter()
            .filter_map(|i| g.pool.get(i))
            .find(|a| a.class == ActorClass::LiftChair)
            .expect("chairlift chair materialized");
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
        let mut g = GameState::new_seeded(640, 480, 1, SpriteSizes::placeholder());
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
        // The start line was crossed right at the previous tick's position
        // (y_old = 0x280), so the interpolated start time is that tick's
        // clock (FUN_00402e30): two ticks later the clock reads 80 ms.
        assert_eq!(g.hud_time_ms(), 80, "clock runs from the interpolated start");
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

        // Sprite 0x17 is passed at or left of the gate's x (FUN_00403250 at
        // 0x403350: missed when the crossing x > gate x).
        let gx = g.spawn_events[gi].x;
        place_player(&mut g, gx - 20, gy - 1, 4);
        g.tick(40);
        assert_eq!(g.spawn_events[gi].sprite, Some(0x19), "passed gate turns sprite 0x19");

        // Sprite 0x18 is missed left of the gate's x.
        let gi2 = g.courses[0].next_gate.unwrap();
        let (gx2, gy2) = (g.spawn_events[gi2].x, g.spawn_events[gi2].y);
        assert_eq!(g.spawn_events[gi2].sprite, Some(0x18));
        let start_before = g.courses[0].start_ms;
        place_player(&mut g, gx2 - 20, gy2 - 1, 4);
        g.tick(40);
        assert_eq!(g.spawn_events[gi2].sprite, Some(0x1a), "missed gate turns sprite 0x1a");
        assert_eq!(g.courses[0].start_ms, start_before - 5000, "miss: start moves back 5 s");
        // The clock is updated before the gate check, so the penalty shows
        // from the next tick on.
        let clock = g.hud_time_ms();
        g.tick(40);
        assert_eq!(g.hud_time_ms(), clock + 40 + 5000);
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
        assert!(g.pending_score_save);
        // High-score entry (FUN_00402ec0): slalom time recorded at rank 0,
        // and the skier stands (FUN_00402e80).
        let f = g.last_finish.expect("finish recorded");
        assert_eq!(f.course, Course::Slalom);
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
        assert_eq!(f.course, Course::Freestyle);
        assert_eq!(f.style, -22);
        assert_eq!(g.high_scores.freestyle_style, vec![-22]);
        assert_eq!(g.high_scores.slalom_ms, Vec::<u64>::new());
        assert_eq!(g.high_scores.giant_slalom_ms, Vec::<u64>::new());
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
        let mut g = GameState::new_seeded(640, 480, 1, SpriteSizes::placeholder());
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
