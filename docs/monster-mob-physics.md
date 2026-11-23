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

### The step hop

Whenever the monster moves (chasing or drifting back to its region), the
shared tail (`0x4045d8`–`0x40461d`) sets **`vz = 1`**. It rescales vx or vy
first, but both are overwritten right after, so only the hop survives. The
monster is then airborne (z = 1) for two ticks, and an airborne monster skips
its whole logic (step 1 above). So it only steers, rubber-bands and plays its
chase sound (`docs/how-ski32-works.md`, "Sound") on every third tick. In
between, it coasts at its last velocity. (Found in Frida while tracing sounds,
2026-11-23: in `compare_monster` the chase at tick 803 is followed by z = 1 at
ticks 804 and 805.)

### Eating (collision `FUN_00403a00` case 5–8)

When a monster actor touches the player, the skier actor is freed and the
monster enters state `0x32` with a timestamp. The eating animation is a
wall-clock state machine (runs even while "paused"):

- `0x32 → 0x33` immediately
- `0x33 → 0x34` once ≥ 500 ms have passed, **else back to `0x32`** (chewing)
- `0x34 → 0x35` after 700 ms
- `0x35 → 0x36` after 1000 ms
- `0x36 → 0x37` immediately
- `0x37 → 0x2a` once ≥ 3000 ms have passed, **else back to `0x36`** (chewing)

(Switch at `0x4043be`; verified via Frida with a fixed 40 ms/tick clock —
see `docs/vm-ground-truth.md` item 11. The port originally walked straight
through `0x32–0x37` without the two chewing loops.)

## Mobs

### Dog (`FUN_00403540`, class 1)

Integrates, applies `DOG_PHYSICS[state - 0x16]`, and only processes states
`0x16–0x18` (returns early above `0x18`). 1-in-12 chance to re-randomize
direction: `rand(3)` → state `0x16 / 0x17 / 0x18`.

### Snowboarder (`FUN_00403610`, class 2)

- `0x1b`: `vy = rand(3) - 1`, integrate, → `0x1c`
- `0x1c`: `vx = 4`, integrate, → `0x1b`
- `0x1d`: `vx = vy = 0`, integrate, `rand(32)` → **`0x1e` if non-zero else `0x1b`**
- `0x1e`: `rand(100)` non-zero → integrate, → `0x1d`; else (1-in-100) **jump**:
  spawn an effect actor (see below) at `(x-4, y-2, z)` from the
  pre-integration position, fire a sound (`FUN_00402ba0` — silent, the WAVE
  resource is absent from the shipped binary), then integrate and → `0x1b`.

The jump effect is a **class `0x11` actor with raw sprite `0x52`** (16×8,
created via `FUN_004026a0`, which bypasses the state→sprite table). Classes
≥ `0xb` get no behavior update (`FUN_00401e50` only dispatches classes
< `0xb`), so the puff just sits where it spawned until it scrolls out and is
culled. (The port previously stayed in `0x1e` after a jump and never spawned
the effect; both are fixed.)

### Secondary-actor spawns

Two behavior routines create actors mid-tick. Both allocate from the actor
pool, then place the actor with `FUN_00402390` (a full pool returns NULL and
the spawn is dropped):

| Source | Creator | Class | State/Sprite | Position |
|--------|---------|-------|--------------|----------|
| Snowboarder `0x1e` jump | `FUN_004026a0` | `0x11` (effect) | sprite `0x52` | `(x-4, y-2, z)` pre-integration |
| Ambient mob `0x27` (1-in-1000) | `FUN_004020d0` | 3 (obstacle) | state `0x21` | mob's `(x, y, z)` post-integration |
| Main tick tail (1-in-666) | `FUN_004020d0` + `FUN_00402350` | 3 (obstacle) | state `0x1f` | random x across window, `viewport_top - 0x3c`, z 0 |

The ambient-spawned class-3 actor inherits the mob's **z = 0x20**, so it
visibly drops in from above; state `0x21` means "airborne" and lands to
`0x20`. The 1-in-666 spawn enters above the top edge in state `0x1f`
(down-left, max vy 18 > the player's 16), so it overtakes the player from
behind — that is where the crossing skiers come from. `FUN_00402350`
direction 2's position is `camera + (rand(client_w), -0x3c)`; the Win32
window-frame offsets it subtracts (`DAT_0040c704`/`DAT_0040c5fc`) have no
equivalent in the port's camera model and are 0. (The port previously spawned
the wrong class — ambient mob — at `(0,0,0)`; fixed.)

### Crossing skier (`FUN_00403910`, class 3)

Integrates, applies `CROSSING_SKIER_PHYSICS[state - 0x1f]`, then:
`0x1f →(rand 10) 0x20`, `0x20 →(rand 10) 0x1f`, `0x21 →(z==0) 0x20`,
`0x22–0x26 → +1` (wrapping `0x27 → 0x20`). Already faithful.

### Ambient mob (`FUN_00404290`, class 4)

A boundary-wrapped spawner that drifts **vertically** just left of the slope
center. Course generation (`FUN_00404b50`) seeds pairs of these proxies every
`0x800` units of y: state `0x27` at `x = -112, vy = -2` (drifting up) and
state `0x29` at `x = -144, vy = +2` (drifting down), all floating at
**z = `0x20`** (proxy integration has no gravity, so they never land).

- `y < -1023`: state `0x29`, `x = -144`, `vy = 2`
- `y > 23551`: state `0x27`, `x = -112`, `vy = -2`
- otherwise, in state `0x27`, 1-in-1000 chance to drop a crossing skier
  (class 3, state `0x21`) at the mob's position — z = `0x20` included, so the
  skier falls in from above — and set state `0x28`

The original gates the roll (and so the spawn) on the proxy having a live
actor (`*proxy != 0`): the node got one when its point `(x, y − z)` entered
the padded viewport, and keeps it until the tick after its sprite box is
culled. The port models this with `flags.on_screen` + `proxy_unlink` on the
merged actor (see `docs/vm-ground-truth.md` item 7); proxies are updated in
their own pass, in node order, after the actor loop. Rows start at y = ±1024
+ 0x800·k (verified via Frida). The spawn itself is implemented (see
"Secondary-actor spawns").

### Lift chair (`FUN_004037b0`, class 10) and Eating (`FUN_00403750`, class 9)

Both already faithful: the lift idles/moves with `vx = ±1` and random
start/stop; eating loops `0x38 → 0x39 → 0x3a → 0x3b → 0x38`.

## Corrections to `docs/monster-ai.md`

- Sprite selection is **4-direction / 2-frame**, not 8-way slope ratio.
- The arrival "pause" is a **hop** (`vz = 4` + gravity), not a timed stop.
- Rubber-band step values are the **window width and height**, not unspecified
  constants.
- Ambient wrap thresholds are **-1023 and 23551**, and it uses `vy`.
