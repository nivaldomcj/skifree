# Deterministic replay harness

## Goal

Level 2 fidelity adds a deterministic replay harness to the SkiFree port. It lets us run scripted games headlessly, compare the resulting per-tick state logs against committed "golden" logs, and detect when any code change alters game behavior.

## How it works

- **Scenarios** are JSON files under `replays/` describing a deterministic run:
  - `seed`: the MSVC LCG seed.
  - `ticks`: number of ticks (timer callbacks) to simulate.
  - `delta_ms`: the `GetTickCount` delta each tick sees. It defaults to 40, the fixed clock `tools/vm/fixed_clock.js` forces on the original for the verified captures. That is not the original's real rate: Windows delivers its 40 ms timer every 46.875 ms, with deltas of 46 and 47 (`docs/vm-ground-truth.md` item 24), so at 40 the status display repaints every 9th tick instead of every 7th and Speed reads 47/40 higher. A single `delta_ms` can't reproduce the 46/47 mix; 47 comes closest.
  - `screen_w`, `screen_h` (optional): the client size the world is built and run at. Without them a scenario runs at `DEFAULT_CLIENT_SIZE`, 640×480 (`Scenario::client_size`); the `compare_*` scenarios set 704×681, except the two `compare_size_*` ones.
  - `inputs`: a list of `{tick, type, ...}` events mapping to the same `GameState` methods the real game uses.
- **Records** (`TickRecord`, one JSON line per tick): the tick, `elapsed_ms`, the player's position, velocity, state and sprite, the active actor count (`actor_count`, off-screen mob nodes included, so not the original's live count), the score, the course clock, the `rand()` call count, the status display's four values when it was repainted that tick (`status`), the sounds played that tick in call order (`sounds`, WAVE ids 1–9, left out when empty), and a running FNV-1a `hash` of every record so far. `status` and `sounds` are compared with the original by `status_align.py` and `sound_align.py`.
- `cargo run --bin replay -- <scenario.json>` runs the scenario and prints JSONL tick records. Run it from the repo root: it refuses to run without the extracted `assets/meta.json` (README, "Getting the assets"), because the sprite sizes drive collisions, density and culling.
- `cargo run --bin replay -- <scenario.json> --check <golden.jsonl>` checks a run against a golden log.
- `cargo run --bin replay -- <scenario.json> --update-golden <golden.jsonl>` regenerates the golden file after an intentional change.
- `cargo run --bin replay -- <scenario.json> --draw <out.jsonl> --draw-ticks 100,200` also writes the draw list (`[sprite, left, top]` in painting order, `game::draw`) of the frame after each listed tick, for the pixel comparison with the original (`tools/vm/frames.sh`, see `docs/vm-ground-truth.md`).
- `cargo test` runs the integration tests in `tests/replay.rs`, which compare every scenario that has a golden in `replays/golden/` against it (`showcase.json`, the README screenshot run, has none). Each golden has its own `#[test]`, so a new golden needs a new test too. Sanity checks come on top: the skier descends in `descend`, ≤ 100 live actors (off-screen mob nodes excluded) at 640×480 and 5120×2880, and deterministic across runs. With the committed goldens these add little beyond the golden comparison; they matter once a golden is regenerated.
- The replay tests need the extracted `assets/meta.json` and panic without it, so CI, which has no game files, skips them (`.github/workflows/ci.yml` runs `--lib --bins --test extract`). Run `cargo test` locally before merging. `tools/test_wasi.sh` runs the same tests compiled to `wasm32-wasip1` under wasmtime.

## Environment variables

Read by the game binary (`target/debug/skifree`) unless noted. The `SKIFREE_LOG_*` switches are read by the game logic (`game/`), so the `replay` tool and the tests honor them too. None of them changes what a run computes, except `SKIFREE_SEED`, which picks the seed.

| Variable | Effect |
|----------|--------|
| `SKIFREE_REPLAY=<scenario.json>` | frame capture: play the scenario at its client size, one tick per frame, no live input (`docs/vm-ground-truth.md`) |
| `SKIFREE_FRAMES=<dir>`, `SKIFREE_FRAME_TICKS=<t1,...>` | where and after which ticks the capture saves frames; it quits after the last one |
| `SKIFREE_REPLAY_DIALOGS=1` | show the high-score box in a capture (normally skipped, like `tools/vm/no_dialogs.js`) |
| `SKIFREE_SEED=<u32>` | every new game starts from this seed instead of the clock, for side-by-side play with the patched original |
| `SKIFREE_LOG_RNG=1` | print every `rand()` call, in order (`rng_align.py`); only in a build with `--features rng_sites`, as `tools/vm/compare.sh` makes |
| `SKIFREE_LOG_UPDATES=1` | print each actor-loop update (`trace_updates.js`) |
| `SKIFREE_LOG_PAIRS=1` | print every ordered collision-handler call (`trace_pairs.js`) |
| `SKIFREE_LOG_COLLISIONS=1` | print each skier-first collision |
| `SKIFREE_LOG_SPAWNS=1` | print each spawn event as it gets its actor |
| `SKIFREE_LINUX_BACKEND` | Linux: `x11` (the default), `wayland`, `wayland-first` or `x11-first` (README) |
| `SKIFREE_NO_ASSETS=1` or `sound` | build time: let build.rs build without the game files (CI) |

## Files

