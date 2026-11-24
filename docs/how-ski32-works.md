# How `ski32.exe` works

Reverse-engineered with [REA](https://github.com/morluto/rea) 6.10.1 + Ghidra 12.2 (native `decompile` built from official source for macOS arm64). Full pseudocode for all 165 decompiled functions is in `/tmp/ski32_decomp2/functions/`.

## Identity

- **What it is:** the 2005 32-bit recompile of Chris Pirih's *SkiFree* (1991)
- **Format:** PE32 GUI executable, Intel 386, 118,784 bytes, image base `0x400000`
- **SHA-256:** `3572d3757638bc1ae388c3d007ba59ba1f370911901e2a35d2f1297e8cf0ff35`
- Plain Win32 GDI game — no DirectX, no installer, no DRM, no network, no threads, no registry
- Debug assertions still name the original source file: `V:\hack\ski32\ski2.c` (in `FUN_00401240` assert calls)
- Imports: KERNEL32, USER32, GDI32, and a single WINMM import (`sndPlaySoundA`) for sound

## Architecture

### Startup (`FUN_004052d0`, called from `WinMain`)

- Single-instance check via `FindWindowA("SkiMain")` — a second launch just brings the existing window to front
- Registers two window classes: `SkiMain` (game window, min size forced to 320×300 via `WM_GETMINMAXINFO`) and `SkiStatus` (child status bar: Time / Dist / Speed / Style)
- Loads 9 sound resources with `FindResourceA`/`LoadResource`/`LockResource`, only if `sndPlaySoundA` resolves (`FUN_00405620`); see "Sound" below

### Game loop — message loop + timer, no threads

- `FUN_00404ad0` starts the timer: `SetTimer(hwnd, 0x29a, 0x28, GameTimerProc)` — timer **ID 666**, **40 ms** requested — Windows delivers it every **46.875 ms** (three 15.625 ms timer interrupts, ≈ 21.3 ticks/s; `docs/vm-ground-truth.md` item 24)
- Each tick, `GameTimerProc` (`0x4047c0`) calls the frame update `FUN_00401000`, which:
  1. Computes frame delta with `GetTickCount`
  2. Runs the world update `FUN_00401e50` (iterates the actor linked list: velocities, collisions, spawn events)
  3. Repaints via `FUN_00401060`
  4. Repaints the status display (`FUN_00401b80`) only when more than 327 ms have passed since its last repaint (every 7th tick at the real 46.875 ms; every 9th at 40 ms)
- Pause (F3, `FUN_00405760`) kills/starts the timer and retitles the window `"Ski Paused … Press F3 to continue"` (string id 2)

### Input (`SkiMainWndProc`, `0x405800`)

| Message | Handler | Action |
|---|---|---|
| `WM_KEYDOWN` | `FUN_00406170` | Arrows/numpad steer; **Enter/F2** restart; **Esc** minimizes; **F3** pause |
| `WM_MOUSEMOVE` | `FUN_00406550` | Mouse steering |
| `WM_LBUTTONDOWN` / DBLCLK | `FUN_004066d0` | Click actions |
| `WM_CHAR` | `FUN_00406780` | **Cheat/debug keys** (below) |
| `WM_GETMINMAXINFO` | — | Forces min window 320×300 |

Cheat keys (`WM_CHAR` handler, so case-sensitive, and they repeat while
held at the keyboard's repeat rate):
- **`f`** — toggles `DAT_0040c670`, making the physics integrator (`FUN_00402be0`) apply velocity **twice per frame** (fast-forward)
- **`t`** — runs one game tick on the spot (it calls the timer callback), even while paused
- **`r`** — forces a repaint
- **`x`** / **`X`** — teleport the skier 2 units right / left; **`y`** / **`Y`** — 2 units down / up. The view jumps with him (`FUN_00402390` → `FUN_00402470`), outside the scroll accounting, so a teleport spawns nothing at the screen edges

Uppercase F, R and T do nothing.

### Physics & world

- Integrator `FUN_00402be0`: `position += velocity` per axis, gravity decrementing z
- Movement `FUN_00403430` and steering `FUN_00406170` are ported faithfully — vy is the downhill speed (accelerates toward `max_speed`), vx is lateral and coupled to vy via a friction term. See `docs/skier-physics.md` for the full model.
- Distances in **1/16-meter units** — e.g. the slalom finishes at `0x21c0` = 8640 units = 540 m; tree slalom and freestyle finish at `0x4100` = 16640 units = 1040 m (the HUD's Dist counts down to the active course's finish)

### Rendering

- Pure GDI. **89 bitmap resources** (skier poses, trees, rocks, jumps, lift poles, dogs, snowboarders, yeti frames) + 6 icons
- Repaints only dirty rectangles: overlapping sprites are merged into a group, composed back to front (by world y) in a scratch `CreateCompatibleDC` buffer, then `BitBlt` to the window (`FUN_00401060` → `FUN_00401540`; full repaint on `WM_PAINT` → `FUN_004060b0`). Sprites are bottom-centre anchored. See `docs/vm-ground-truth.md` items 20–21
- Status display text via `TextOutA` in the OEM fixed font ("Terminal") with format strings from the string table (`Time:`, `Dist:`, `Speed:`, `Style:`, ids 3–14), in a black frame at the top-right

### Sound

The 32-bit build has a complete sound system. Only the sounds themselves are
missing: the shipped `ski32.exe` has no WAVE resources, so `FindResourceA`
fails and the game has always been silent.

- **Loading:** `FUN_004052d0` loads WAVE resources 1–9 into nine 8-byte
  slots (`FUN_00405640`: handle, locked pointer). Ids 10 and 11 exist in
  some builds but are never loaded.
- **Playing:** `FUN_00402ba0(ecx = slot)` calls `sndPlaySoundA(data, 5)`,
  where 5 = `SND_ASYNC | SND_MEMORY`. Only one sound plays at a time; a new
  one cuts off whatever is playing.
- **Missing resources:** the null check comes *inside* `FUN_00402ba0`, so the
  routine is called at every event even when the resources are absent.
  `tools/vm/trace_sounds.js` logs these calls on the shipped EXE.
- **Muting:** the command line `nosound` (the whole command line, compared
  with `lstrcmpiA` in `FUN_004047e0`) sets `DAT_0040c794`, which mutes
  everything. On exit, `FUN_004056a0` calls `sndPlaySoundA(NULL, 0)` and
  frees the resources.

| Id | Slot | When it plays | Call site |
|---|---|---|---|
| 1 | `0x40c6c0` | A hit that costs 32 style (the landing code at `0x402aeb` also has a −64 crash-landing branch, but it never runs: the landing table has no `0x11`) | `0x403e48` |
| 2 | `0x40c768` | The ramp, the automatic hop over a large rock, an airborne bounce off a rock or a large rock | `0x403ccb`, `0x403be8` |
| 3 | `0x40c5d0` | A snowboarder knocked down by anything moving | `0x403fa8` |
| 4 | `0x40c718` | Every landing (the landing table never yields `0x11`) | `0x402aeb` |
| 5 | `0x40c750` | A crossing skier (class 3) launched by a large rock or the ramp (the table at `0x404054`; a snow bump does nothing) | `0x403ef3` |
| 6 | `0x40c628` | A dog knocked over, by anything | `0x403f5f` |
| 7 | `0x40c6e0` | The monster catches the player | `0x403b1e` |
| 8 | `0x40c608` | A snowboarder jumps | `0x40371a` |
| 9 | `0x40c6f0` | The monster chases the player (each chase step) | `0x4045d3` |

The chase sound plays on every chase step. Each step makes the monster hop
(`vz = 1`), and it skips the chase while airborne, so a chasing monster
steps, and plays sound 9, on every third tick (`docs/monster-mob-physics.md`).

The High Scores box makes no sound. It is a `MessageBoxA` with `MB_OK` and
no icon, and Windows' `SoftModalMessageBox` calls neither `MessageBeep`
nor `PlaySound` for such a box (checked in the VM), so the port's box is
silent too.

**A build with sounds:** foone's `ski32sounds.exe` (2005) has the same
code, `.rdata` and `.data` as `ski32.exe`, plus WAVE resources 1–11
(mono, 8-bit, 11025 Hz; ids 4, 10 and 11 at 22050 Hz).

### Persistence

- High scores via `GetPrivateProfileStringA`/`WritePrivateProfileStringA` in **`entpack.ini`** — the shared Microsoft Entertainment Pack settings file
- `"High Scores"` entries with the `" <-- that's you!"` / `" <-- try again!"` markers (string ids 15–17)

## Evidence

- PE imports: REA Evidence `ev_0c73280c8f43bd9d5b2d438fecbbf08fe5154a6c53cd09dafcf45bc7195b015b`
- PE resources: `/tmp/ski32_rsrc.json` (89 bitmaps ids 1–89, 6 icons, 2 string tables)
- Ghidra workflow overview: REA Evidence `ev_5ae27d9bf1b7fbe25b2a839729bd271b80e20ee9a48c4852ca56be5dfdf527e9` (257 procedures, 170 strings)
- Decompiled functions: `/tmp/ski32_decomp2/functions/` (165 of 257 procedures; the rest are external thunks)

## Limitations

- Pseudocode is Ghidra decompiler output, **not** original source
- Ghidra 12.2 has two known switch-recovery defects (ghidra-9996, ghidra-10154); neither is known to affect this x86 binary
- Two address-taken routines (timer proc, main WndProc) were missed by auto-analysis and seeded manually via `REA_GHIDRA_SEED_FILE`
