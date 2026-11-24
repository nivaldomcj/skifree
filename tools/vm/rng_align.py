#!/usr/bin/env python3
"""Align the original's rand() calls with the port's, call by call.

Usage:
  rng_align.py <original-run.txt> <port-rng-log.txt> [context] [seed]

<original-run.txt>: output of spawn_locked.py with trace_ticks.js +
trace_rand.js. <port-rng-log.txt>: stderr of
  SKIFREE_LOG_RNG=1 cargo run --features rng_sites --bin replay -- replays/compare_vm.json

Every original call's post-call LCG state is mapped to its stream index
from the scenario's seed (default 0; compare.sh passes it), so the two sides are compared by stream position: same index,
same modulus. Prints the first divergence with surrounding context: the
original's caller address (tick = main-tick call index, = port tick) next to
the port's file:line.
"""
import ast
import sys


def original_calls(path, seed=0):
    calls, ticks = [], 0
    for line in open(path):
        line = line.strip()
        if not line.startswith("{"):
            continue
        d = ast.literal_eval(line)
        if d.get("t") == "tick":
            ticks += 1
        elif d.get("t") == "rb":
            for call, ra, n, st in d["calls"]:
                calls.append((call, ra, n, st))
        elif d.get("t") == "r":
            # Prefer the main-tick index; older captures only have the
            # integrated-tick count (which stalls while the skier is crashed).
            calls.append((d.get("call", ticks), d["ra"], d["n"], d["s"]))
    need, idx, s = {c[3] for c in calls}, {}, seed
    for i in range(1, 50_000_000):
        s = (s * 214013 + 2531011) % 2**32
        if s in need:
            idx[s] = i
            if len(idx) == len(need):
                break
    return [(idx.get(st), t, ra, n) for t, ra, n, st in calls]


def port_calls(path):
    out = []
    for line in open(path):
        parts = line.split()
        if len(parts) == 4 and parts[0] == "rng" and parts[1].isdigit():
            out.append((int(parts[1]), int(parts[2]), parts[3]))
    return out


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    ctx = int(sys.argv[3]) if len(sys.argv) > 3 else 10
    seed = int(sys.argv[4]) if len(sys.argv) > 4 else 0
    o, p = original_calls(sys.argv[1], seed), port_calls(sys.argv[2])
    print(f"original calls: {len(o)}  port calls: {len(p)}")
    if not o:
        print("the original's trace has no rand() calls")
        return 1
    # Walk both logs. Calls the original makes inside an injected input
    # handler (an F2 world rebuild) are invisible to trace_rand.js — Frida
    # does not fire hooks for calls made from inside a hook callback. Those
    # runs show up as a port re-seed (stream index drops back) the original
    # log skips; accept them when the original resumes at exactly the stream
    # index the port reaches after the run, which proves the same number of
    # calls happened.
    i = j = hidden = 0
    while i < len(o) and j < len(p):
        if o[i][0] == p[j][0] and o[i][3] == p[j][1]:
            i += 1
            j += 1
            continue
        if j > 0 and p[j][0] < p[j - 1][0]:
            k = j
            while k < len(p) and k - j < 1000 and p[k][0] != o[i][0]:
                k += 1
            if k < len(p) and p[k][0] == o[i][0] and p[k][1] == o[i][3]:
                print(f"  port calls {j}..{k - 1} (stream {p[j][0]}..{p[k - 1][0]}, after a "
                      f"re-seed) happened inside an injected input in the original; "
                      f"it resumes at stream index {o[i][0]} like the port")
                hidden += k - j
                j = k
                continue
        break
    if i >= len(o) or j >= len(p):
        print(f"identical for all {i} common calls" + (f" (+{hidden} verified by stream position)" if hidden else ""))
        return 0
    print(f"first divergence at original call {i} / port call {j}")
    for d in range(-ctx, ctx):
        a, b = i + d, j + d
        if a < 0 or b < 0 or a >= len(o) or b >= len(p):
            continue
        _, t, ra, on = o[a]
        _, pn, site = p[b]
        mark = ">>" if d == 0 else "  "
        print(f"{mark}{a:5d} | orig t{t:<4} {ra} n={on:<5} | port n={pn:<5} {site}")
    return 1


if __name__ == "__main__":
    sys.exit(main())
