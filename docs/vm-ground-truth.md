# Ground truth from the original: Frida on Windows 11 ARM (VM)

How we verify the port against the original game itself, and what we learned.
This is the methodology that produced the collision/RNG fidelity fixes.

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
- Game launched via scheduled task `SkiAutoplay` (`/it` so it lands on the
  interactive desktop). SSH key auth works for orchestration.
- frida-tools (pipx) on the Mac connects to `<vm-ip>:27042`. Frida's
  x86-under-Prism instrumentation works — a key result for future
  DX6-era ports.

## Key addresses (module base is always 0x400000, no ASLR)

| Address | Meaning |
|---------|---------|
| `0x0040c16c` | MSVC LCG state (`rand` is statically linked at `0x406cda`, `srand` at `0x406cd0`) |
| `0x00402be0` | physics integrator (`__thiscall`, ecx = actor) — hook to trace any actor |
| `0x00401e50` | main tick |
| `0x00403a00` | collision dispatch |
| `0x00402390` | position setter (ecx=actor, edx=x, [esp+4]=y, [esp+8]=z) |
| `0x004020d0` | create actor (ecx=class, edx=state) → eax |
| `0x004020b0` | `rand() % n` wrapper — hook `0x406cda` and unwind past it for real callers |
| `0x0040c618` | actor-list-related global |

Actor struct (0x50 bytes): `+0x10` sprite, `+0x14` sprite-data ptr,
`+0x18` class, `+0x1c` state, `+0x20`/`+0x30` cached screen AABBs,
`+0x40/42/44` x/y/z, `+0x46/48/4a` vx/vy/vz (all s16), `+0x4c` flags.

## Verified facts

1. **RNG**: the game uses the MSVC LCG statically linked; patching `srand`
   to seed 0 makes the entire stream reproducible. Any observed
   `DAT_0040c16c` value maps to an exact stream index — the universal
   alignment key between runs and between original and port.
2. **World build consumes exactly 117 rand calls** (39 cycles of 3:
   24 slalom rows + 15 tree-slalom rows). The port matches exactly.
   F2 re-seeds (srand) and rebuilds, but **an F2-restarted world is NOT
   identical to a fresh-process world** (populator accumulators carry
   over) — always compare against fresh processes.
3. **Descent physics** (vy acceleration, integrator) match the port
   tick-for-tick. The tick trace hooks the integrator, so it emits one
   record per *integrated* tick: a crashed skier (state 0xb) is not
   integrated, so the trace appears frozen while it decelerates.
4. **Course markers** carry z = 0 in their spawn-event nodes; once
   materialized they draw 16 units up, and the skier grazes the banner's box
   on the crossing tick (see item 5).
5. **Collision switch** (`FUN_00403a00`): first-actor class ≥ 0xb → early
   out; the `cmp [var_ch], 0x11` is a *state* check (crash-landing), not a
   class check. The inner switch is indexed by (class−1). Its
   **crossing flag** (`local_4`) compares the skier's current and saved
   pre-move y against the other actor's y (`FUN_00404070` returns a static
   actor itself as its "previous" copy) and is set on exactly one tick per
   crossing: the tick the skier's y reaches or passes the other's y. It
   gates the graze (cases 2/12/17: halve vy) and the solid-obstacle hit
   (cases 1/3/4/9/10/13: crash, or for trees a glancing blow — `|dx| >
   max(w)/2` only halves vy). Port: `collision::crossed`.
   Case 0xf (large rock): grounded → hop (vz 4); airborne below the rock's
   top (`z < other.z + h`) → bounce at vy/2; at or above the top → nothing.
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
   −1024), drifting 2/tick from world build.
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
   sprite at z = 0); (3) mob nodes (`FUN_004040a0`); (4) collisions; (5)
   scroll accumulators → populator edge spawns (`FUN_004025c0`); (6) the
   1-in-666 crossing-skier roll, whose x is `rand_range(screen_w)` inside
   `FUN_004024f0` direction 2.
10. **Crash (state 0xb)**: `FUN_004028e0` steps vx/vy one unit toward zero
    per tick *without integrating* (the skier stays put), then switches to
    state 0xc once both are zero.

### Result: call-level lockstep

With items 4–10 in the port, a seeded descent (`replays/compare_vm.json`:
seed 0, 704×681, a single "down" at tick 0 to stand in for the autoplay
start-state patch, no other input) consumes the RNG **identically to a
fresh `ski32-autoplay.exe` process for all 2519 calls of a 30 s capture**:
same stream index, same modulus, call for call, including a tree crash at
integrated tick 228. Spawned obstacle layouts are therefore identical too.

## Reproducing a comparison

Scripts live in `tools/vm/` (Frida on the Mac; the frida-server scheduled
task must be running in the VM).

```bash
FRIDA_PY="$HOME/Library/Application Support/pipx/venvs/frida-tools/bin/python"
EXE='C:\path\to\ski32-autoplay.exe'   # in the VM
VM=<vm-host>
# kill leftovers: every capture must be a fresh process
ssh "$VM" 'taskkill /f /im ski32-autoplay.exe'
# 1. original: per-tick player records + every rand() call (30 s)
"$FRIDA_PY" tools/vm/spawn_locked.py "$EXE" 30 "$VM" \
    tools/vm/trace_ticks.js tools/vm/trace_rand.js > /tmp/orig_run.txt
# 2. port: every rand() call in order, with its call site
SKIFREE_LOG_RNG=1 cargo run -q --features rng_sites --bin replay -- \
    replays/compare_vm.json 2> /tmp/port_rng.txt > /tmp/port_ticks.jsonl
# 3. first divergence, with both sides' call sites around it
python3 tools/vm/rng_align.py /tmp/orig_run.txt /tmp/port_rng.txt
```

When `rng_align.py` reports a divergence, the original's caller address
(e.g. `0x40430a` = class-4 roll, `0x40467c` = monster roll, `0x402569` =
populator position, `0x402718` = open-slope picker, `0x40208e` = 1-in-666)
says which consumer ran or didn't; dump the relevant state with an extra
Frida script appended to step 1 (`tick2` from `trace_ticks.js` is in scope).

Port-side RNG call-site histogram: `cargo run --features rng_sites --bin
replay -- replays/compare_vm.json` (via `#[track_caller]` in
`Rng::rand_range`).
