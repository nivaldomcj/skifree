# Courses, timing, gates and the HUD

How SkiFree's three courses work and what the status display shows. Recovered
from `FUN_00404b50` (course generation), `FUN_00402c60` / `FUN_00403250` /
`FUN_00403180` (crossing detectors), `FUN_00401b80` (status painter) and
`FUN_004051e0` (title splash).

## The world at game start

`FUN_00404a80` creates the player at `(0,0)` in state 3 (standing), places
the title splash, and builds **all three courses at once** on separate x
strips:

| Course | Strip (world x) | Finish (y) | Gates | Scored by |
|--------|-----------------|-----------|-------|-----------|
| Slalom | [-0x240, -0x140] | 0x21c0 (540 m) | 22 flag gates | **time** |
| Tree slalom | [0x140, 0x200] | 0x4100 (4160 m) | gates (trees come from the populator) | **time** |
| Freestyle | [-0xa0, 0xa0] | 0x4100 (4160 m) | none | **style** |

Course identity is settled by the entrance banner sprites: 61 "Slalom ↙"
(left), 62 "Tree Slalom ↘" (right), 63 "Free style ↓" (center). Records:

- **Entrance banner** (class 0x11, sprites 61/62/63) near the spawn area;
  its x comes from a camera formula clamped into the strip's direction, its
  y is `client_h − client_h/3 − 0x3c` (clamped to 0x208 past 0x280) at the
  client size of world-build time. Course markers' event nodes carry z = 0;
  materialized they draw 16 units up, and the skier grazes the banner on the
  crossing tick.
- **Start posts** (sprites 57/58, "Start" arrows) at the strip edges, y=0x280.
- **Gate posts** (class 0x0c, sprites 0x17/0x18 alternating): slalom at
  x=-496/-400, y from 0x3c0 step 0x140; tree slalom at x=400/432, y from
  0x410 step 400. Each tree-slalom row also *prepares* a random tree record
  (`rand(8)` sprite, `rand(0x20)` x, a discarded `rand(400)`) but the
  original never adds it — the loop head overwrites the record with the next
  gate (see `docs/vm-ground-truth.md` item 2). The rolls still happen, so
  world-build RNG consumption is unchanged; the strip's trees come from the
  populator's zone-B picker.
- **Finish posts** (sprites 59/60) at the finish line.
- **Lift poles** (class 0xd, sprite 64) at x=-128 every 0x800 — the pole
  line the lift chairs ride.

## Course timing

Each detector runs after the skier update with the pre-update position:

- **Start**: crossing y=0x280 downward with x inside a strip activates that
  course. Slalom/tree slalom start the clock (`DAT_0040c948 = crossing
  tick`; the HUD clock `DAT_0040c944` is now − start). Freestyle (center)
  has **no clock** — it is scored on style points only.
- **Abandon**: moving back above y=0x280 deactivates the course.
- **Finish**: crossing the finish y stops the clock, records the time
  (slalom/tree slalom) or the style score (freestyle), and enters the
  top-10 high-score table (`FUN_00402e80` / `FUN_00402ec0`): the skier
  stands (state 3, unless crashing/airborne), and the result is inserted
  into the course's sorted list — `entpack.ini` section `Ski`, keys
  **SS** (slalom), **GS** (giant slalom), **FS** (freestyle style). The
  display marks your entry with ` <-- that's you!`, or shows it below the
  table with ` <-- try again!` when it doesn't place. The port persists
  the same three lists in `~/.skifree/scores.json`.
- **Gates**: each gate has a flag sprite — 0x17 means *pass on its right*,
  0x18 *pass on its left*. When the player crosses a gate's y, the flag
  turns sprite **0x19** (correct side) or **0x1a** (wrong side, plus a
  **5-second clock penalty**: `DAT_0040c948 -= 5000`, which can take the
  start time negative). The gate cursor (`DAT_0040c6f8`) advances through
  the course's gate records.

## Status display (FUN_00401b80)

Four stacked rows at the **top-right** of the window (the `SkiStatus`
child window), with exact printf formats from the string table:

- `Time: %2u:%2.2u:%2.2u.%2.2u` — the course clock (h:mm:ss.cc), 0 before
  any course starts.
- `Dist: %5.2dm` — inside a course: `(finish_y - y) / 16` (remaining);
  free-skiing: `y / 16` (traveled); no player: 0.
- `Speed: %5.2dm/s` — `(vy * 1000) / (frame_ms * 16)` — downhill speed,
  from **vy**, not vx.
- `Style: %7ld` — the style score, space-padded to 7.

## Title splash (FUN_004051e0)

Four inert effect actors (class 0x11, raw sprites) placed at game setup —
they are world objects you simply ski away from:

| Sprite | Content | Position |
|--------|---------|----------|
| 53 (93×57) | SkiFree logo incl. copyright/version block | (-40 - w/2, 0) |
| 54 (52×10) | divider line | (-40 - w/2, h53 + 4) |
| 55 (92×30) | "Use NumPad [0-9] for better control" sign | (max(w55, w56), 30) |
| 56 (63×32) | "F2 = Restart / F3 = Pause" sign | (max(w55, w56), 66) |

## Camera anchor

`FUN_00406060` positions the player at **(w/2, h/3)** of the client area
(recomputed on `WM_SIZE`), not at the center. All screen-edge logic
(populator, culling, banners) uses the viewport derived from that anchor.

## Style scoring gate (FUN_00403420)

Style points (`FUN_00403420(delta)` → `DAT_0040c6a8`) accumulate **only
while the freestyle course is active** (`DAT_0040c954 != 0`). Points earned
in freestyle stay on the display after the run ends, which is why the HUD
can show a nonzero Style while the Dist row reads "free-skiing". The port
mirrors this by gating `Scoring.add` on the freestyle course flag.

## Port notes

- The original interpolates the exact y=0x280 / finish crossing point
  (`FUN_00402e30`); the port uses the post-tick position.
- Gate pass/miss uses the post-tick player x rather than the interpolated
  crossing x.
- The 5 s gate-miss penalty keeps `start_ms` signed so the penalty applies
  even seconds after the start line (the original's `DAT_0040c948` is a
  plain signed int).
