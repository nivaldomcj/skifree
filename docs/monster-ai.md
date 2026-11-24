# The monster (yeti) AI in `ski32.exe`

> **Note:** this is an overview. `docs/monster-mob-physics.md` has the full
> per-tick physics (gravity, the step hop, drift) and the other mobs.

How the SkiFree monster works, from the Ghidra decompilation. Key functions: `0x404350.c` (monster brain), `0x404b50.c` (course generator), `0x403a00.c` (collisions), `0x4041c0.c` / `0x404290.c` (mover dispatchers), in `/tmp/ski32_decomp2/functions/`.

## Architecture: actors, not scripts

Everything on the slope is an **actor** in a fixed pool of **100 slots** of 80 bytes each, built as a free list on new game (`FUN_00404a00`). Each actor has a **behavior class** (field at `+0x18`) that picks its "brain".

The chairlift chairs and the monsters are different: they are **mob nodes**, 0x24-byte records with their own class field at `+0xc`. A node has a live actor only while it is on screen. Every tick `FUN_004040a0` runs each node through the dispatcher `FUN_004041c0`, which integrates position (`x += vx`, `y += vy`, `z += vz`) and calls the behavior routine:

- class 4 → `FUN_00404290`: the chairlift chairs (riding the lift-pole line)
- **classes 5–8 → `FUN_00404350`: the monster** — four variants, one per approach direction

The course is a set of **spawn-event lists** generated at new-game time (`FUN_00404b50`), consumed by `FUN_004046e0` as the player scrolls past each event's y coordinate. The mob nodes are the exception: they exist from the first tick and update wherever the player is.

The RNG is the classic MSVC LCG: `seed = seed*0x343FD + 0x269EC3`, output `(seed>>16)&0x7FFF` (`FUN_00406cda`), seeded once per game from `GetTickCount()`. `FUN_004020b0(n)` = `rand() % n`.

## The four yetis: they're boundary guards

`FUN_00404b50` appends exactly **four** monster spawn events to the "mobs" list (`DAT_0040c720`). Each arms only inside its guard region, which it starts in. Distances are in 1/16-meter units:

| Type | Spawn position | Arms when | Chases when |
|------|---------------|-----------|-------------|
| 5 (guards uphill) | y = −2060 | monster.y < −1999 | player.y < −2000 (**~125 m back above the start**) |
| 6 (guards downhill) | y = 32060 | monster.y > 31999 | player.y > 32000 (**the famous 2000 m yeti**) |
| 7 (guards the left side) | x = −16060, y = 0 | monster.x < −15999 | player.x < −16000 (**1000 m too far left**) |
| 8 (guards the right side) | x = +16060, y = 0 | monster.x > 15999 | player.x > 16000 (**1000 m too far right**) |

The "one" yeti everyone knows is just the bottom guard at 2000 m. Three more punish leaving the course upward or sideways. All four exist from game start and wait off-screen in their regions until you cross a trigger. A guard outside its region drifts back toward it.

## The chase: why it's inescapable

Once armed (`FUN_00404350`, label `LAB_00404539`), the monster doesn't really pursue — it **rubber-bands**:

```c
dx = player.x - monster.x;  dy = player.y - monster.y;
if (abs(dx) > step_x) monster.x = player.x - sign(dx) * step_x; // pulled back to one step away
if (abs(dy) > step_y) monster.y = player.y - sign(dy) * step_y;
vx = clamp(dx, -16, +16);
vy = clamp(dy, -10, +26);
```

`step_x` / `step_y` are the window's client width and height. On each axis where it is farther than that, it is teleported to exactly one step from your *current* position, then closes by the clamped velocity. Anchored to you rather than the world, it cannot be outrun.

Each step also makes it hop (`vz = 1`). It is airborne for the next two ticks and skips its logic while airborne, so it steps, and plays its chase sound (slot 9), on every third tick and coasts at its last velocity in between.

The pose follows the direction, with a two-frame walk cycle: moving up (`vy < 0`) alternates `0x30`/`0x31`, left (`vx < 0`) `0x2c`/`0x2d`, down or right `0x2e`/`0x2f`.

Whenever it stands still on the ground (`vx == vy == 0`: holding its region with the player not past the trigger or no player at all, or exactly on the player), there's a **1-in-10 random chance** (`FUN_004020b0(10)`) it hops (`vz = 4`, state `0x2b`); otherwise it idles in `0x2a`. This roll runs on every grounded tick of a holding guard, not only on arrival, so all four guards draw from the RNG from tick 0.

## Catching and eating you

Collision (`FUN_00403a00`, `case 5..8`) triggers only against the player pointer `DAT_0040c72c`:

1. Plays the roar (sound slot 7)
2. Removes your skier actor
3. Puts the monster into state `0x32`, timestamping the entry, and stops its `vx`/`vy`

The AI then runs the eating animation as a **wall-clock state machine**. Nothing ticks while the game is paused, but the wall clock keeps running, so after a long pause the remaining frames go by in a few ticks:

- `0x32 → 0x33` immediately (grab)
- `0x33 → 0x34` once ≥ 500 ms, else back to `0x32` (chewing loop)
- `0x34 → 0x35` after 700 ms
- `0x35 → 0x36` after 1000 ms
- `0x36 → 0x37` immediately
- `0x37 → 0x2a` once ≥ 3000 ms, else back to `0x36` (chewing loop)

`0x2a` is the idle pose. With the player gone there is no one to chase, so the monster holds its region (or drifts back to it) until a new game.

## Chairlift chairs

Class-4 mob nodes (`FUN_00404290`) are the chairlift chairs (sprites 65–67: two riders, one rider, empty). They ride the lift-pole line, up at x = −112 and down at x = −144, wrap when y < −1023 or y > 23551, and while going up (state `0x27`) and on screen have a **1-in-1000 chance** per tick to drop a rider: a class-3 skier in state `0x21`, at the chair's own position and z = `0x20`, so it falls off the chair.

## Limitations

Pseudocode is Ghidra decompiler output, not original source, but the constants and control flow above are directly observed in the binary.
