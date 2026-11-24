# Porting guide — architecture, fidelity method, known gaps

How the Rust port is organized and how we keep it faithful to `ski32.exe`.
Read this before changing game logic. For the physics details see
`docs/skier-physics.md` and `docs/monster-mob-physics.md`.

## Crate layout

The crate is split so the game logic runs headless (no window) for testing.

```
src/
  lib.rs            # exposes `game`, `replay` + `extract` (no macroquad)
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
    sound.rs        # the nine sound events (FUN_00402ba0), recorded per tick
  extract/          # AssetSet + reading it from the player's exe/zip/fon (web)
  platform/         # macroquad-specific: window, input, renderer, audio, persistence
    assets.rs       # loads the AssetSet (assets/, embedded, or extracted)
    web.rs          # bindings for web/skifree.js (wasm32 only)
  replay.rs         # Scenario/InputEvent/TickRecord + run_scenario
tests/
  replay.rs         # golden-file regression tests
  extract.rs        # Rust extractor vs tools/extract_assets.py output
replays/            # scenario JSON + golden/*.jsonl
web/                # the page (index.html) and its miniquad plugin (skifree.js)
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

- **Units:** positions/velocities in **1/16-meter** units; one tick = the 40 ms
  timer (ID 666) as Windows delivers it, every 46.875 ms (≈ 21.3/s;
  replays use a fixed 40 ms). Freestyle course is `0x21c0` units ≈ 540 m.
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
- **Sound is optional, and a small addition.** The shipped `ski32.exe` has no
  `WAVE` resources, so the original is silent. The port can play the sounds
  from foone's `ski32sounds.exe` (`docs/how-ski32-works.md`, "Sound"):
  - **Logic:** `game::sound` records each `FUN_00402ba0` call per tick. The
    calls are verified against the original by `tools/vm/trace_sounds.js` /
    `sound_align.py`.
  - **Playback:** `platform::audio` plays the last sound of each tick, cutting
    off the previous one, like `sndPlaySoundA`. On Windows it *is*
    `sndPlaySoundA`; macOS uses NSSound; Linux loads libasound at runtime.
  - **Build switch:** build.rs embeds the nine WAVs and sets
    `cfg(skifree_sound)` only when `assets/sounds/` holds all nine. Without
    game files, `SKIFREE_NO_ASSETS=sound` compiles the audio code over empty
    stand-ins for linting, and never opens a player.
  - **Additions:** M mutes and unmutes; the original's `nosound` command line
    starts the game muted.
- **Ambient-mob secondary spawn** (1-in-1000) and the **snowboarder jump
  effect** are ported via `SpawnRequest` (see `docs/monster-mob-physics.md`).
- **Monster/mob proxy vs actor**: the port uses one merged actor instead of
  the original's mob node + actor pair, with `flags.on_screen` standing in
  for "the node has a live actor" (materialization, culling, density and
  collisions all honor it). Observable behavior — down to every RNG call —
  matches; internal structure does not. The pool reserves the nodes' slots
  on top of the original's 100 actors and counts a node as live only while
  it is on screen, so the 100-actor limit falls where the original's does
  (it is reached only on windows above 4K).
- **Collisions** now follow the original exactly — both pair orders, the
  crossing flag, every first-actor class (see `docs/vm-ground-truth.md`).
- **Diagonal keys** map through the cardinal transitions rather than setting
  states entirely independently (Home/End/PgUp/PgDn now map to states 3/1/6/4).

## Feel on real displays and input devices

Checked on macOS (Retina, 2560×1080 points) beyond the VM comparisons:

- **Pacing:** 21.3 ticks/s, the original's timer as Windows delivers it
  (`platform::timer`, `docs/vm-ground-truth.md` item 24). On a 60 Hz
  display a tick lands on every 3rd or 2nd frame, which the original on a
  60 Hz monitor shows the same way (GDI paints at the tick, the screen
  shows it at the next refresh).
- **Sharp pixels:** `high_dpi` is off, so the game renders at 1× client
  pixels and macOS scales the window 2× on Retina. The captured window is
  nearest-neighbour (99.97 % of 2×2 blocks uniform, only the 16 sprite
  colours): no blur.
- **Mouse coordinates:** miniquad's macOS backend reads the pointer one
  backing pixel too high (`height − y·scale − 1`): with synthetic NSEvents
  at known positions the port saw row 99 for a pointer in row 100, which
  shifts mouse steering's direction thresholds. `client_mouse_position()`
  undoes it on macOS and floors Cocoa's fractional positions; verified
  exact at 8 positions (both halves of a pixel row, top row included).
  Windows and X11/Wayland pass client coordinates through unchanged.
- **Key repeat:** the original steers one step per `WM_KEYDOWN`, auto-
  repeats included, so holding an arrow depends on the OS repeat delay and
  rate; the port does the same with the OS's repeats (a deliberate choice:
  no emulated Windows timing). Windows' defaults are ~500 ms delay and
  ~33 ms rate; elsewhere the player's own keyboard settings apply.

## Linux (tested on Debian 13, arm64)

All four sessions of one install — GNOME (Wayland), GNOME on Xorg, Plasma
(Wayland), Plasma (X11) — and, in the Wayland sessions, both of the port's
backends (default X11 through XWayland, and `SKIFREE_LINUX_BACKEND=wayland`):

- The full test suite passes on aarch64 Linux, golden logs included (the
  simulation is the same on every platform).
- Rendering, the extracted Terminal font and the bundled IBM VGA fallback
  all display correctly.
- **Pause:** on X11 and XWayland the window title changes like the
  original's. miniquad doesn't expose its X11 window, so a separate Xlib
  connection (libX11 loaded at runtime) takes the window that has the
  keyboard focus when F3 is pressed, climbs to the top-level window whose
  `WM_CLASS` is `skifree`, and sets `WM_NAME`/`_NET_WM_NAME`
  (`platform::os_window`). On native Wayland the title lives on miniquad's
  private `xdg_toplevel`, so the notice box shows in the window there.
  Verified in all four sessions: title on X11/XWayland, box on native
  Wayland, both appearing on F3 and going away on resume.
- **High-score box:** the drawn box appears at a finish and closes on
  Enter in every session.
- **Title bar:** GNOME on Wayland leaves it to the app — miniquad draws it
  through libdecor ("SkiFree", GNOME style); KWin and the X11 sessions
  draw their own.
- **Icon:** X11 windows (and XWayland) show the SkiFree icon (Plasma shows
  it in the title bar and taskbar; GNOME shows no title-bar icons). Native
  Wayland takes icons from an installed `.desktop` file matched by app id:
  the port's app id / `WM_CLASS` is `skifree`, and
  `tools/install_linux_desktop.sh` installs `skifree.desktop` plus the icon
  (copied from your extracted assets) for the current user; with it, KWin's
  title bar and taskbar and GNOME's overview and dash show the SkiFree
  face.

Driving the VM over VMware's VNC with `vncdotool`: Shift never reaches the
guest (capitals arrive lowercase, `&` as `7`, `|` as `\`), so type commands
without shifted characters (`;` instead of `&&`) or use SSH.

## Web platform layer

The browser build is the same crate compiled to `wasm32-unknown-unknown`;
`game/` is untouched. `tools/build_web.sh` puts the `.wasm` next to
`web/index.html`, `web/skifree.js` and the `gl.js` loader of the miniquad
version in `Cargo.lock`.

- **Assets.** `platform::assets` loads one in-memory `AssetSet` on every
  platform: from `assets/` natively, embedded by build.rs in the private
  web build (`--features embed-assets`), or, in the public build, extracted
  in the page by `skifree::extract` from the files the player drops (cached
  in IndexedDB as dropped, re-extracted on each visit). `tests/extract.rs`
  checks the extractor against the Python script: same pixels, sizes and
  WAV bytes.
- **Game hooks.** wasm32-unknown-unknown has no file system and no
  `SystemTime`, so the platform layer installs the sprite sizes
  (`game::assets::install_sprite_dims`) and the clock that seeds new games
  (`game::state::set_clock_source`). Both fall back to `meta.json` and
  `SystemTime` for the replay tools and tests.
- **Glue.** `web/skifree.js` is a miniquad plugin; `platform::web` holds
  its Rust bindings. The pause notice goes in the tab title (in fullscreen
  the game draws it, as natively without a title bar). High scores live in
  `localStorage` under `skifree.scores`, the same JSON as
  `~/.skifree/scores.json`. Sound goes through Web Audio, one at a time
  with a new one cutting off the old, decoded from the WAVs' own PCM;
  browsers allow audio only after the first key press or click. The plugin
  also runs the drop box, sets the favicon from the player's icon 3 (16×16
  reduced like the title-bar icon) and calls `preventDefault` for the game
  keys gl.js leaves to the browser (Home, End, PgUp, PgDn, Insert, Enter,
  the numpad), and for `'` and `/`, which gl.js means to block but doesn't
  (it maps them to key codes 222 and 189 and checks 39 and 47), so
  Firefox's Quick Find would take the keyboard.
