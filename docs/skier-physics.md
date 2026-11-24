# Skier physics and steering

Faithful port of the skier movement model, recovered from `ski32.exe` (Ghidra
12.2 decompilation). The actor field offsets come from the integrator
`FUN_00402be0`; the movement math comes from `FUN_00403430`; the steering comes
from `FUN_00406170`.

## Actor layout (offsets used by movement)

| Offset | Field |
|--------|-------|
| `0x18` | class (0 = skier) |
| `0x1c` | state |
| `0x40` | x |
| `0x42` | y |
| `0x44` | z (height) |
| `0x46` | vx (lateral velocity) |
| `0x48` | vy (downhill velocity) |
| `0x4a` | vz (vertical velocity) |

The skier spawns in **state 3** (`FUN_00404a80` → `FUN_004020d0(0, 3)`) at
`(0, 0, 0)`. Distances are in **1/16-meter units** and velocities in units per
tick (the 40 ms timer, delivered every 46.875 ms by Windows).

## Movement (`FUN_00403430`, `physics::apply_steering`)

Each skiing state has a 5-field physics entry (`DAT_0040a308`, 16 bytes per
state, first 10 bytes used): `accel, max_speed, decel, friction_div, direction`.

- **vy is the downhill speed.** It accelerates by `accel` toward `max_speed`,
  and decays by 2 when above `max_speed`:
  - `vy > max_speed` → `vy = max(vy - 2, max_speed)`
  - `vy <= max_speed` → `vy = min(vy + accel, max_speed)`
- **vx is the lateral speed**, coupled to vy through a friction term. With
  `dir = direction` (or the sign of `vx` when `direction == 0`):
  - `s4 = dir * vx` (signed lateral magnitude)
  - `friction = friction_div * max(vy, 0) / 2`
  - `s2 = clamp(friction, s4 - 2, s4 + decel)`
  - `vx = dir * s2`

All intermediate math is done in `int` and truncated to `short` on store; the
port mirrors this with `i32` intermediates and `as i16` stores.

Key consequence: **state 0 (straight down)** has `accel=1, max_speed=16,
direction=0`, so vy climbs 1→16 and the skier descends 16 units/tick (1 m/tick,
≈25 m/s). **States 3 and 6 (hard turns)** have `max_speed=0`, so vy scrubs to 0
— turning fully sideways stops your descent, exactly as in the real game.

## Steering (`FUN_00406170`, `skier::steer_keyboard`)

Each cardinal key is a discrete event:

| Key | Action |
|-----|--------|
| **Down** (`0x28`/numpad2) | On the ground → **state 0** (ski straight down). In the air → cycle jump-trick poses (`0xd→0x13→0x12→0xd`, `0x14→0xe`, `0x15→0xf`). |
| **Up** (`0x26`/numpad8) | On the ground, only when stopped (`vy == 0`): from state 3, 7 or `0xc` → state 9, from 6 or 8 → state 10, with `vy = -4`. This is a sidestep about 10 units uphill (y −4, −3, −2, −1 as vy comes back to 0); it also gets a fallen skier (`0xc`) back up. While moving it does nothing, so holding Up walks uphill one step every 4 ticks. In the air → cycle trick poses the other way. |
| **Left** (`0x25`/numpad4) | Step through `LEFT_TRANSITION`: `0→1→2→3→7`. Reaching state 7 also kicks `vx = max(vx - 8, -8)`. |
| **Right** (`0x27`/numpad6) | Step through `RIGHT_TRANSITION`: `0→4→5→6→8`. Reaching state 8 also kicks `vx = min(vx + 8, 8)`. |

The transition tables are stored interleaved at `DAT_0040a258` (8 bytes per
state: LEFT u32 then RIGHT u32). States 0–6 are the direction fan (straight,
slight/medium/hard left, slight/medium/hard right); 7 and 8 are hard-carve
snaps that `update_skier` immediately settles back to 3 and 6; 9/10 are the
uphill sidestep, which also settles back to 3 and 6 on the same tick;
`0xb`/`0xc` are crash/stopped; `0xd`–`0x15` are jump/trick poses.

Diagonal keys set a skiing state directly (only on the ground):

| Key | State |
|-----|-------|
| numpad7 / Home | 3 (hard left) |
| numpad9 / PgUp | 6 (hard right) |
| numpad1 / End | 1 (slight left) |
| numpad3 / PgDn | 4 (slight right) |

## Jumping (`FUN_004066d0`, `state::click`)

The two jump triggers differ, and both are ported:

- **Mouse click** (`FUN_004066d0`): on the ground → `vz = 4`, state `0xd`. In
  mid-air (and not in the flip state `0x11`) → cycles the trick poses
  (`0xd→0x12→0x13→0xd`, `0xe→0x14`, `0xf→0x15`). Does nothing while crashed.
  Once the yeti has eaten the skier, a click starts a new game, like Enter
  (it jumps to `FUN_00406500`).
