#!/usr/bin/env python3
"""Compare the player tick by tick: original (trace_player.js) vs port replay.

Usage: player_align.py <original-run.txt> <port-ticks.jsonl> [context]

Original call N (end of FUN_00401e50) is matched with the port's tick N.
Reports the first tick where x/y/z/vx/vy/vz/state differ, with context.
"""
import ast
import json
import sys

FIELDS = ("x", "y", "z", "vx", "vy", "vz", "state")


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    ctx = int(sys.argv[3]) if len(sys.argv) > 3 else 5
    orig = {}
    for line in open(sys.argv[1]):
        line = line.strip()
        if line.startswith("{'t': 'p'"):
            d = ast.literal_eval(line)
            orig[d["call"]] = d
    port = [json.loads(l) for l in open(sys.argv[2])]
    n = min(len(orig), len(port))

    def o_tuple(i):
        d = orig[i]
        return tuple(d.get(f) for f in FIELDS) if "x" in d else None

    def p_tuple(i):
        p = port[i]["player"]
        return tuple(p[f] for f in FIELDS) if p else None

    first = next((i for i in range(n) if o_tuple(i) != p_tuple(i)), None)
    print(f"original ticks: {len(orig)}  port ticks: {len(port)}  fields: {FIELDS}")
    if first is None:
        print(f"player identical for all {n} common ticks")
        return
    print(f"first player mismatch at tick {first}")
    for i in range(max(0, first - ctx), min(n, first + ctx)):
        mark = ">>" if o_tuple(i) != p_tuple(i) else "  "
        print(f"{mark}{i:4d} orig {o_tuple(i)}  port {p_tuple(i)}")


if __name__ == "__main__":
    main()
