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
- Loads 9 sound resources with `FindResourceA`/`LoadResource`/`LockResource`, only if `sndPlaySoundA` resolves (`FUN_00405620`)

### Game loop — message loop + timer, no threads

- `FUN_00404ad0` starts the timer: `SetTimer(hwnd, 0x29a, 0x28, GameTimerProc)` — timer **ID 666**, **40 ms interval (25 fps)**
- Each tick, `GameTimerProc` (`0x4047c0`) calls the frame update `FUN_00401000`, which:
  1. Computes frame delta with `GetTickCount`
  2. Runs the world update `FUN_00401e50` (iterates the actor linked list: velocities, collisions, spawn events)
  3. Repaints via `FUN_00401060`
  4. Flips the offscreen buffer to the screen, throttled to at most every ~328 ms (`FUN_00401b80`)
- Pause (F3, `FUN_00405760`) kills/starts the timer and retitles the window `"Ski Paused … Press F3 to continue"` (string id 2)

### Input (`SkiMainWndProc`, `0x405800`)

| Message | Handler | Action |
|---|---|---|
| `WM_KEYDOWN` | `FUN_00406170` | Arrows/numpad steer; **Enter/F2** restart; **Esc** minimizes; **F3** pause |
| `WM_MOUSEMOVE` | `FUN_00406550` | Mouse steering |
| `WM_LBUTTONDOWN` / DBLCLK | `FUN_004066d0` | Click actions |
| `WM_CHAR` | `FUN_00406780` | **Cheat/debug keys** (below) |
| `WM_GETMINMAXINFO` | — | Forces min window 320×300 |

Cheat keys (`WM_CHAR` handler):
- **`F`** — toggles `DAT_0040c670`, making the physics integrator (`FUN_00402be0`) apply velocity **twice per frame** (fast-forward)
- **`T`** — single-steps one game tick (slow motion)
- **`R`** — forces a repaint
- **`X`/`Y`/`x`/`y`** — teleport the skier ±2 px

### Physics & world

- Integrator `FUN_00402be0`: `position += velocity` per axis, gravity decrementing z
- Distances in **1/16-meter units** — e.g. slalom course length constant `0x4100` = 16640 units = 1040 m; freestyle counts down from `0x21c0`

### Rendering

- Pure GDI. **89 bitmap resources** (skier poses, trees, rocks, jumps, lift poles, dogs, snowboarders, yeti frames) + 6 icons
- Draws to an offscreen `CreateCompatibleDC`/`CreateCompatibleBitmap` buffer, then `BitBlt` to the window (`WM_PAINT` → `FUN_004060b0`)
- Status bar text via `TextOutA` with format strings from the string table (`Time:`, `Dist:`, `Speed:`, `Style:`, ids 3–14)

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
