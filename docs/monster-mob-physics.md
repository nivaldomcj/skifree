# Monster & mob physics

Faithful port of the monster (yeti) and mob behavior, recovered from `ski32.exe`
(Ghidra 12.2 decompilation). This corrects and supersedes the looser claims in
`docs/monster-ai.md` where they conflict (see the notes at the bottom).

## Per-tick update order

Every moving thing is driven by two routines working on a small **proxy
struct** (not the 80-byte actor): `FUN_004041c0` then the behavior routine.

`FUN_004041c0` (integrator, runs for class 4–8 proxies):

```
x += vx;  y += vy;  z += vz;          // pure, no gravity here
```

Then the behavior routine runs. The monster's is `FUN_00404350`; the ambient
mob's is `FUN_00404290`. Afterward the proxy position/state is synced back to
the visible actor. The port models the monster/mob as a single actor but
applies the same per-tick math, which reproduces the observable behavior.

## Monster (`FUN_00404350`)

Proxy layout: `x=+0x14, y=+0x16, z=+0x18, vx=+0x1a, vy=+0x1c, vz=+0x1e,
class=+0xc, state=+0x10, timestamp=+0x20`.

Each tick, in order:

1. **Gravity.** If `z < 1`: `z = 0, vz = 0`. Else `vz -= 1`. If `z != 0` (mid-hop),
   the monster does nothing else this tick.
2. **Eating machine** (states `0x32–0x37`) — see below.
3. **Chase / hold / drift**, then pick the sprite.

### Guard regions, triggers, drift

Four monsters, one per boundary. Each has a *guard region* (its own position)
and a *player trigger*:

| Class | In region when | Chases when | Drift when out of region |
|-------|----------------|-------------|--------------------------|
| 5 (left) | `monster.y < -1999` | `player.y < -2000` | `vy = -10` |
| 6 (right) | `monster.y > 31999` | `player.y > 32000` | `vy = 26` |
| 7 (top) | `monster.x < -15999` | `player.x < -16000` | `vx = -16` |
| 8 (bottom) | `monster.x > 15999` | `player.x > 16000` | `vx = 16` |

- **In region + triggered** → chase.
- **In region + not triggered** → hold still (`vx = vy = 0`).
- **Out of region** → drift back toward it with the velocity above.

### The chase: rubber-band + clamped velocity

```
dx = player.x - monster.x
dy = player.y - monster.y
if dx >  screen_w: monster.x = player.x - screen_w
if dx < -screen_w: monster.x = player.x + screen_w
if dy >  screen_h: monster.y = player.y - screen_h
if dy < -screen_h: monster.y = player.y + screen_h
vx = clamp(dx, -16, +16)
vy = clamp(dy, -10, +26)
```

**`screen_w` / `screen_h` are the live window dimensions** (`DAT_0040c5f0 =
client width`, `DAT_0040c6d8 = client height`, recomputed on `WM_SIZE`). The
monster is kept within one screen of the player and teleported to exactly one
screen behind when outrun — so it never falls more than a screen away and can't
be escaped. (Earlier docs left these step values unspecified; they are the
window size.)

### Sprite states: 4 directions, 2-frame animation

After velocity is set, the pose is chosen by direction sign (not an 8-way
slope ratio as earlier docs said):

- `vy < 0` (up): toggle `0x30 ↔ 0x31`
- `vx < 0` (left): toggle `0x2c ↔ 0x2d`
- `vx == 0 && vy == 0` (arrived): **1-in-10 chance** → `vz = 4`, state `0x2b`
  (a hop, animated by gravity); otherwise idle `0x2a`
- else (down/right): toggle `0x2e ↔ 0x2f`

"Toggle" means `state = (current == base) ? base + 1 : base`, producing a
two-frame walk cycle.

### Eating (collision `FUN_00403a00` case 5–8)

When a monster actor touches the player, the skier actor is freed and the
monster enters state `0x32` with a timestamp. The eating animation is a
wall-clock state machine (runs even while "paused"):

- `0x32 → 0x33` immediately
- `0x33 → 0x34` after 500 ms
- `0x34 → 0x35` after 700 ms
- `0x35 → 0x36` after 1000 ms
- `0x36 → 0x37` immediately
- `0x37 → 0x2a` after 3000 ms

(The port already matched these timings; it is unchanged.)

## Mobs

### Dog (`FUN_00403540`, class 1)

Integrates, applies `DOG_PHYSICS[state - 0x16]`, and only processes states
`0x16–0x18` (returns early above `0x18`). 1-in-12 chance to re-randomize
direction: `rand(3)` → state `0x16 / 0x17 / 0x18`.

### Snowboarder (`FUN_00403610`, class 2)

- `0x1b`: `vy = rand(3) - 1`, integrate, → `0x1c`
- `0x1c`: `vx = 4`, integrate, → `0x1b`
- `0x1d`: `vx = vy = 0`, integrate, `rand(32)` → **`0x1e` if non-zero else `0x1b`**
- `0x1e`: `rand(100)` non-zero → integrate, → `0x1d`; else spawns a jump-effect
  actor (the port does not yet spawn this effect)

(The port previously had `vx`/`vy` swapped in `0x1b`/`0x1c` and the `0x1d`
transition inverted; both are fixed.)

### Obstacle / tree (`FUN_00403910`, class 3)

Integrates, applies `SNOWBOARDER_PHYSICS[state - 0x1f]`, then:
`0x1f →(rand 10) 0x20`, `0x20 →(rand 10) 0x1f`, `0x21 →(z==0) 0x20`,
`0x22–0x26 → +1` (wrapping `0x27 → 0x20`). Already faithful.

### Ambient mob (`FUN_00404290`, class 4)

A boundary-wrapped spawner that moves **vertically**:

- `y < -1023`: state `0x29`, `x = -144`, `vy = 2`
- `y > 23551`: state `0x27`, `x = -112`, `vy = -2`
- otherwise, 1-in-1000 chance to spawn a secondary actor (type 3, state `0x21`)
  and set state `0x28`

(The port previously used `vx`, wrong x positions, and a `-1024` threshold;
these are fixed. The original is a persistent *proxy* spawner — the port's
actor-based approximation does not yet spawn the secondary actor.)

### Lift chair (`FUN_004037b0`, class 10) and Eating (`FUN_00403750`, class 9)

Both already faithful: the lift idles/moves with `vx = ±1` and random
start/stop; eating loops `0x38 → 0x39 → 0x3a → 0x3b → 0x38`.

## Corrections to `docs/monster-ai.md`

- Sprite selection is **4-direction / 2-frame**, not 8-way slope ratio.
- The arrival "pause" is a **hop** (`vz = 4` + gravity), not a timed stop.
- Rubber-band step values are the **window width and height**, not unspecified
  constants.
- Ambient wrap thresholds are **-1023 and 23551**, and it uses `vy`.
