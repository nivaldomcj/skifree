# Porting guide — architecture, fidelity method, known gaps

How the Rust port is organized and how we keep it faithful to `ski32.exe`.
Read this before changing game logic. For the physics details see
`docs/skier-physics.md` and `docs/monster-mob-physics.md`.

## Crate layout

The crate is split so the game logic runs headless (no window) for testing.

```
src/
  lib.rs            # exposes `game` + `replay` (no macroquad)
  main.rs           # the playable game (macroquad window, input, render)
  bin/replay.rs     # CLI: run a scenario, print/check/update golden logs
  game/             # pure game logic — macroquad-free, fully deterministic
    state.rs        # GameState: actor pool, tick loop, input entry points
    actor.rs        # 100-slot actor pool (free list), mirrors FUN_00404a00
    types.rs        # Actor, ActorClass, SpawnEvent, Camera, CourseType
    physics.rs      # integrator (FUN_00402be0) + steering (FUN_00403430)
    skier.rs        # skier state machine + steering (FUN_00406170)
    monster.rs      # yeti (FUN_004041c0 + FUN_00404350)
    mob.rs          # dog/snowboarder/obstacle/ambient/lift/eating
    course.rs       # course + spawn-event generation (FUN_00404b50)
    collision.rs    # actor collision (FUN_00403a00 subset)
    rng.rs          # MSVC LCG (FUN_00406cda), with call counter
    state_table.rs  # byte-verified data tables from the binary
    scoring.rs      # HUD scoring/formatting
  platform/         # macroquad-specific: window, input, renderer, audio, persistence
  replay.rs         # Scenario/InputEvent/TickRecord + run_scenario
tests/
  replay.rs         # golden-file regression tests
replays/            # scenario JSON + golden/*.jsonl
```

`main.rs` owns the only macroquad dependency (`platform/`). Everything under
`game/` is plain Rust and can be driven without a window, which is what makes
the replay harness possible.

## Fidelity method

The workflow for any behavior fix:

1. **Decompile** the relevant `FUN_*` from the original binary.
2. **Port** it faithfully — same constants, same control flow, same integer
   widths (the original does `int` math with `short` stores; we use `i32`
   intermediates and `as i16` to match truncation).
3. **Verify** with the deterministic replay harness (below) and byte-level
   table tests.

### Deterministic replay

`GameState::new_seeded(seed)` makes runs reproducible (the shipped `new()`
still uses the clock). A *scenario* (JSON: seed, course, tick count, scripted
inputs) runs N fixed 40 ms ticks and emits a per-tick `TickRecord` (player
pos/vel/state, actor count, score, RNG call count, running FNV-1a hash).
Committed *golden* logs under `replays/golden/` lock the behavior; `cargo test`
re-runs every scenario and diffs exactly. Regenerate only intentionally:

```bash
cargo run --bin replay -- replays/descend.json --update-golden replays/golden/descend.jsonl
```

### Testing ladder (how we validated)

- **Level 1 — static tables.** `state_table.rs` tests read `ski32.exe` and
  assert `STATE_TO_SPRITE`, `SKIER_PHYSICS`, `LEFT/RIGHT_TRANSITION`,
  `CRASH_LANDING`, dog/snowboarder physics byte-for-byte. (These need a local
  `ski32.exe`; see "Copyrighted files" below.)
- **Level 2 — deterministic replay.** Golden logs + invariants (pool ≤ 100,
  RNG monotonic, run-to-run identical).
- **Level 3 — visual playtest** against the original (Wine/VM). Not done yet.

## Recovered model highlights

- **Units:** positions/velocities in **1/16-meter** units; one tick = 40 ms
  (25 fps, timer ID 666). Freestyle course is `0x21c0` units ≈ 540 m.
- **Actor struct (80 bytes):** `next=+0x00, class=+0x18, state=+0x1c,
  x=+0x40, y=+0x42, z=+0x44, vx=+0x46, vy=+0x48, vz=+0x4a, flags=+0x4c`.
- **Mover proxy struct (monsters + ambient mobs):** `class=+0xc, state=+0x10,
  x=+0x14, y=+0x16, z=+0x18, vx=+0x1a, vy=+0x1c, vz=+0x1e, timestamp=+0x20`.
  `FUN_004041c0` integrates it; a behavior routine sets velocity/state; the
  result is synced to a visible actor. The port folds this into one actor but
  keeps the same per-tick math.
- **Steering is table-driven**: two interleaved transition tables
  (`DAT_0040a258`, LEFT at +0 / RIGHT at +4, 8 bytes per state).

## Regenerating the decompilation

The 165 decompiled functions used for the port live in
`/tmp/ski32_decomp2/functions/` (one `0xADDR.c` each) — **not committed** and
may be gone after a reboot. To regenerate: configure Ghidra 12.x
(`GHIDRA_INSTALL_DIR`) and use REA's `open_binary` + `decompile_all` on
`ski32.exe`. `docs/how-ski32-works.md` has the binary identity and REA evidence.

## Known gaps & deliberate deviations

- **Esc → quit**, not minimize. macroquad/miniquad expose no minimize API (the
  `WindowMinimized` event is inbound-only and the native handle is private), so
  a literal minimize would need unsafe per-OS FFI. Chosen deliberately; see
  `docs/skier-physics.md`.
- **No sound.** The binary references 9 `WAVE` resources but they are absent
  from the shipped `ski32.exe` (`FindResourceA` returns NULL). The audio layer
  is a stub awaiting free replacement sounds.
- **Ambient-mob secondary spawn** (1-in-1000) and the **snowboarder jump
  effect** are ported via `SpawnRequest` (see `docs/monster-mob-physics.md`).
  The one simplification: the original re-materializes culled proxy actors
  when they scroll back into view; the port keeps actors alive instead.
- **Monster/mob proxy vs actor**: the port uses one actor instead of the
  proxy+actor pair; observable behavior is preserved, internal structure is not.
- **Diagonal keys** map through the cardinal transitions rather than setting
  states entirely independently (Home/End/PgUp/PgDn now map to states 3/1/6/4).

## Copyrighted files (not in this repo)

`SKI.EXE`, `ski.zip`, `ski32.exe`, `ski32.zip` and everything extracted from
them (all of `assets/` except `assets/README.md`) are Microsoft/Chris Pirih
property and are **gitignored and scrubbed from history**. Place a `ski32.exe`
in the repo root and run `python3 tools/extract_assets.py` to regenerate
`assets/`. The `state_table` byte-comparison tests also need that file; they
skip gracefully when it's absent.
