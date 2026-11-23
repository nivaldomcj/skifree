# Ground truth from the original: Frida on Windows 11 ARM (VM)

How we verify the port against the original game itself, and what we learned.
The port is checked **call for call** against a fresh `ski32-autoplay.exe`:
every `rand()` call (stream position and modulus), the player's full state,
the style score and the course clock on every tick must be identical.
Fourteen scenarios — keyboard and mouse steering, freestyle tricks, all three
courses (timed runs included), a monster encounter, F2 restarts, window
focus changes, three window sizes and a long run to the 2000 m yeti — pass
this test and are locked in as golden replays (`tests/replay.rs`,
`verified_*`).

## Setup

- Windows 11 ARM64 VM (VMware Fusion) with `ski32-seeded.exe` and
  `ski32-autoplay.exe` in `C:\Users\<you>\Downloads\`:
  - `ski32-seeded`: patched `srand` to force seed 0 (`mov eax,[esp+4]` →
    `xor eax,eax; nop; nop` at module offset `0x6cd0`).
  - `ski32-autoplay`: same seed patch + start-state patch (state 3 → 0 at
    offset `0x4a85`), so it starts skiing immediately — no title screen.
- `frida-server-x86.exe -l 0.0.0.0:27042` (32-bit build, matches the x86
  game). Runs as scheduled task `FridaServer` (SSH-spawned processes get
  job-killed when the session closes; scheduled tasks survive).
- SSH key auth into the VM, used only to kill leftover game processes.
- frida-tools (pipx) on the Mac connects to `<vm-ip>:27042`. Frida's
  x86-under-Prism instrumentation works — a key result for future
  DX6-era ports. Under Frida the game runs at ~19 ticks/s instead of 25.

## Key addresses (module base is always 0x400000, no ASLR)

| Address | Meaning |
|---------|---------|
| `0x0040c16c` | MSVC LCG state (`rand` is statically linked at `0x406cda`, `srand` at `0x406cd0`) |
| `0x00401000` | timer callback: `GetTickCount` → `DAT_0040c698`, then the main tick |
| `0x00401e50` | main tick (call N = the port's replay tick N) |
| `0x00402be0` | physics integrator (`__thiscall`, ecx = actor) |
| `0x004028e0` | per-actor updater (classes < 0xb, first tick loop) |
| `0x00403a00` | collision handler for one ordered pair (ecx = first, edx = second) |
| `0x00402390` | position setter (sets flag 0x20 "moved"; moves the camera for the player) |
| `0x004020d0` | create actor (ecx=class, edx=state) → eax |
| `0x004020b0` | `rand() % n` wrapper (ecx = n) — hook it to get every caller |
| `0x00404b50` | world build (course events, mob nodes) |
| `0x004040a0` / `0x004046e0` | mob-node pass / course spawn-event scan |
| `0x00406170` | keyboard handler (ecx = virtual-key code) |
| `0x004066d0` | mouse-click (jump) handler, no arguments |
| `0x00406550` | mouse-move handler (ecx = x, edx = y, client coordinates) |
| `0x00403420` | add style points (ecx) to `DAT_0040c6a8` when `DAT_0040c954` (freestyle running) |
| `0x00404970` | new game (re-seeds, resets camera/scroll/score/course globals) |
| `0x004057c0` / `0x00404ad0` | pause (kill timer) / resume (restart timer, shift course clock) |
| `0x0040c618` | actor list head (newest first; next pointer at `+0`) |
| `0x0040c72c` | player actor |
| `0x0040c6c8` | main window handle |
| `0x0040c720` | mob-node list (`[+0]` begin, `[+4]` end, 0x24-byte nodes) |

Actor struct (0x50 bytes): `+0x04` previous-position copy (see item 5),
`+0x0c` mob-node link, `+0x10` sprite, `+0x14` sprite-table entry,
`+0x18` class, `+0x1c` state, `+0x20`/`+0x30` cached screen AABBs,
`+0x40/42/44` x/y/z, `+0x46/48/4a` vx/vy/vz (all s16), `+0x4c` flags
(`0x08` being removed, `0x20` moved this tick).

Sprite-table entry (16 bytes): `+0x08` x offset in the source strip,
`+0x0a` width, `+0x0c` height, `+0x0e` area.

## Verified facts

1. **RNG**: the game uses the MSVC LCG statically linked; patching `srand`
   to seed 0 makes the entire stream reproducible. Any observed
   `DAT_0040c16c` value maps to an exact stream index — the universal
   alignment key between runs and between original and port.
2. **World build consumes exactly 117 rand calls** (39 cycles of 3:
   24 slalom rows + 15 tree-slalom rows). F2 re-seeds (srand) and rebuilds,
   but **an F2-restarted world is NOT identical to a fresh-process world**
   (populator accumulators carry over) — always compare fresh processes.
   The tree-slalom rows *prepare* a tree record (class 0xd, sprite via
   `FUN_00402850` → rand(8), x = 400 + rand(32), then a discarded rand(400))
   **but never add it**: the loop head overwrites the record with the next
   gate before calling `FUN_00405120` (`0x404df8..0x404e32`). The strip's
   trees come from the populator's zone B only. The port used to add them.
3. **The world is built at the client size of that moment.** The banner y
   is `client_h − client_h/3 − 0x3c` (clamped), and other layout depends
   on the size too. The window is created at a size derived from the screen
   (768×768 outer; the client size varies between launches), so comparisons
   must force the client size *before* the build (see "Harness"). An
   earlier "+32 title bar" correction in the port only compensated for runs
   whose world had been built at a different size than the one played at.
4. **Course markers** (banners, Start/Finish posts) carry z = 0, both in
   their spawn-event nodes and as live actors (read from the actor list).
   The port once gave them z = 0x10, which drew them 16 px too high and
   culled them one tick early; nothing checked against the original noticed,
   because `actor_count` is a port-only field. Found by the pixel
   comparison (item 20): the original culls the freestyle banner of
   `compare_vm` in main-tick call 58, when its box bottom (−135) leaves the
   padded view (−120); at call 57 it is −119, still inside.
5. **Collision pass** (`FUN_00401e50` → `FUN_00403a00`):
   - **Pairs**: for each actor `j` in list order, against each actor `i`
     *before* it (the inner loop stops at the outer actor, so every
     unordered pair is visited once); the pair is tested only if at least
     one of them moved this tick (flag 0x20) and the cached boxes intersect.
     The handler then runs for **(later, earlier)** and, unless either got
     removed (flag 0x08), **(earlier, later)**.
   - **Outer switch** by the *first* actor's class: 0 skier; 1 dog (state
     < 0x19 → 0x19, or 0x1a if the other is airborne; +20 style if it was the
     player); 2 snowboarder (state < 0x1d and the other is moving → 0x1d;
     +3 if the player); 3 crossing skier (by the other's class via the byte
     table at `0x404054`: player → +20 then as below; dog/skier/tree/rock →
     below the other's top falls to 0x22; large rock/ramp → below the top
     jumps, vz = vy/2, state 0x21); 5–8 monster vs the player → eat; 10 lift
     → vx = 0, state 0x3c; 4, 9 and ≥ 0xb → nothing.
   - **Crossing flag** (`local_4`): compares both actors' current y with
     their previous y — `FUN_00404070` returns the previous-position copy at
     `[actor+4]` (the position at the start of the tick; the paint shadow),
     or the actor itself. Set when their vertical order flipped or they met,
     i.e. exactly one tick per crossing. Using `y − vy` as "previous y" is
     wrong whenever vy changed during the tick (e.g. getting up after a
     crash next to the obstacle that caused it).
   - **Skier first** (inner switch by the other's class − 1): graze
     (cases 2/12/17, crossing tick only: halve vy; sprite 0x52 → −16);
     solid obstacles (1/3/4/9/10/13, and 14 when grounded): fly over/under
     → +6 (+1000 over the eating monster, which leaves a dead tree); else
     crossing tick only: a tree hit off-centre by more than half the wider
     sprite halves vy; otherwise down — state 0xb if grounded and still,
     else 0x11 (the dead tree 0x32 breaks for +16 instead); an uphill hit
     (vy < 0) on the small rock 0x2e flips it to 0x56 and still downs the
     skier but without the −32; any other hit −32. Rock 0xe airborne: above
     its top a flipped rock is collected (+100); below its top, on the
     crossing tick, bounce (vz = vy/2, +1). Large rock 0xf: grounded → hop
     (vz 4, state 0xd, +1); airborne below its top → bounce; above → nothing.
     Ramp 0x10: crossing tick, z < top/2, vy > 0 → vz = vy, state 0xd, +1.
     Monsters (5–8): nothing (the eating runs monster-first).
6. **Sprite sizes**: the sprite table's `+0x0a/+0x0c` are the bitmap's
   width/height; skier sprites 1–6 are 16×32 / 24×28. (The port once read
   the 32×32 *icon* resources 1–6 over them — only `RT_BITMAP` counts.)
   Entry 0 is an empty 0×0 sprite, used by every mob node.
7. **Mob nodes vs actors**: class-4 mobs and the four monsters (classes
   5–8) are nodes (0x24-byte records) updated globally every tick by
   `FUN_004040a0` (`FUN_004041c0`, then `FUN_00404130`), wherever the
   player is. A node gets a live actor when its point `(x, y−z)` lies
   strictly inside the padded viewport (its sprite is entry 0, so the box
   test degenerates to a point test). The live actor is culled by its real
   sprite box in the next tick's first loop, but the node keeps its actor
   link until the end of that tick — so the node still acts as live for its
   update in the tick it is culled. The class-4 1-in-1000 roll
   (`0x40430a`) happens only while the node has a live actor. Class-4 rows
   start at y = ±1024 + 2048k (up-drifters at +1024…, down-drifters from
   −1024), drifting 2/tick from world build. Nodes without a live actor are
   not in the actor list, so they never collide.
8. **Populator density** `DAT_0040c6fc` is a running sum of the sprite areas
   (`+0x0e`) of live actors only: off-screen mob nodes contribute nothing.
   The cap compares against `DAT_0040c748 = (w+0xf0)(h+0xf0)`.
9. **Tick order** (`FUN_00401e50`): (1) for each actor in list order
   (newest first, player last): update (classes < 0xb, not node-owned),
   then cull-test against the padded viewport `DAT_0040c680` (client rect
   + 0x78) using the camera *as of that moment* — the camera
   (`DAT_0040c640/c5f2`) follows the player inside `FUN_00402390` when the
   player moves, so actors ahead of it see the pre-move view; (2) the four
   course spawn-event lists (`FUN_004046e0`: y window `[pad_top−0x3c,
   pad_bottom+0x3c)`, then `FUN_00404130`'s box test with the node's own
   sprite at z = 0); (3) mob nodes (`FUN_004040a0`); (4) collisions (item
   5); (5) scroll accumulators → populator edge spawns (`FUN_004025c0`);
   (6) the 1-in-666 crossing-skier roll, whose x is `rand_range(screen_w)`
   inside `FUN_004024f0` direction 2. Removed actors (flag 0x08) are freed
   after the tick — `DAT_0040c72c` still points at a just-eaten player until
   then.
10. **Crash (state 0xb)**: `FUN_004028e0` steps vx/vy one unit toward zero
    per tick *without integrating* (the skier stays put), then switches to
    state 0xc once both are zero.
11. **Monster meal** (`FUN_00404350`, switch at `0x4043be`, elapsed =
    `DAT_0040c698 − node+0x20`): 0x32 → 0x33; 0x33 → 0x34 once ≥ 500 ms,
    else back to 0x32 (chewing); 0x34 → 0x35 after 700 ms; 0x35 → 0x36
    after 1000 ms; 0x36 → 0x37; 0x37 → 0x2a (running again) once ≥ 3000 ms,
    else back to 0x36 (chewing). The port used to walk straight through
    0x32–0x37.
12. **Inputs**: the window procedure calls `FUN_00406170(vk)` for
    WM_KEYDOWN and `FUN_004066d0()` for a left click; both act immediately
    on the player actor. Calling them at the start of a main-tick call is
    exactly the port's "input at tick N, then `tick()`".
13. **Time**: `GetTickCount` is read only by the timer callback (before
    every main tick), new game and timer setup. Everything wall-clock based
    (monster meal, course timers) derives from `DAT_0040c698`.
14. **Trick scoring** (`FUN_004028e0`, after the state update): style points
    by the skier's *new* state every tick (byte table at `0x402b88`): 0x10
    → +2, 0x12/0x13 → +4, 0x14/0x15 → +8, 7–10 → −1 (unreachable there:
    the same update already turns 7–10 into 3/6). A landing whose
    `CRASH_LANDING` result is 0x11 costs −64. Points only count while the
    freestyle course runs (`DAT_0040c954`). The port had no per-tick trick
    scoring.
15. **Mouse** (`FUN_00406550(x, y)`): the very first mouse message only
    records the position (`DAT_0040c760`), a message at the unchanged
    position is ignored, and nothing happens while the skier is in 0xb/0x11.
    The offset is from the skier's anchor (`x − w/2`, `y − h/3`). On the
    ground `FUN_004065e0` picks a direction 0–6 from `dy·4/dx` (a ratio
    truncated to 0 goes by the sign of dx: 6 or 3 — the port returned 3);
    in the air `FUN_00406670` picks a jump pose 0xd–0x10 by quadrant and the
    dominant axis (the port used the ground picker in the air too).
16. **F2 restart** (key handler → `FUN_00406500`): new game
    (`FUN_00404970`: **re-seeds** with `GetTickCount`, resets camera, both
    scroll accumulators, score, course globals), unpause if paused, then
    `FUN_00404a80` builds the world and spawns the skier — the same as a
    fresh launch apart from creating the window. The port did not re-seed
    on a new game. (The note that an F2 world differs from a fresh one came
    from posting F2 at an arbitrary moment mid-tick.)
17. **Pause** (F3 → `FUN_00405760`; also on focus loss / minimize via the
    activation handler `FUN_00405a10`): `FUN_004057c0` kills the timer —
    no ticks at all — and remembers the last tick time; `FUN_00404ad0`
    resumes, sets `DAT_0040c698` to now and, while a course runs, pushes
    the course start reference (`DAT_0040c948`) forward by the paused
    time. So course times exclude pauses, but wall-clock logic does not:
    the monster's meal keeps progressing while paused. The port froze its
    only clock; it now keeps a separate wall clock for the meal.
    Activation (window procedure): `WM_SIZE` stores "minimized"
    (`DAT_0040c770`), `WM_ACTIVATE` stores "active" (`DAT_0040c694`, then
    `SetFocus`), and both call `FUN_00405a10`: active and not minimized →
    input enabled (`DAT_0040c67c` = 1) and `FUN_00404ad0` (which never
    overrides an F3 pause, `DAT_0040c650`); otherwise input disabled and
    `FUN_004057c0`. With input disabled `WM_KEYDOWN`, `WM_CHAR`,
    `WM_MOUSEMOVE` and `WM_LBUTTONDOWN` are dropped. `WM_MOUSEACTIVATE`
    returns `MA_ACTIVATEANDEAT` for client clicks (the activating click is
    eaten); `WM_GETMINMAXINFO` sets the 320×300 minimum. The port mirrors
    this in `GameState::set_active` / `toggle_pause` / `timer_running`.
18. **Course finish**: `FUN_00402e80` stands the skier up (state 3 on the
    ground, 0xe airborne, unless 0xb/0x11), then `FUN_00402ec0` updates the
    high scores in `entpack.ini` and shows a **modal `MessageBoxA`**, which
    blocks the game thread (and a capture) until dismissed. Course time is
    `DAT_0040c944 = now − DAT_0040c948`; a missed gate subtracts 5000 from
    the start reference (+5 s).
19. **Mob-node actors carry no velocity**: `FUN_004041c0` moves the node by
    the node's velocity and copies only position and state to its actor.
    Collision cases that test whether the *other* actor moves (the
    snowboarder's, case 2) therefore see drifting chairs and monsters as
    still. The port's merged actors carry the node velocity, so that test
    now ignores it for classes 4–8.

20. **Painting** (`FUN_00401060` → `FUN_00401540`, box from `FUN_004014b0`):
    a sprite's box is bottom-centre anchored — left = x − w/2, top =
    y − z − h on screen. Only dirty rectangles are repainted; actors whose
    boxes overlap are merged into one group, and a group is painted back
    to front by repeatedly picking the unpainted actor with the smallest
    key (strict `<`, so ties keep chain order): the key is the world y,
    or y − h when flag 0x40 is set. Flag 0x40 is `FUN_00402310(sprite)`,
    recomputed on every sprite change, and is true only for the flat
    ground sprites 0x1b (mogul field) and 0x52 (small yellow patch), so
    anything standing on them is painted over them. Sprites are masked
    with white as the colour key (also inside a sprite): mask `SRCPAINT`,
    then colour `SRCAND`, on a `WHITENESS` background. The port's
    `game::draw::draw_list` sorts the visible sprites by that key (stable)
    and draws them at those boxes; the port used to draw at (x, y − z) as
    the top-left corner, in list order, so every sprite was half a width
    right and a full height low. Verified pixel for pixel, see "Pixel
    comparison" below.
21. **Status display** (child window `SkiStatus`, painted by
    `FUN_00401b80`): `TextOutA` in `GetStockObject(OEM_FIXED_FONT)` (the
    "Terminal" bitmap font, 8×12 on the VM), black on white, inside a
    one-pixel black `FrameRect` (stock black brush) at the client's
    top-right corner (140×52 at 704 px wide). Row height = the font's
    `tmHeight` (`DAT_0040c668`); label column = the widest of strings 3–6
    (`DAT_0040c66e`), value column = the widest of strings 7–10, measured
    as raw format strings (`FUN_00406a70`); labels drawn at x 2, values at
    label width + 2, first row at y 2. The timer callback repaints it only
    when more than 327 ms (`0x147`) have passed since its last paint
    (`0x40104d`, `DAT_0040c5dc`): at 40 ms per tick, every 9th tick,
    counted from the first paint when the window shows (before tick 0).
    The course finish (`FUN_00402e80`, after clearing the course flag and
    setting the clock) repaints it at once; an **F2 restart doesn't**, and
    doesn't restart the schedule either, so the old run's values stay up
    to 8 ticks. Values: Time = `FUN_00401d70` (signed truncating division,
    each part masked to 16 bits, `%2u:%2.2u:%2.2u.%2.2u` — note the
    leading space), Dist/Speed = C `%5.2d` (precision on the digits:
    −5 → `  -05`), Dist in 16-bit arithmetic. The port keeps the painted
    values (`GameState::status_values`) and repaints by the same rules; its
    replay records carry `status` on repaint ticks, and `trace_status.js` +
    `status_align.py` compare them with the original in every scenario.
    Its glyphs come from Windows' `vgaoem.fon` (the file behind
    OEM_FIXED_FONT at 96 DPI), extracted by `tools/extract_assets.py`;
    they equal the font as GDI renders it in the VM (`export_font.js` +
    `make_font.py`), pixel for pixel. Without `vgaoem.fon` the port uses a bundled
    fallback, IBM VGA 8x14 from The Ultimate Oldschool PC Font Pack (CC
    BY-SA 4.0, `resources/fonts/`): the closest of the pack's 296 bitmap
    fonts that fits the 12-pixel rows (11 rows of ink, slashed zero), 311
    differing pixels inside the box's text at `compare_steer` tick 300.

## Verified scenarios

All run with seed 0, from a fresh process, world built at the scenario's
size. "Calls" = rand() calls compared (the original runs for the capture
time, the port for the scenario's ticks; the shorter side bounds the check).

| Scenario (`replays/`) | Size | What it covers | Ticks | Calls | Result |
|---|---|---|---|---|---|
| `compare_vm.json` | 704×681 | straight descent, no input; tree crash | 617 | 2772 | identical |
| `compare_steer.json` | 704×681 | turns, diagonals, brake, both jumps, air tricks, crash + get-up | 618 | 3004 | identical |
| `compare_size_640x480.json` | 640×480 | same inputs as `compare_steer` | 655 | 2796 | identical |
| `compare_size_1024x700.json` | 1024×700 | same inputs as `compare_steer` | 656 | 3157 | identical |
| `compare_slalom.json` | 704×681 | slalom strip: gates, rocks, repeated crash/recovery | 1250 | 6566 | identical |
| `compare_tree_slalom.json` | 704×681 | tree-slalom strip (populator zone B trees) | 1250 | 6414 | identical |
| `compare_monster.json` | 704×681 | uphill walk past y −2000: monster chase, meal, departure | 1350 | 5339 | identical |
| `compare_freestyle.json` | 704×681 | freestyle tricks: click jumps, air poses, trick scoring | 1250 | 5766 | identical, score included |
| `compare_mouse.json` | 704×681 | mouse steering on the ground and in the air, first/unchanged messages | 1250 | 6268 | identical, score included |
| `compare_restart.json` | 704×681 | two F2 restarts mid-run | 700 | 3752 (+234) | identical; the rebuilds' calls verified by stream position |
| `compare_slalom_timed.json` | 704×681 | slalom course run: interpolated start, every gate missed (+5 s each), course clock | 1250 | 6560 | identical, clock included |
| `compare_tree_slalom_timed.json` | 704×681 | tree-slalom course run, course clock | 1250 | 5452 | identical, clock included |
| `compare_focus.json` | 704×681 | focus loss/gain via real `WM_ACTIVATE`, keys/clicks/mouse dropped while inactive, F3 surviving a focus change | 700 | 3528 | identical (a control run that accepts the dropped key diverges at tick 100) |
| `compare_long.json` | 704×681 | freestyle top to bottom: finish line (high-score box auto-dismissed), then caught by the 2000 m yeti | 4500 | 20384 | identical |

Each scenario has a golden log in `replays/golden/`; `cargo test` replays
them (`verified_*` in `tests/replay.rs`), so a port change that would break
agreement with the original fails the test suite even without the VM.

## Pixel comparison

`tools/vm/frames.sh <scenario> <out-dir> <t1,t2,...>` captures the
original's client area after the listed main ticks (`capture_frames.js`:
GDI `BitBlt` of the window into a 32-bit DIB on entry to call t + 1,
before that call's injected inputs; raw pixels, no compression) and the
port's draw lists for the same ticks (`replay --draw`), then
`frames.py` composites the port's frames from the same sprite PNGs and
diffs them pixel by pixel. `frames.py --window DIR` also diffs frames
saved by the real game binary: `SKIFREE_REPLAY=<scenario>
SKIFREE_FRAMES=DIR SKIFREE_FRAME_TICKS=<t1,...> target/debug/skifree`
replays the scenario at fixed ticks (no live input, no clock, no
high-score writes) and saves each listed frame drawn by the normal
renderer (macroquad) into an offscreen target of the scenario's client
size, as raw RGBA.

Two VM setup requirements, both learned the hard way:

- frida-server must run in the **console session** (the `FridaServer`
  scheduled task, interactive logon). Started over SSH it runs in session
  0, where the game's window has no visible surface and every capture is
  black.
- The whole client area must be on screen: GDI doesn't paint rows that
  hang off the screen, so they read back white. `spawn_locked.py` moves
  the window up (keeping its size) when it would extend past the bottom —
  the VM's screen is 1280×720 and a 1024×700 client at y 61 lost its last
  41 rows.

Results (ticks spread over each run):

| Scenario | Frames | Result |
|---|---|---|
| `compare_steer` | 0, 25, 50, 120, 163, 300, 600 | identical |
| `compare_slalom` | 100 … 1240 (7) | identical |
| `compare_freestyle` | 200 … 1240 (6) | identical |
| `compare_monster` | 300 … 1340 (6) | identical |
| `compare_size_1024x700` | 0, 100, 300, 600 | identical |

"Identical" is the whole client area, status box included, for frames
drawn by the port's real renderer (macroquad) — both the macOS build and
the Windows build running in the same VM as the original
(`port_frames_win.py`; OpenGL 2.1 context there) — and for the composite,
which covers the sprite layer only.

They cover the title splash, course banners and Start posts, gates,
crashes, moguls, jumps and air tricks, the monster's chase and meal, and
two window sizes. The comparison found three port bugs: the sprite anchor
and order (item 20), course markers at z = 0x10 (item 4) and the status
display (item 21: font, frame, layout, repaint schedule, formats).

## Harness

`tools/vm/compare.sh` drives one comparison (or `--suite` for all of the
above). Pieces:

| File | Role |
|---|---|
| `spawn_locked.py` | spawns a fresh game under Frida; `--size WxH` resizes the window **on entry to the world build** (`FUN_00404b50`), on the game thread, so `MoveWindow`'s synchronous WM_SIZE updates the client-rect globals before the build reads them; tears down with a timeout (Frida's detach can hang) |
| `scenario_inputs.py` | turns a replay scenario's inputs into a Frida script that calls the game's own handlers on entry to main-tick call N (steer: down/up/left/right in the port's order; `steer_absolute` 1/3/4/6 → End/Home/PgDn/PgUp; `jump` → Insert; `restart` → F2; `click` → `FUN_004066d0`; `mouse {x, y}` → `FUN_00406550`). A tick containing a `focus` input is instead delivered as real window messages (`SendMessageA`: `WM_ACTIVATE`, `WM_KEYDOWN` — `pause` = F3 —, `WM_LBUTTONDOWN`, `WM_MOUSEMOVE`) right after main-tick call N−1, so the original's window procedure and its input gating handle them. Inputs run inside a Frida hook, where `fixed_clock.js` can't act, so afterwards the injector writes the fixed clock's time back to `DAT_0040c698` (an F2 new game and a timer restart store `GetTickCount` there; the real uptime used to leak in and stop the status display's repaints) |
| `no_dialogs.js` | patches the three `call [MessageBoxA]` sites to return IDOK (`push 1; pop eax; add esp, 0x10`), so the course-finish high-score box doesn't block the run |
| `fixed_clock.js` | makes the clock deterministic: after each of the three `GetTickCount` call sites in the game module, sets `eax = T0 + 40 · timer callbacks`, matching the replay's 40 ms ticks (hooking the kernel32 export does not take effect under the VM's emulation) |
| `trace_player.js` | player state, style score, course clock (`DAT_0040c944`) and the three course flags at the end of every main-tick call (a player flagged 0x08 is reported as gone) |
| `trace_rand.js` | every `rand_range` call: caller, modulus, post-call LCG state, main-tick index — batched into one message per tick |
| `trace_ticks.js` | player per *integrated* tick plus skier collisions (legacy view; defines `tick2`) |
| `trace_updates.js` | each `FUN_004028e0` update (class/state/position) per tick — diff with the port's `SKIFREE_LOG_UPDATES=1` |
| `trace_pairs.js` | each collision-handler call (both actors, both orders) per tick |
| `rng_align.py` | maps LCG states to stream indices and prints the first call where original and port differ, with call sites on both sides. Calls the original makes *inside* an injected input (an F2 world rebuild) are invisible to Frida — it doesn't fire hooks for calls made from a hook callback — so a port re-seed whose calls the original's log skips is accepted when the original resumes at exactly the same stream index |
| `player_align.py` | first tick where the player's x/y/z/vx/vy/vz/state, the style score or the course clock differ |
| `trace_status.js` | every status-display repaint (`FUN_00401b80`) with its value strings and main-tick call |
| `status_align.py` | first status repaint where original and port differ (tick or strings) |
| `capture_frames.js` | saves the client area after each main tick in `CAPTURE_TICKS` (pixel comparison) |
| `export_font.js` / `make_font.py` | render the OEM fixed font in the VM and turn it into `assets/fonts/oem_fixed.png` (cross-check of the `vgaoem.fon` extraction) |
| `port_frames_win.py` | runs the port's Windows build (`x86_64-pc-windows-gnu`) in the VM in frame-capture mode, through a one-shot interactive scheduled task (the VM's x86 frida-server can't spawn x64 programs) |
| `frames.sh` / `frames.py` | run a pixel comparison and write `orig_`, `port_`, `diff_`, `side_` (and `win_`, `diffwin_`) PNGs per tick |

```bash
export SKIFREE_VM_HOST=<vm-ip>             # frida-server :27042
export SKIFREE_VM_SSH=user@vm-host         # for killing leftovers
export SKIFREE_VM_EXE='C:\Users\<you>\Downloads\ski32-autoplay.exe'
tools/vm/compare.sh replays/compare_steer.json /tmp/steer 32
tools/vm/compare.sh --suite /tmp/suite     # all verified scenarios (~6 min)
```

New scenarios need a duration that covers their ticks at ~19 ticks/s.
`fast_forward` (the F cheat) has no deterministic mapping and is rejected
by `scenario_inputs.py`. Pause can't be injected per tick (a paused original
runs no ticks at all); its behavior is covered by item 17 and a unit test.

## Debugging a divergence

1. `rng_align.py` names the first call that differs. The original's caller
   address says which consumer ran or didn't: `0x40430a` class-4 roll,
   `0x40467c` monster roll, `0x4039c2`/`0x4039e2` crossing-skier wander,
   `0x403829` lift wander, `0x402569` populator position, `0x402718`
   open-slope picker, `0x402808` tree-slalom-strip picker, `0x40286e` tree
   sprite, `0x40208e` 1-in-666.
2. `player_align.py` shows whether the player differs first (a physics or
   collision bug) or the RNG does (a consumer bug).
3. Diff actor updates: `trace_updates.js` (edit its tick window) vs
   `SKIFREE_LOG_UPDATES=1`; collisions: `trace_pairs.js` vs
   `SKIFREE_LOG_COLLISIONS=1`; spawns: `SKIFREE_LOG_SPAWNS=1`.
4. Read the responsible routine with rizin
   (`rizin -q -e scr.color=0 -c 'aaa; pdf @ 0x…' originals/ski32.exe`) and
   dump whatever state it reads with a small Frida script appended to the
   capture.

Pitfalls met along the way:

- Labelling original events by integrated-tick count drifts during crashes
  (the crashed skier is not integrated); always key on main-tick calls.
- Posting window messages for input is asynchronous; call the handlers.
- Resizing the window after the world build mixes two sizes.
- Wall-clock code needs the fixed clock, or the meal timing drifts.
- `DAT_0040c72c` lags one tick behind a removal; check flag 0x08.
- Each actor appears twice when walking memory around it: its live record
  and its previous-position copy.
- A "slow" capture that stops advancing is usually blocked in a modal
  dialog (course finish); load `no_dialogs.js`.
- One Frida message per event throttles the game several times in busy
  scenes; batch per tick.
- Keep at most three listeners on `FUN_00401e50`: with a fourth, the last
  attached one's `onLeave` silently stopped running under the VM's x86
  emulation (the injector's window messages vanished and `compare_focus`
  diverged). Count main ticks on `FUN_00401000` instead (its only caller).
- frida-server started over SSH runs in session 0: everything works except
  pixels (captures come back black). Use the interactive `FridaServer`
  task.
- Inputs injected from a hook bypass `fixed_clock.js`; the injector puts
  the fixed time back into `DAT_0040c698` (see `scenario_inputs.py`).

Port-side RNG call-site histogram: `cargo run --features rng_sites --bin
replay -- replays/compare_vm.json` (via `#[track_caller]` in
`Rng::rand_range`); ordered call log: add `SKIFREE_LOG_RNG=1`.
