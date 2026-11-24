use serde::{Deserialize, Serialize};

use crate::game::cheat::Cheat;
use crate::game::skier::ArrowKey;
use crate::game::sprites::SpriteSizes;
use crate::game::state::GameState;
use crate::game::DEFAULT_CLIENT_SIZE;

#[derive(Debug, Clone, Deserialize)]
pub struct Scenario {
    pub seed: u32,
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

impl Scenario {
    /// The client size the scenario runs at (`DEFAULT_CLIENT_SIZE` unless
    /// it sets its own).
    pub fn client_size(&self) -> (i16, i16) {
        (self.screen_w.unwrap_or(DEFAULT_CLIENT_SIZE.0), self.screen_h.unwrap_or(DEFAULT_CLIENT_SIZE.1))
    }
}

fn default_delta_ms() -> u32 {
    40
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
    /// Mouse moved to client position (x, y), like `WM_MOUSEMOVE`.
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
    /// The client area resized to w x h (`WM_SIZE`).
    Resize {
        w: i16,
        h: i16,
    },
    /// A cheat key (`WM_CHAR`, `FUN_00406780`): `x`/`X` move the skier
    /// right/left, `y`/`Y` down/up, `t` runs one tick on the spot (that
    /// tick's record is the stepped one, which takes the timer's place),
    /// `r` repaints. `f` is [`InputKind::FastForward`].
    Cheat {
        key: char,
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
    /// Course clock (`DAT_0040c944`, ms).
    #[serde(default)]
    pub course_ms: i64,
    pub rng_calls: u64,
    /// Status display values (Time, Dist, Speed, Style) when it was
    /// repainted during this tick (`FUN_00401b80`); compared with
    /// `tools/vm/trace_status.js` by `status_align.py`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<[String; 4]>,
    /// Sounds played during this tick (WAVE resource ids, in call order;
    /// `FUN_00402ba0`); compared with `tools/vm/trace_sounds.js` by
    /// `sound_align.py`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sounds: Vec<u8>,
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
            #[serde(skip_serializing_if = "Option::is_none")]
            status: &'a Option<[String; 4]>,
            #[serde(skip_serializing_if = "<[u8]>::is_empty")]
            sounds: &'a [u8],
        }
        serde_json::to_string(&Payload {
            tick: self.tick,
            elapsed_ms: self.elapsed_ms,
            player: &self.player,
            actor_count: self.actor_count,
            score: self.score,
            course_ms: self.course_ms,
            rng_calls: self.rng_calls,
            status: &self.status,
            sounds: &self.sounds,
        })
        .expect("tick record payload is always serializable")
    }
}

pub fn fnv1a_update(hash: u64, bytes: &[u8]) -> u64 {
    let mut h = hash;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

pub const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;

/// Feed the scenario's inputs for `tick` to the game, like the original's
/// window procedure would before that main tick.
///
/// Returns whether a `t` cheat stepped the game: that step is this tick.
pub fn apply_inputs(game: &mut GameState, scenario: &Scenario, tick: u32) -> bool {
    let mut stepped = false;
    for input in scenario.inputs.iter().filter(|i| i.tick == tick) {
        // The original's window procedure drops keyboard and mouse
        // messages while the window is inactive (DAT_0040c67c).
        if !game.input_enabled && !matches!(input.kind, InputKind::Focus { .. } | InputKind::Resize { .. }) {
            continue;
        }
        match &input.kind {
            // One record may press several keys: they act in this order.
            InputKind::Steer { left, right, up, down } => {
                let keys =
                    [(down, ArrowKey::Down), (up, ArrowKey::Up), (left, ArrowKey::Left), (right, ArrowKey::Right)];
                for (_, key) in keys.into_iter().filter(|(pressed, _)| **pressed) {
                    game.steer_keyboard(key);
                }
            }
            InputKind::SteerAbsolute { state } => game.steer_absolute(*state),
            InputKind::Jump => game.jump_keyboard(),
            InputKind::Click => game.click(),
            InputKind::Mouse { x, y } => game.mouse_move(*x, *y),
            InputKind::FastForward => {
                game.cheat(Cheat::FastForward, || scenario.delta_ms);
            }
            InputKind::Restart => game.new_game(),
            InputKind::Pause => game.toggle_pause(),
            InputKind::Focus { active } => game.set_active(*active),
            InputKind::Resize { w, h } => game.resize(*w, *h),
            InputKind::Cheat { key } => {
                if let Some(cheat) = Cheat::from_char(*key) {
                    stepped |= game.cheat(cheat, || scenario.delta_ms);
                }
            }
        }
    }
    stepped
}

/// Run `scenario` at its client size with the given sprite sizes (the
/// real ones, `crate::assets::load_sprite_sizes`, for records that match
/// the original).
pub fn run_scenario(scenario: &Scenario, sprites: &SpriteSizes) -> Vec<TickRecord> {
    let (w, h) = scenario.client_size();
    run_scenario_observed(scenario, sprites, w, h, |_, _| {})
}

/// Like [`run_scenario`] at a given client size, calling
/// `observe(tick, &game)` after each tick (e.g. to dump the frame the
/// original paints at the end of it).
pub fn run_scenario_observed(
    scenario: &Scenario,
    sprites: &SpriteSizes,
    screen_w: i16,
    screen_h: i16,
    mut observe: impl FnMut(u32, &GameState),
) -> Vec<TickRecord> {
    let mut game = GameState::new_seeded(screen_w, screen_h, scenario.seed, sprites.clone());
    game.new_game();

    let mut records = Vec::with_capacity(scenario.ticks as usize);
    let mut hash = FNV_OFFSET_BASIS;

    for tick in 0..scenario.ticks {
        if !apply_inputs(&mut game, scenario, tick) {
            game.tick(scenario.delta_ms);
        }

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
            // Repaints since the last record: during this tick, or between
            // ticks before it (the first paint, a resize).
            status: std::mem::take(&mut game.status_painted).then(|| game.status_values.clone()),
            sounds: std::mem::take(&mut game.sounds).into_iter().map(super::game::sound::Sound::id).collect(),
            hash: 0,
        };
        hash = fnv1a_update(hash, record.payload_json().as_bytes());
        record.hash = hash;
        records.push(record);
        observe(tick, &game);
    }

    records
}

/// One JSON object per line.
///
/// # Panics
///
/// Never in practice: a `TickRecord` always serializes.
pub fn records_to_jsonl(records: &[TickRecord]) -> String {
    let mut out = String::new();
    for r in records {
        out.push_str(&serde_json::to_string(r).expect("record is always serializable"));
        out.push('\n');
    }
    out
}

/// The records of a JSON Lines text; blank lines are skipped.
///
/// # Errors
///
/// When a line is not a valid `TickRecord`.
pub fn parse_jsonl(text: &str) -> Result<Vec<TickRecord>, serde_json::Error> {
    text.lines().filter(|line| !line.trim().is_empty()).map(serde_json::from_str).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_scenario() -> Scenario {
        Scenario { seed: 1, ticks: 100, delta_ms: 40, screen_w: None, screen_h: None, inputs: vec![] }
    }

    #[test]
    fn run_is_deterministic() {
        let a = run_scenario(&base_scenario(), &SpriteSizes::placeholder());
        let b = run_scenario(&base_scenario(), &SpriteSizes::placeholder());
        assert_eq!(a, b);
    }

    #[test]
    fn jsonl_round_trip() {
        let records = run_scenario(&base_scenario(), &SpriteSizes::placeholder());
        let text = records_to_jsonl(&records);
        let parsed = parse_jsonl(&text).expect("golden text parses");
        assert_eq!(records, parsed);
    }
}
