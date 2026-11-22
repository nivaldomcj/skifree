# The monster (yeti) AI in `ski32.exe`

> **Note:** a few details below are superseded by the deeper analysis in
> `docs/monster-mob-physics.md` (4-direction sprites, the arrival *hop* rather
> than a pause, and the rubber-band step being the window dimensions). Read
> that doc for the authoritative physics; this one remains a good overview.

How the SkiFree monster works, from the Ghidra decompilation. Key functions: `0x404350.c` (monster brain), `0x404b50.c` (course generator), `0x403a00.c` (collisions), `0x4041c0.c` / `0x404290.c` (mover dispatchers), in `/tmp/ski32_decomp2/functions/`.

## Architecture: actors, not scripts

Everything that moves is an **actor** in a fixed pool of **100 slots** of 80 bytes each, built as a free list on new game (`FUN_00404a00`). Each actor has a **behavior class** (field at `+0xc`) that picks its "brain". The per-frame dispatcher `FUN_004041c0` integrates position (`x += vx`, `y += vy`, `z += vz`) and calls the behavior routine:

- class 4 → `FUN_00404290`: ambient mobs (dogs, snowboarders crossing the slope)
- **classes 5–8 → `FUN_00404350`: the monster** — four variants, one per approach direction

The course is a set of **spawn-event lists** generated at new-game time (`FUN_00404b50`), consumed by `FUN_004046e0` as the player scrolls past each event's y coordinate.

The RNG is the classic MSVC LCG: `seed = seed*0x343FD + 0x269EC3`, output `(seed>>16)&0x7FFF` (`FUN_00406cda`), seeded once per game from `GetTickCount()`. `FUN_004020b0(n)` = `rand() % n`.

## The four yetis: they're boundary guards

`FUN_00404b50` appends exactly **four** monster spawn events to the "mobs" list (`DAT_0040c720`). Each arms only past its boundary. Distances are in 1/16-meter units:

| Type | Spawn position | Arms when | Chases when |
|------|---------------|-----------|-------------|
| 5 (from left) | y = −2060 | monster.y < −1999 | player.y < −2000 (**~125 m back above the start**) |
| 6 (from right) | y = 32060 | monster.y > 31999 | player.y > 32000 (**the famous 2000 m yeti**) |
| 7 (from top) | x = −16060, y = 0 | always (dormant off-screen) | player.x < −16000 (**1000 m too far left**) |
| 8 (from bottom) | x = +16060, y = 0 | always (dormant off-screen) | player.x > 16000 (**1000 m too far right**) |

The "one" yeti everyone knows is just the bottom guard at 2000 m. Three more punish leaving the course upward or sideways — types 7/8 spawn at game start and sit invisibly off-screen until you stray ±1000 m in x.

## The chase: why it's inescapable

Once armed (`FUN_00404350`, label `LAB_00404539`), the monster doesn't really pursue — it **rubber-bands**:

```c
dx = player.x - monster.x;  dy = player.y - monster.y;
if (abs(dx) > step_x) monster.x = player.x - sign(dx) * step_x; // repositioned near you every frame
if (abs(dy) > step_y) monster.y = player.y - sign(dy) * step_y;
vx = clamp(dx, -16, +16);
vy = clamp(dy, -10, +26);
```

Every 40 ms tick it is teleported to exactly `step` away from your *current* position on each axis, then closes by the clamped velocity. Anchored to you rather than the world, it cannot be outrun. The clamped `vx/vy` ratio picks one of its 8 run-direction sprite frames (states `0x2a`–`0x31`, by comparing |dx| vs |dy| via a slope ratio).

When it arrives (|dx| < 1 and |dy| < 1), there's a **1-in-10 random chance** (`FUN_004020b0(10)`) it pauses in state `0x2b` for 4 frames — the little hesitation before the grab.

## Catching and eating you

Collision (`FUN_00403a00`, `case 5..8`) triggers only against the player pointer `DAT_0040c72c`:

1. Plays the roar (sound slot 7)
2. Removes your skier actor
3. Puts the monster into state `0x32`, timestamping the entry

The AI then runs the eating animation as a **wall-clock state machine** (that's why it plays even while the game is "paused"):

- `0x32 → 0x33` immediately (grab)
- `0x33 → 0x34` once ≥ 500 ms, else back to `0x32` (chewing loop)
- `0x34 → 0x35` after 700 ms
- `0x35 → 0x36` after 1000 ms
- `0x36 → 0x37` immediately
- `0x37 → 0x2a` once ≥ 3000 ms, else back to `0x36` (chewing loop) — back to chase state, ready for the next victim

## Ambient mobs

Class-4 actors (dogs etc., `FUN_00404290`) cross the slope horizontally, wrap at y ∈ [−1024, 23552], and each frame have a **1-in-1000 chance** to spawn a secondary actor (type 3, variant `0x21`) behind them.

## Limitations

Pseudocode is Ghidra decompiler output, not original source, but the constants and control flow above are directly observed in the binary.
