# SkiFree — a faithful Rust port (personal project)

A multiplatform reimplementation of **SkiFree** in Rust, built by reverse
engineering the original `ski32.exe` (the 2005 32-bit recompile of Chris
Pirih's 1991 classic) with [REA](https://github.com/morluto/rea) + Ghidra and
porting the recovered logic faithfully. macOS first; the code is portable.

## ⚠️ No copyrighted game files here

This repository contains **only our own code and documentation**. It does
**not** contain, and we do **not** distribute:

- the original game binaries (`ski32.exe`, `SKI.EXE`, or any zip of them), or
- the assets extracted from them (sprites, icons, string tables).

The original game is © Chris Pirih / Microsoft. To build and play, **you must
supply your own copy of `ski32.exe`** that you already own. The build extracts
the assets from your local copy at setup time — nothing copyrighted is ever
downloaded or committed.

## Getting the assets (required once)

1. Put your `ski32.exe` in the `originals/` folder:

   ```
   originals/ski32.exe
   ```

2. Extract the assets (Python 3, standard library only):

   ```bash
   python3 tools/extract_assets.py
   ```

   This reads `originals/ski32.exe` and writes the sprites, icons, string
   table, and metadata into `assets/`. Both `originals/` and the generated
   `assets/` files are gitignored.

   There are no placeholder assets: `build.rs` verifies the extraction and
   **fails the build** when `assets/` is missing or incomplete.

## Build & run

```bash
cargo run            # play (window opens)
cargo build --release
```

Controls are faithful to the original (arrows/numpad steer, Insert/numpad0 or
click to jump, F2 restart, F3 pause, F fast-forward, T step, X/Y teleport).
See `docs/skier-physics.md` for the full map.

## Verify

```bash
cargo test           # unit + golden-replay regression tests
```

The `state_table` tests byte-compare against `originals/ski32.exe` and **skip
gracefully** if it isn't present; everything else runs without it.

## Documentation

| Doc | Content |
|---|---|
| `docs/porting-guide.md` | **Start here** — crate layout, fidelity method, known gaps |
| `docs/how-ski32-works.md` | How the original binary works (loop, input, rendering, persistence) |
| `docs/skier-physics.md` | Skier movement, steering, jumping, full control map |
| `docs/monster-mob-physics.md` | Yeti + mob physics (rubber-band chase, eating, drift) |
| `docs/monster-ai.md` | Yeti overview (narrative) |
| `docs/replay-harness.md` | The deterministic replay / golden-log testing harness |
| `TODO.md` | Progress tracker |

## Legal note

This is an unofficial, personal, non-commercial project for interoperability
and study. It is not affiliated with or endorsed by Chris Pirih or Microsoft.
All game code, assets, and the SkiFree name remain the property of their
respective owners. Do not redistribute the original binaries or extracted
assets.
