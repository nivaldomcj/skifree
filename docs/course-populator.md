# Course populator (dynamic obstacle spawning)

How the original fills the slope with trees, rocks, dogs, snowboarders, lifts,
snow bumps and rainbow ramps — and how the port reproduces it. Ported from
`FUN_004025c0` and friends (Ghidra 12.2 decompilation).

## The edge spawner

The main tick (`FUN_00401e50`) accumulates camera travel and, for every
`0x3c` (60 px) crossed, calls `FUN_004025c0(direction)` once — order: bottom,
top, right, left. Each call:

1. Picks a random position along the entering edge (`FUN_004024f0`): random x
   (top/bottom edges) or y (left/right edges) across the client area, 0x3c
   beyond the edge. z = 0.
2. Picks a class by **zone test on the spawn position**, then a per-zone
   probability table.
3. Skips the spawn entirely (`0x12`) when the **density cap** is hit.
4. Creates the actor: classes < 0xb with an initial state from
   `DAT_0040a22c`, classes ≥ 0xb with a raw sprite from `FUN_00402850`.

## Zones

| Zone | x range | y range | Picker | Content |
|------|---------|---------|--------|---------|
| A (slalom strip) | [-0x240, -0x140] | [0x280, 0x21c0] | `FUN_00402770` | 100% snow bumps |
| B (tree-slalom strip) | [0x140, 0x200] | [0x280, 0x4100] | `FUN_004027a0` | 63/64 trees, 1/64 snowboarder |
| C (freestyle strip) | [-0xa0, 0xa0] | [0x280, 0x4100] | `FUN_004027e0` | 18% tree, 30% large rock, 10% bump, 20% rock, 20% banner, 2% lift |
| default | everything else | | `FUN_004026f0` | 45% tree, 20% large rock, 5% bump, 20% rock, 2% banner, 5% lift, 2% dog, 1% snowboarder |

(The course strips are named after the banner sprites at each course's
entrance: 61 "Slalom ↙" left, 62 "Tree Slalom ↘" right, 63 "Free style ↓"
center. Earlier docs had slalom and freestyle swapped — the sprite contents
settle it. See `docs/courses-and-scoring.md`.)

## Density cap

`DAT_0040c6fc` = sum of **sprite areas** (width × height, sprite-table entry
`+0xe`) over all live actors. Compared against `DAT_0040c748` =
`(client_w + 0xf0) × (client_h + 0xf0)` (the viewport padded by 0x78):

- Zone A: cap `area / 64`
- Zone B: cap `area / 16`
- Zone C: cap `area / 32`
- Default: cap `area / 32`

The port computes the sum on demand per spawn (≤100 actors) instead of
maintaining the global incrementally.

## Scenery classes (11–16) and their sprites

Classes ≥ 0xb get no behavior update — they're static scenery, culled when
they scroll out of the padded viewport.

| Class | Name | Sprites (`FUN_00402850`) |
|-------|------|--------------------------|
| 0xb (11) | SnowBump | 27 (64×32 snow mound) |
| 0xd (13) | Tree | 49 small pine (6/8), 50 dead tree (1/8), 51 big pine (1/8) |
| 0xe (14) | Rock | 45 or 46 (1/4) |
| 0xf (15) | RockLarge | 47 or 48 (1/3) |
| 0x10 (16) | Banner | 52 (32×8 rainbow freestyle ramp) |

## Scenery collisions (`FUN_00403a00` case 0)

- **SnowBump**: skier in state 0 → `vz = 1`, state `0x0d`; `vy` halved if > 4.
- **Tree / Dog / Obstacle / Ambient / Lift / Eating**: z-overlap test —
  flying over (or under) → **+6 style**, no crash. Otherwise grounded → crash
  (`0x0b`), airborne → crash-landing (`0x11`), **−32 style**. Hitting the
  **dead tree (sprite 50)** airborne breaks it (+16, it becomes a class-9
  breaking animation in state `0x38`).
- **Rock**: airborne above → sail over (a rock previously flipped to sprite
  86 is collected for **+100**); airborne level → bounce (`vz = vy/2`, +1);
  grounded → crash −32 — **except** uphill hits (`vy < 0`) on sprite 46,
  which flip the rock flat (sprite 86) with no penalty.
- **RockLarge**: grounded → auto-hop (`vz = 4`, +1, state `0x0d`); airborne →
  bounce.
- **Banner (ramp)**: skier below half its height and moving down → launched
  at its own speed (`vz = vy`, +1, state `0x0d`).
- **Snowboarder / Effect**: brush past — `vy` halved, no crash; skiing
  through a jump puff (sprite 0x52) costs **−16 style**.

### Deliberate simplifications

- The original's `bVar4` (actors moving apart) gates a few graze/bounce
  nuances (tree graze halving `vy`, mid-air rock bounce, ramp trigger); the
  port applies the dominant branch and drops the `bVar4` condition.
- The original runs the collision switch for both pair orders (so the dog's
  own side — +20 and fleeing states — also fires); the port resolves only the
  player side.
- Hitting an Eating actor mid-flight converts it to a dead tree for +1000
  (original quirk) — ported as-is.

## Culling and re-materialization

Every tick, actors fully outside the padded viewport (client rect + 0x78) are
freed, except the player and the proxy classes (4–8: ambient mobs, monsters —
their original proxies keep running off-screen). When a culled actor came
from a spawn event, the event's link is reset so the actor re-materializes
when scrolled back into range (the original does the same through the
record's `+0x00` actor pointer and `FUN_00404130`).
