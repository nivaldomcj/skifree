#!/usr/bin/env python3
"""Align the original's rand() calls with the port's, call by call.

Usage:
  rng_align.py <original-run.txt> <port-rng-log.txt> [context]

<original-run.txt>: output of spawn_locked.py with trace_ticks.js +
trace_rand.js. <port-rng-log.txt>: stderr of
  SKIFREE_LOG_RNG=1 cargo run --features rng_sites --bin replay -- replays/compare_vm.json

Every original call's post-call LCG state is mapped to its stream index
(seed 0), so the two sides are compared by stream position: same index,
same modulus. Prints the first divergence with surrounding context: the
original's caller address (tick = integrated-tick count at the call) next to
the port's file:line.
"""
import ast
import sys


def original_calls(path):
    calls, ticks = [], 0
    for line in open(path):
        line = line.strip()
        if not line.startswith("{"):
            continue
        d = ast.literal_eval(line)
        if d.get("t") == "tick":
            ticks += 1
        elif d.get("t") == "r":
            calls.append((ticks, d["ra"], d["n"], d["s"]))
    need, idx, s = {c[3] for c in calls}, {}, 0
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
    o, p = original_calls(sys.argv[1]), port_calls(sys.argv[2])
    n = min(len(o), len(p))
    first = next((i for i in range(n) if o[i][0] != p[i][0] or o[i][3] != p[i][1]), None)
    print(f"original calls: {len(o)}  port calls: {len(p)}")
    if first is None:
        print(f"identical for all {n} common calls")
        return
    print(f"first divergence at call {first}")
    for j in range(max(0, first - ctx), min(first + ctx, n)):
        _, t, ra, on = o[j]
        _, pn, site = p[j]
        mark = ">>" if j == first else "  "
        print(f"{mark}{j:5d} | orig t{t:<4} {ra} n={on:<5} | port n={pn:<5} {site}")


if __name__ == "__main__":
    main()