- **Keyboard** (Insert/numpad0, in `FUN_00406170`): `vz = 2`, state `0xd`, and
  scrubs downhill speed (`vy -= 4` when `vy > 4`).

So the mouse gives a higher hop that keeps your speed, the keyboard a smaller
hop that also slows you down. Both are mapped in the port.

## Mouse steering (`FUN_00406550`, `GameState::mouse_move`)

Moving the mouse sets the skier's state directly from where the cursor is
relative to the skier's screen anchor (`dx = x − w/2`, `dy = y − h/3`):

- The very first mouse message only records the position; a message at the
  unchanged position is ignored; nothing happens while crashed (`0xb`) or
  crash-landing (`0x11`).
- **On the ground** (`FUN_004065e0`): cursor above the skier (`dy ≤ 0`) →
  hard right `6` / hard left `3` by the side; straight below (`dx = 0`) →
  `0`; otherwise `r = dy·4/dx` (truncated) picks `0` (|r| ≥ 12), `1`/`4`
  (≥ 6), `2`/`5` (≥ 3), `3`/`6` (≥ 1) for left/right; a ratio truncated to
  0 goes by the side like `dy ≤ 0`.
- **In the air** (`FUN_00406670`): a jump pose by quadrant and dominant
  axis — left: above → `0xe` (more sideways) or `0x10` (more upward), below
  → `0xe` or `0xd`; right: above → `0xf` or `0x10`, below → `0xe` (more
  sideways) or `0xd`.

## Trick scoring (`FUN_004028e0`)

Every tick, after the skier's state update, the new state earns style points
(byte table at `0x402b88`): pose `0x10` → +2, `0x12`/`0x13` → +4,
`0x14`/`0x15` → +8. Like all style points they only count while the
freestyle course runs (`DAT_0040c954`). Holding a trick pose through a long
jump is therefore worth a lot.

On touchdown the landing table (`0x40a434`) sends `0x0d`/`0x0e`/`0x0f` to
0/3/6 and every other air state (the trick poses `0x10`, `0x12`–`0x15`, and
the crash-landing `0x11`) to crash `0xb`. No entry is `0x11`, so the −64 /
sound 1 branch at `0x402ad0` never runs, in the original or in the port.
Every landing plays sound 4 and costs no style; landing in a trick pose only
crashes the skier.

## Controls parity

The port implements the original's full control set. Keys act per
**key-down message**, like the original's `WM_KEYDOWN` handling: one step
when a key is pressed, then one per OS auto-repeat while it is held (the
port used to step the state every frame a key was down). While the window
is inactive or minimized all keys, mouse moves and clicks are ignored, and
the click that re-activates the window is swallowed (see
`docs/courses-and-scoring.md`, Pause).

| Input | Original | Port |
|-------|----------|------|
| ← → ↑ ↓ / numpad 4 6 8 2 | turn / step uphill / straight down | same |
| numpad 1 3 7 9, Home End PgUp PgDn | direct diagonal states | same |
| Insert / numpad0 | keyboard jump (vz=2) | same |
| mouse move / click | steer / jump + tricks; a new game once the skier is gone | same |
| F2 | restart anytime | same |
| Enter | restart only once the skier is gone | same |
| F3 | pause | same |
| f, t, r (lowercase only); x/X, y/Y | fast-forward, step, repaint; teleport right/left, down/up (`WM_CHAR`, repeats while held) | same |
| Esc | **minimize window** | mapped to **quit** (see note) |

Note: the original maps Esc to `ShowWindow(hwnd, SW_MINIMIZE)`. The port maps
Esc to quit instead. This is a deliberate, considered deviation: macroquad /
miniquad (the port's windowing layer) expose **no minimize API** — the
`WindowMinimized` event is inbound-only and the native window handle is not
public — so a literal minimize would require unsafe, platform-specific FFI or
a windowing-library rewrite. Quit is the clean, portable choice across
macOS / Windows / Linux and avoids leaving a hidden process running. This is
the only deliberate deviation from the original controls.

## Verification

- `state_table` tests compare `SKIER_PHYSICS`, `LEFT_TRANSITION`,
  `RIGHT_TRANSITION`, `CRASH_LANDING`, and `STATE_TO_SPRITE` byte-for-byte
  against `ski32.exe`.
- `physics` tests assert downhill acceleration to max speed, decay above max
  speed, and speed-scrubbing on hard turns.
- The `descend` replay scenario drives the skier from `(0,0)` to
  `y ≈ 7864` over 500 ticks and is locked by a golden log.
