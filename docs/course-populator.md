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
`+0xe`) over all live actors — mob nodes without a live actor (off-screen
ambient mobs and monsters) don't count. Compared against `DAT_0040c748` =
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

## Collisions (`FUN_00403a00`)

The original handles each intersecting pair (at least one of them moved this
tick) twice: (later, earlier) then (earlier, later) in actor-list order, and
the *first* actor's class picks the behavior. The port mirrors that
(`collision::check_all_collisions` / `handle_collision`); the full rules,
verified call-for-call against the original, are in
`docs/vm-ground-truth.md` item 5. In short, with the skier first:

- **SnowBump**: skier in state 0 → `vz = 1`, state `0x0d`; `vy` halved if > 4.
- **Tree / Dog / Obstacle / Ambient / Lift / Eating** (solid path): flying
  over (or under) → **+6 style**. Otherwise nothing happens except on the
  **crossing tick** (the tick the skier's y reaches the obstacle's y, using
  both actors' start-of-tick y): a tree hit off-centre by more than half the
  wider sprite only halves `vy`; anything else downs the skier — grounded
  and still → crash `0x0b`, otherwise crash-landing `0x11` — for **−32
  style**. The **dead tree (sprite 50)** breaks instead (+16, class-9
  breaking animation `0x38`); hurdling the **eating monster** scores +1000
  and leaves a dead tree.
- **Rock (0xe)**: grounded → the solid path, where an uphill hit (`vy < 0`)
  on sprite 46 flips it flat (sprite 86) and downs the skier without the
  −32; airborne above its top → a flipped rock is collected (**+100**);
  airborne below its top, on the crossing tick → bounce (`vz = vy/2`, +1).
- **RockLarge (0xf)**: grounded → auto-hop (`vz = 4`, +1, state `0x0d`);
  airborne below its top → bounce; above it → nothing.
- **Banner (ramp, 0x10)**: on the crossing tick, below half its top and
  moving down → launched at its own speed (`vz = vy`, +1, state `0x0d`).
- **Snowboarder / Gate / Effect (classes 2/0x0c/0x11)**: on the crossing
  tick a graze halves `vy`; brushing the snowboarder's **jump puff (sprite
  0x52)** also costs **−16 style**.
- **Monsters**: nothing skier-first; the monster-first call eats the player.

Other first actors: a **dog** yelps/flees (`0x19`/`0x1a`, +20 if it was the
player), a **snowboarder** falls (`0x1d`, +3 if the player) when hit by
something moving, a **crossing skier** falls (`0x22`; +20 if the player) on
dogs/skiers/trees/rocks or jumps (`vz = vy/2`, state `0x21`) off large rocks
and the ramp, a **lift chair** stops (`vx = 0`, state `0x3c`).

## Culling and re-materialization

Culling happens inside the first loop of the tick, right after each actor's
update: an actor whose sprite box no longer overlaps the padded viewport
(client rect + 0x78) is removed, using the camera *as of that moment* (the
camera only follows the player when the player itself updates, so actors
ahead of it in the list see the pre-move view). The player is exempt. Mob
nodes (classes 4–8) are never removed — only their live actor is dropped.

Spawn events materialize after that loop (`FUN_004046e0`): the event's y must
be inside `[pad_top − 0x3c, pad_bottom + 0x3c)` and its sprite box (event
node z = 0) must overlap the padded viewport — so off-to-the-side events
such as the slalom start markers stay bare until scrolled into view. When a
culled actor came from a spawn event, the event's link is reset at the end
of the tick, so it can re-materialize from the next tick on (the original
does the same through the record's `+0x00` actor pointer and
`FUN_00404130`).
