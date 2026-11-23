use serde::{Deserialize, Serialize};

use crate::game::state::GameState;
use crate::game::types::CourseType;

#[derive(Debug, Clone, Deserialize)]
pub struct Scenario {
    pub seed: u32,
    #[serde(default = "default_course")]
    pub course: CourseKind,
    pub ticks: u32,
    #[serde(default = "default_delta_ms")]
    pub delta_ms: u32,
    #[serde(default)]
    pub screen_w: Option<i16>,
    #[serde(default)]
    pub screen_h: Option<i16>,
    #[serde(default)]
    pub inputs: Vec<InputEvent>,
}

fn default_course() -> CourseKind {
    CourseKind::Freestyle
}

fn default_delta_ms() -> u32 {
    40
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CourseKind {
    Freestyle,
    Slalom,
    TreeSlalom,
}

impl From<CourseKind> for CourseType {
    fn from(k: CourseKind) -> Self {
        match k {
            CourseKind::Freestyle => CourseType::Freestyle,
            CourseKind::Slalom => CourseType::Slalom,
            CourseKind::TreeSlalom => CourseType::TreeSlalom,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct InputEvent {
    pub tick: u32,
    #[serde(flatten)]
    pub kind: InputKind,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputKind {
    Steer {
        #[serde(default)]
        left: bool,
        #[serde(default)]
        right: bool,
        #[serde(default)]
        up: bool,
        #[serde(default)]
        down: bool,
    },
    SteerAbsolute {
        state: u16,
    },
    Jump,
    Click,
    /// Mouse moved to client position (x, y), like WM_MOUSEMOVE.
    Mouse {
        x: i16,
        y: i16,
    },
    FastForward,
    /// F2: start a new game (re-seeds and rebuilds the world).
    Restart,
    /// F3: toggle pause. Paused ticks still advance the wall clock.
    Pause,
    /// Window activation change (focus gained/lost, minimize/restore).
    Focus {
        active: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlayerSnapshot {
    pub x: i16,
    pub y: i16,
    pub z: i16,
    pub vx: i16,
    pub vy: i16,
    pub vz: i16,
    pub state: u16,
    pub sprite_id: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TickRecord {
    pub tick: u32,
    pub elapsed_ms: u32,
    pub player: Option<PlayerSnapshot>,
    pub actor_count: usize,
    pub score: i32,
    /// Course clock (DAT_0040c944, ms).
    #[serde(default)]
    pub course_ms: i64,
    pub rng_calls: u64,
    /// FNV-1a hash of every record up to and including this one.
    pub hash: u64,
}

impl TickRecord {
    fn payload_json(&self) -> String {
        #[derive(Serialize)]
        struct Payload<'a> {
            tick: u32,
            elapsed_ms: u32,
            player: &'a Option<PlayerSnapshot>,
            actor_count: usize,
            score: i32,
            course_ms: i64,
            rng_calls: u64,
        }
        serde_json::to_string(&Payload {
            tick: self.tick,
            elapsed_ms: self.elapsed_ms,
            player: &self.player,
            actor_count: self.actor_count,
            score: self.score,
            course_ms: self.course_ms,
            rng_calls: self.rng_calls,
        })
        .expect("tick record payload is always serializable")
    }
}

pub fn fnv1a_update(hash: u64, bytes: &[u8]) -> u64 {
    let mut h = hash;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

pub const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;

/// Feed the scenario's inputs for `tick` to the game, like the original's
/// window procedure would before that main tick.
pub fn apply_inputs(game: &mut GameState, scenario: &Scenario, tick: u32) {
    for input in scenario.inputs.iter().filter(|i| i.tick == tick) {
        // The original's window procedure drops keyboard and mouse
        // messages while the window is inactive (DAT_0040c67c).
        if !game.input_enabled && !matches!(input.kind, InputKind::Focus { .. }) {
            continue;
        }
        match &input.kind {
            InputKind::Steer {
                left,
                right,
                up,
                down,
            } => game.steer_keyboard(*left, *right, *up, *down),
            InputKind::SteerAbsolute { state } => game.steer_absolute(*state),
            InputKind::Jump => game.jump_keyboard(),
            InputKind::Click => game.click(),
            InputKind::Mouse { x, y } => game.mouse_move(*x, *y),
            InputKind::FastForward => game.fast_forward = !game.fast_forward,
            InputKind::Restart => game.new_game(),
            InputKind::Pause => game.toggle_pause(),
            InputKind::Focus { active } => game.set_active(*active),
        }
    }
}

pub fn run_scenario(scenario: &Scenario) -> Vec<TickRecord> {
    run_scenario_with_size(
        scenario,
        scenario.screen_w.unwrap_or(640),
        scenario.screen_h.unwrap_or(480),
    )
}

pub fn run_scenario_with_size(
    scenario: &Scenario,
    screen_w: i16,
    screen_h: i16,
) -> Vec<TickRecord> {
    run_scenario_observed(scenario, screen_w, screen_h, |_, _| {})
}

/// Like [`run_scenario_with_size`], calling `observe(tick, &game)` after each
/// tick (e.g. to dump the frame the original paints at the end of it).
pub fn run_scenario_observed(
    scenario: &Scenario,
    screen_w: i16,
    screen_h: i16,
    mut observe: impl FnMut(u32, &GameState),
) -> Vec<TickRecord> {
    let mut game = GameState::new_seeded(screen_w, screen_h, scenario.seed);
    game.course_type = scenario.course.into();
    game.new_game();

    let mut records = Vec::with_capacity(scenario.ticks as usize);
    let mut hash = FNV_OFFSET_BASIS;

    for tick in 0..scenario.ticks {
        apply_inputs(&mut game, scenario, tick);

        game.tick(scenario.delta_ms);

        let player = game.player_idx.and_then(|idx| {
            game.pool.get(idx).map(|a| PlayerSnapshot {
                x: a.x,
                y: a.y,
                z: a.z,
                vx: a.vx,
                vy: a.vy,
                vz: a.vz,
                state: a.state,
                sprite_id: a.sprite_id,
            })
        });

        let mut record = TickRecord {
            tick,
            elapsed_ms: game.elapsed_ms,
            player,
            actor_count: game.pool.active_count(),
            score: game.scoring.score,
            course_ms: game.course_clock_ms,
            rng_calls: game.rng.call_count(),
            hash: 0,
        };
        hash = fnv1a_update(hash, record.payload_json().as_bytes());
        record.hash = hash;
        records.push(record);
        observe(tick, &game);
    }

    records
}

pub fn records_to_jsonl(records: &[TickRecord]) -> String {
    let mut out = String::new();
    for r in records {
        out.push_str(&serde_json::to_string(r).expect("record is always serializable"));
        out.push('\n');
    }
    out
}

pub fn parse_jsonl(text: &str) -> Result<Vec<TickRecord>, serde_json::Error> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_scenario() -> Scenario {
        Scenario {
            seed: 1,
            course: CourseKind::Freestyle,
            ticks: 100,
            delta_ms: 40,
            screen_w: None,
            screen_h: None,
            inputs: vec![],
        }
    }

    #[test]
    fn run_is_deterministic() {
        let a = run_scenario(&base_scenario());
        let b = run_scenario(&base_scenario());
        assert_eq!(a, b);
    }

    #[test]
    fn jsonl_round_trip() {
        let records = run_scenario(&base_scenario());
        let text = records_to_jsonl(&records);
        let parsed = parse_jsonl(&text).expect("golden text parses");
        assert_eq!(records, parsed);
    }
}
