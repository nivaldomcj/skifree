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
- `cargo run --bin replay -- <scenario.json> --draw <out.jsonl> --draw-ticks 100,200` also writes the draw list (sprite, box, painting order — `game::draw`) of the frame after each listed tick, for the pixel comparison with the original (`tools/vm/frames.sh`, see `docs/vm-ground-truth.md`).
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

- `idle.json` — 250 ticks, no input. The skier stays at the start; baseline for monster approach and ambient spawns.
- `descend.json` — 500 ticks, DOWN pressed once. The skier accelerates downhill and scrolls the course; the main descent baseline.
- `steer_and_jump.json` — 500 ticks with a DOWN start, left/right fan steering, direct diagonal states, a mouse jump, a keyboard jump, a brake, and fast-forward toggles. Exercises the full skier state machine and control set.

### Verified against the original

The `compare_*.json` scenarios were run against the original `ski32.exe` in
a VM (`tools/vm/compare.sh`) and match it call for call — every `rand()` call
and every player tick. Their goldens therefore encode the original game's
behavior, not just the port's: `verified_*` tests in `tests/replay.rs` fail
if a change breaks that agreement. See `docs/vm-ground-truth.md` for the
table of scenarios and how they were verified.

- `compare_vm.json` — straight descent, no input after the tick-0 DOWN.
- `compare_steer.json` — every input type: turns, diagonals, brake, both jumps, air tricks.
- `compare_size_640x480.json`, `compare_size_1024x700.json` — the steering inputs at other window sizes.
- `compare_slalom.json`, `compare_tree_slalom.json` — down the two gated strips, with crashes and recoveries.
- `compare_monster.json` — walking uphill past y −2000: the monster's chase, meal and departure.
- `compare_freestyle.json` — click jumps and air poses in the freestyle strip (trick scoring).
- `compare_mouse.json` — mouse steering on the ground and in the air.
- `compare_restart.json` — two F2 restarts mid-run.
- `compare_long.json` — the whole freestyle strip, past its finish line.

Their inputs use only what the original's handlers accept (`steer`,
`steer_absolute` 1/3/4/6, `jump`, `click`, `mouse`, `restart`), and the
scenario's `screen_w`/`screen_h` is the client size the original's world is
built at.

Input types: `steer {left,right,up,down}`, `steer_absolute {state}`, `jump`
(keyboard), `click` (mouse button), `mouse {x, y}` (cursor moved to client
coordinates), `restart` (F2), `pause` (F3 toggle), `focus {active}` (window
activation; while inactive every other input is dropped, like the
original's window procedure), `fast_forward` (F cheat).
Records include the style score.

## Determinism

Two key changes make the harness deterministic:

1. `GameState::new_seeded(screen_w, screen_h, seed)` replaces the time-based seed.
2. The RNG now counts every `rand()` call, so divergences can be localized.

The original `GameState::new()` still uses the system clock, so `cargo run` remains a normal, non-deterministic play session.

## Resolved: downhill physics

The first golden logs showed the skier's `y` never changed: the early
`apply_steering` was an approximation that only accelerated `vx` and decayed
`vy`, so the skier never descended. This was fixed by porting the real
`FUN_00403430` (downhill `vy` accelerates toward `max_speed`) and the real
`FUN_00406170` steering (DOWN enters state 0). See `docs/skier-physics.md`.
The `descend` scenario now locks the corrected descent with a golden log.

## Verification

```bash
cargo test
cargo run -q --bin replay -- replays/steer_and_jump.json > /tmp/a.jsonl
cargo run -q --bin replay -- replays/steer_and_jump.json > /tmp/b.jsonl
diff /tmp/a.jsonl /tmp/b.jsonl   # byte-identical
cargo build --release
```

All tests pass and the two runs are byte-identical.
