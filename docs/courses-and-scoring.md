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

- **Start**: crossing y=0x280 downward activates the course whose strip
  contains the **crossing x** — interpolated between the pre- and
  post-update positions (`FUN_00402e30`). Slalom/tree slalom start the
  clock at the interpolated **crossing time** between the previous tick and
  this one (`DAT_0040c948`); the HUD clock `DAT_0040c944` is updated to
  now − start at the start of every check (on the start tick itself it is
  start − now, slightly negative). Freestyle (center, `FUN_00403180`) has
  **no clock** — it is scored on style points only. The flags are
  `DAT_0040c95c` (slalom, `FUN_00402c60`), `DAT_0040c958` (tree slalom,
  `FUN_00403250`) and `DAT_0040c954` (freestyle).
- **Abandon**: moving back above y=0x280 deactivates the course.
- **Finish**: crossing the finish y stops the clock at the interpolated
  crossing time, records the time (slalom/tree slalom) or the style score
  (freestyle), and enters the
  top-10 high-score table (`FUN_00402e80` / `FUN_00402ec0`): the skier
  stands (state 3, unless crashing/airborne), and the result is inserted
  into the course's sorted list — `entpack.ini` section `Ski`, keys
  **SS** (slalom), **GS** (giant slalom), **FS** (freestyle style). The
  display marks your entry with ` <-- that's you!`, or shows it below the
  table with ` <-- try again!` when it doesn't place. The port persists
  the same three lists in `~/.skifree/scores.json`. (The original shows the
  table in a modal message box.)
- **Gates**: each gate has a flag sprite. When the player's y passes the
  next gate's y, the crossing x (interpolated at the gate's y) decides:
  sprite **0x17** is missed when that x is **right of** the gate's x
  (`x > gate.x`), sprite **0x18** when it is **left of** it (`x < gate.x`)
  (`0x403350`). The flag turns sprite **0x19** (passed) or **0x1a**
  (missed, plus a **5-second clock penalty**: `DAT_0040c948 -= 5000`,
  which can take the start time negative; it shows on the clock from the
  next tick, since the clock is refreshed before the gate check). One gate
  is checked per tick; the gate cursor (`DAT_0040c6f8`) then advances.

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
| 54 (52×10) | divider line | (-40 - w53/2, h54 + 4) — its own height (verified: (−86, 14)) |
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

Sources of style points: tricks held in the air (+2/+4/+8 per tick by pose,
see `docs/skier-physics.md`), a botched landing (−64), hopping bumps and
rocks (+1), flying over obstacles (+6, +1000 over the eating yeti), breaking
the dead tree (+16), collecting a flipped rock (+100), knocking into a dog
(+20), snowboarder (+3) or crossing skier (+20), and crashes (−32); the
snowboarder's jump puff costs −16.

## Pause

F3 and **losing focus or minimizing the window** both stop the game timer:
nothing moves. When play resumes, the original pushes the course start
reference forward by the paused time, so **course times exclude pauses** —
but its wall clock keeps running, so a yeti that is eating you finishes its
meal during the pause. The port matches both: its game clock (course
timing) freezes while paused and a separate wall clock drives the meal.

The two pauses differ (window procedure + `FUN_00405a10`):

- **F3** (`FUN_00405760`) shows "Ski Paused ... Press F3 to continue" and
  only another F3 (or F2) resumes.
- **Focus loss / minimize** (`WM_ACTIVATE` / `WM_SIZE`) stops the timer
  silently and **ignores every key, mouse move and click** until the window
  is active again; regaining focus resumes play — unless F3 had paused it,
  which survives the focus change. The click that re-activates the window
  is swallowed (`WM_MOUSEACTIVATE` → `MA_ACTIVATEANDEAT`), so clicking back
  into the game doesn't make the skier jump.
- The **T** cheat steps one tick even while paused (it calls the timer
  callback directly).

The port reproduces all of this: miniquad's focus events (it reports focus
loss as "minimized" on Windows, macOS and X11) drive
`GameState::set_active`, which mirrors `FUN_00405a10`.

## Port notes

- Start, finish and gate checks use the original's interpolated crossing x
  and crossing time (`course::interpolate` = `FUN_00402e30`), run at the end
  of the player's update like the original, and the gate rule follows
  `0x403350` (an earlier port had 0x17/0x18 the wrong way round and used the
  post-tick x and time).
- The 5 s gate-miss penalty keeps `start_ms` signed so the penalty applies
  even seconds after the start line (the original's `DAT_0040c948` is a
  plain signed int).