- **Files.** Only the drop box takes files. Over the running game a drop
  does nothing (the cursor shows "not allowed"); `?forget` clears the saved
  files so the box comes back.
- **Timer, focus, size.** The 46.875 ms timer runs on `get_time()` as
  natively. gl.js reports focus loss and hidden tabs through the same
  events as a native window, so the game deactivates the same way. The
  canvas fills the page at one game pixel per CSS pixel (nearest-neighbour
  on high-DPI screens) with the 304×261 minimum client size set in CSS.
- **Deviations:** Esc does nothing (a tab can't minimize like the
  original, nor quit like the native port). The high-score box is the one
  the game draws.
- **Additions:** the file drop and its cache (`?forget` clears it), the
  favicon, and `?nosound` for the original's `nosound` argument.
- **Checks.** `tools/test_wasi.sh` runs the replay tests compiled to
  `wasm32-wasip1` under wasmtime (cargo builds the game binary for WASI
  too, which is why build.rs treats every wasm32 target as the web build).
  `tools/web/compare_frames.sh` draws replay frames in headless Chrome
  (`?replay=<scenario>&ticks=...`) and natively and compares them byte for
  byte: 26 frames from 8 scenarios are identical.
- **Browsers.** Played on macOS in Chrome, Firefox and Safari: keys (F2
  and F3 included), sound, the drop box with `ski32.zip`, and high scores
  that survive a reload.

## Copyrighted files (not in this repo)

`SKI.EXE`, `ski.zip`, `ski32.exe`, `ski32.zip`, `ski32sounds.exe` and everything extracted from
them (all of `assets/` except `assets/README.md`) are Microsoft/Chris Pirih
property and are **gitignored and scrubbed from history**. Place a `ski32.exe`
in the repo root and run `python3 tools/extract_assets.py` to regenerate
`assets/`. The `state_table` byte-comparison tests also need that file; they
skip gracefully when it's absent.
