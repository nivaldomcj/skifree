# Deterministic replay harness

## Goal

Level 2 fidelity adds a deterministic replay harness to the SkiFree port. It lets us run scripted games headlessly, compare the resulting per-tick state logs against committed "golden" logs, and detect when any code change alters game behavior.

## How it works

- **Scenarios** are JSON files under `replays/` describing a deterministic run:
  - `seed`: the MSVC LCG seed.
  - `course`: `freestyle`, `slalom`, or `tree_slalom`.
  - `ticks`: number of 40 ms ticks to simulate.
  - `delta_ms`: frame duration (defaults to 40, matching the original 25 fps timer).
  - `inputs`: a list of `{tick, type, ...}` events mapping to the same `GameState` methods the real game uses.
- `cargo run --bin replay -- <scenario.json>` runs the scenario and prints JSONL tick records.
- `cargo run --bin replay -- <scenario.json> --check <golden.jsonl>` checks a run against a golden log.
- `cargo run --bin replay -- <scenario.json> --update-golden <golden.jsonl>` regenerates the golden file after an intentional change.
- `cargo test` runs the integration tests in `tests/replay.rs`, which compare every committed scenario against its golden log and assert structural invariants (pool ≤ 100 actors, RNG call count monotonic, deterministic across runs).

## Files

| File | Purpose |
|------|---------|
| `src/lib.rs` | exposes the game-logic-only library so bins and tests can use it without macroquad |
| `src/replay.rs` | `Scenario`, `InputEvent`, `TickRecord`, `run_scenario`, JSONL helpers |
| `src/bin/replay.rs` | CLI runner |
| `tests/replay.rs` | golden-file regression tests |
| `replays/*.json` | scenario definitions |
| `replays/golden/*.jsonl` | expected per-tick state logs |

## Scenarios

- `idle.json` — 250 ticks, no input. Baseline for monster approach and ambient spawns.
- `drift_left.json` — 500 ticks, no input. The skier accelerates left in the current port, so this is the long-term drift baseline.
- `steer_and_jump.json` — 500 ticks with scripted left/right steering, two jumps, and fast-forward toggles. Exercises the skier state machine, jump states, and fast-forward doubling.

## Determinism

Two key changes make the harness deterministic:

1. `GameState::new_seeded(screen_w, screen_h, seed)` replaces the time-based seed.
2. The RNG now counts every `rand()` call, so divergences can be localized.

The original `GameState::new()` still uses the system clock, so `cargo run` remains a normal, non-deterministic play session.

## Known issue surfaced by the harness

The first golden logs immediately showed that the skier's `y` coordinate never changes: the current `physics::apply_steering` only accelerates `vx` based on the state-table `direction` field and only decays `vy` via friction. There is no downhill acceleration at all. This means gates, finish lines, and most course events are never reached in the current port.

This is a known Phase 3 approximation. The state-table constants (`SKIER_PHYSICS`) are byte-for-byte verified against `ski32.exe`, but `apply_steering` appears to oversimplify how the original interprets them. Recovering the exact original physics is the next fidelity step; the replay harness now gives us a safe way to verify any fix.

## Verification

```bash
cargo test
cargo run -q --bin replay -- replays/steer_and_jump.json > /tmp/a.jsonl
cargo run -q --bin replay -- replays/steer_and_jump.json > /tmp/b.jsonl
diff /tmp/a.jsonl /tmp/b.jsonl   # byte-identical
cargo build --release
```

All tests pass and the two runs are byte-identical.
