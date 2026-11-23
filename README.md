# SkiFree — a faithful Rust port (personal project)

A multiplatform reimplementation of **SkiFree** in Rust, built by reverse
engineering the original `ski32.exe` (the 2005 32-bit recompile of Chris
Pirih's 1991 classic) with [REA](https://github.com/morluto/rea) + Ghidra and
porting the recovered logic faithfully. Runs on macOS, Windows and Linux.

<p align="center">
  <img src="https://github.com/nivaldomcj/skifree/raw/refs/heads/docs/media/screenshots/macos.png"
       alt="The port running on macOS: the yeti chases the skier uphill" width="480">
</p>

<p align="center"><sub>The port on macOS. Screenshot of the game running with
assets extracted from my own copy; SkiFree's graphics are © Chris Pirih /
Microsoft. The same seed and inputs (<code>replays/showcase.json</code>) give
the same picture on Windows and Linux.</sub></p>

## ⚠️ No copyrighted game files here

This repository contains **only our own code and documentation** (plus the
screenshot above, kept on the separate `docs/media` branch). It does **not**
contain, and we do **not** distribute:

- the original game binaries (`ski32.exe`, `SKI.EXE`, or any zip of them), or
- the assets extracted from them (sprites, icons, string tables, sounds).

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

3. Optional, for the original's status display (Time / Dist / Speed /
   Style) pixel for pixel: SkiFree draws it in Windows' "Terminal" font
   (the stock OEM fixed font), which is part of Windows, not of SkiFree.
   Copy `C:\Windows\Fonts\vgaoem.fon` from a Windows installation you
   own into `originals/` before step 2; the extractor then also writes
   `assets/fonts/oem_fixed.png`. Without it the status display uses the
   bundled IBM VGA 8x14 font (from The Ultimate Oldschool PC Font Pack),
   which looks very close.

4. Optional, for sound. The original code plays nine sound effects, but
   the shipped `ski32.exe` never contained the sounds, so SkiFree has always
   been silent. Chris Pirih later sent the sounds to foone, who wrote
   [Uncovering the sounds of SkiFree](https://foone.wordpress.com/2017/06/20/uncovering-the-sounds-of-skifree/)
   and published a build with them embedded, `ski32sounds.exe`
   ([ski32sounds.zip on the Wayback Machine](https://web.archive.org/web/20240324032244/http://foone.org/downloads/skifree/ski32sounds.zip)).
   Put `ski32sounds.exe` in `originals/` (next to `ski32.exe`, or instead
   of it) before step 2. The extractor then writes `assets/sounds/`, and the
   next build has sound. Delete `assets/sounds/` for a silent build.

   Sound is a small addition to the original game. It plays from launch,
   **M** mutes and unmutes, and starting the game with the original's
   `nosound` argument starts it muted. As in the original, only one sound
   plays at a time, and a new one cuts off the one before.

## Build & run

```bash
cargo run            # play (window opens)
cargo build --release
```

Windows build from macOS (`brew install mingw-w64`,
`rustup target add x86_64-pc-windows-gnu`):

```bash
CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
  cargo build --release --target x86_64-pc-windows-gnu
```

Put `skifree.exe` next to an `assets/` folder.

On Linux the game uses X11 by default (through XWayland in a Wayland
session). `SKIFREE_LINUX_BACKEND=wayland` runs it on native Wayland
(`wayland-first` / `x11-first` try one and fall back to the other).
`tools/install_linux_desktop.sh` adds a menu entry and the SkiFree icon for
your user (Wayland shows app icons from desktop entries);
`--uninstall` removes them.

Controls are faithful to the original (arrows/numpad steer, Insert/numpad0 or
click to jump, F2 restart, F3 pause, F fast-forward, T step, X/Y teleport),
plus M to mute in builds with sound.
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

## Legal note

This is an unofficial, personal, non-commercial project for interoperability
and study. It is not affiliated with or endorsed by Chris Pirih or Microsoft.
All game code, assets, and the SkiFree name remain the property of their
respective owners. Do not redistribute the original binaries or extracted
assets.

## License

The original code, tools, and documentation written for this project are
released under the [MIT License](LICENSE). That license grants **no rights to
SkiFree itself**: the original game, its binaries, assets, data, and name are
© Chris Pirih / Microsoft and are not included or licensed here.

The fallback status-display font in `resources/fonts/` is derived from "IBM
VGA 8x14" in [The Ultimate Oldschool PC Font
Pack](https://int10h.org/oldschool-pc-fonts/) by VileR and is licensed under
[CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/); see
`resources/fonts/README.md`.
