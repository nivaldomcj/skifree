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
   tick-for-tick.
4. **Entrance banners never collide**: they hang at z=0x42; their cached
   AABBs stay above the skier's box at all times (dumped and verified).
5. **Collision switch** (`FUN_00403a00`): first-actor class ≥ 0xb → early
   out; skier vs class 0x11 is *not* specially skipped (that `cmp [var_ch],
   0x11` is a state check, not a class check — var_ch holds a *state*);
   the inner switch is indexed by (class−1): cases 2/12/17 share
   "halve vy (flag always set) + sprite==0x52 → −16 style".
6. **Per-tick RNG consumption**: original ≈ 5.2 calls/tick during descent;
   the port ≈ 3.1–4.5. Consumers identified by call site:
   - main tick: `rand_range(666)` event spawn (~0.8/tick) — port has it.
   - mob behavior `fcn.00404350` (monster proxies): `rand_range(10)`
     (~1.7/tick) — port ≈ 1.0/tick.
   - `fcn.00404290` class-4 updater (lift chairs): `rand_range(1000)` in
     state 0x27 (~0.4/tick, drops a crossing skier on win) — port ≈ 1.8/tick
     (both proxies sit in state 0x27 permanently; in the original only a
     fraction of chairs are in that state at a time).
   - `fcn.004028e0` per-actor updates: `rand_range(100)`+`rand_range(2)`
     (lift-chair wander), `rand_range(12)`+`rand_range(3)` (mob wander) —
     port has both patterns.
   - populator: position/zone pickers — port matches (~0.2/tick each).
   The remaining obstacle-layout desync comes from the consumption-rate
   differences above: the streams diverge, so later populator rolls differ.

## Reproducing a comparison

```bash
# 1. fresh game in the VM
ssh user@vm-host 'taskkill /IM ski32-autoplay.exe /F 2>nul & schtasks /run /tn SkiAutoplay'
# 2. trace the original (player per tick: x/y/z/vx/vy/state/rng)
python3 /tmp/skifree-compare/frida_trace.py ski32-autoplay.exe 30 999999 \
    /tmp/skifree-compare/original_fresh.jsonl <vm-ip> 0
# 3. run the port's replay (same seed, same client size 704x681)
cargo run -q --bin replay -- replays/compare_vm.json > /tmp/skifree-compare/port_descent.jsonl
# 4. diff (see /tmp/skifree-compare/compare_descents.py)
```

Port-side RNG call-site accounting: `cargo run --features rng_sites --bin
replay -- replays/compare_vm.json` prints a per-site histogram (via
`#[track_caller]` in `Rng::rand_range`).