| File | Purpose |
|------|---------|
| `src/lib.rs` | exposes the game-logic-only library so bins and tests can use it without macroquad |
| `src/replay.rs` | `Scenario`, `InputEvent`, `TickRecord`, `run_scenario`, `run_scenario_observed`, `apply_inputs` (also used by the game binary's frame capture), JSONL helpers |
| `src/assets.rs` | `load_sprite_sizes`: the sprite sizes from `assets/meta.json`, for the CLI and the tests |
| `src/bin/replay.rs` | CLI runner |
| `tests/replay.rs` | golden-file regression tests |
| `tools/test_wasi.sh` | the replay tests compiled to `wasm32-wasip1`, run under wasmtime |
| `replays/*.json` | scenario definitions |
| `replays/golden/*.jsonl` | expected per-tick state logs |

## Scenarios

- `idle.json` — 250 ticks, no input. The skier stays at the start; baseline for the monsters holding their regions and the random spawns.
- `descend.json` — 500 ticks, DOWN pressed every 10 ticks. The skier descends the course (y ≈ 6700), crashing three times and ending crashed; the main descent baseline.
- `steer_and_jump.json` — 500 ticks with a DOWN start, two Left presses, direct diagonal states, a mouse jump, a keyboard jump, an Up press at speed (no effect), and fast-forward toggles. Exercises the skier's steering and jump states; the `compare_*` scenarios cover restart, pause, focus, resize, mouse steering and cheats.

### Verified against the original

The `compare_*.json` scenarios were run against the original `ski32.exe` in
a VM (`tools/vm/compare.sh`) and match it call for call — every `rand()` call
and every player tick. Their goldens therefore encode the original game's
behavior, not just the port's: `verified_*` tests in `tests/replay.rs` fail
if a change breaks that agreement. See `docs/vm-ground-truth.md` for the
table of scenarios and how they were verified.

- `compare_vm.json` — straight descent, no input after the tick-0 DOWN.
- `compare_steer.json` — turns, diagonals, an Up press at speed (no effect), both jumps, air tricks.
- `compare_size_640x480.json`, `compare_size_1024x700.json` — the steering inputs at other window sizes.
- `compare_slalom.json`, `compare_tree_slalom.json` — down the two gated strips, with crashes and recoveries.
- `compare_slalom_timed.json`, `compare_tree_slalom_timed.json` — the two strips run as timed courses (course clock, +5 s missed-gate penalties).
- `compare_monster.json` — walking uphill past y −2000: the monster's chase, meal and departure.
- `compare_freestyle.json` — click jumps and air poses in the freestyle strip (trick scoring).
- `compare_mouse.json` — mouse steering on the ground and in the air.
- `compare_restart.json` — two F2 restarts mid-run.
- `compare_long.json` — the whole freestyle strip, past its finish line.
- `compare_resize.json` — the window resized four times mid-run, down to the minimum client.
- `compare_focus.json` — focus loss and regain (WM_ACTIVATE), inputs dropped while inactive, and an F3 pause that survives a focus change.
- `compare_sounds.json` — sound calls: a snowboarder's jump and knock-down in the same tick, take-off, landing, crashes.
- `compare_crash_landing.json` (seed 38) — a crash-landing skier crossing a tree: no second penalty.
- `compare_collision_order.json` (seed 20) — a skier state change seen by the next collision pair of the same pass.
- `compare_cheats.json` — the x/X/y/Y teleport cheats.
- `compare_step.json` — t steps during an F3 pause and outside one, in the timed slalom.
- `compare_catch.json` (seed 8) — the yeti's catch on a scroll boundary, then a click that starts a new game.

Their inputs use only what the original's handlers accept (`steer`,
`steer_absolute` 1/3/4/6, `jump`, `click`, `mouse`, `restart`, `pause`,
`focus`, `resize`, `cheat` x/X/y/Y/t/r; only `fast_forward` (and the `f`
cheat) has no deterministic counterpart in the original and is rejected by
`tools/vm/scenario_inputs.py`), and the
scenario's `screen_w`/`screen_h` is the client size the original's world is
built at.

Input types: `steer {left,right,up,down}`, `steer_absolute {state}`, `jump`
(keyboard), `click` (mouse button), `mouse {x, y}` (cursor moved to client
coordinates), `restart` (F2), `pause` (F3 toggle), `focus {active}` (window
activation; while inactive every other input is dropped, like the
original's window procedure), `resize {w, h}` (client area resized,
WM_SIZE), `fast_forward` (f cheat), `cheat {key}` (a `WM_CHAR` cheat key:
`x`/`X` teleport right/left, `y`/`Y` down/up, `r` repaint, `f` the same as
`fast_forward`, and `t`, which runs that tick as a step: record N is the
stepped tick and the timer's tick N doesn't run; put a `pause` before and
after it in the same tick to step while paused).
Records include the style score.

## Determinism

Two key changes make the harness deterministic:

1. `GameState::new_seeded(screen_w, screen_h, seed, sprites)` replaces the time-based seed. The sprite sizes (`SpriteSizes`) are handed in too: collisions, culling and density depend on them, so the goldens only match with the real sizes from `assets/meta.json` (`assets::load_sprite_sizes`).
2. The RNG now counts every `rand()` call, so divergences can be localized.

`GameState::new(screen_w, screen_h, sprites)` still uses the system clock, so `cargo run` remains a normal, non-deterministic play session unless `SKIFREE_SEED` is set.

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
