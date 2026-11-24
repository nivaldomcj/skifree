#!/usr/bin/env python3
"""Compare the player tick by tick: original (trace_player.js) vs port replay.

Usage: player_align.py <original-run.txt> <port-ticks.jsonl> [context]

Original call N (end of FUN_00401e50) is matched with the port's tick N.
Reports the first tick where x/y/z/vx/vy/vz/state, the style score
(DAT_0040c6a8) or the course clock (DAT_0040c944; only when the capture has
it) differ, with context.
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
        player = tuple(d.get(f) for f in FIELDS) if "x" in d else None
        return (player, d.get("score"), d.get("cms", port[i].get("course_ms")))

    def p_tuple(i):
        p = port[i]["player"]
        return (tuple(p[f] for f in FIELDS) if p else None, port[i]["score"], port[i].get("course_ms"))

    first = next((i for i in range(n) if o_tuple(i) != p_tuple(i)), None)
    print(f"original ticks: {len(orig)}  port ticks: {len(port)}  fields: {FIELDS} + score + course clock")
    if not orig:
        print("player: the original's trace has no ticks")
        return 1
    if first is None and len(orig) < len(port):
        # The capture ended before the scenario did (the VM ran slower than
        # the seconds allowed): the tail went unchecked, so this is no pass.
        # The other aligners stop at the original's end too.
        print(f"player identical for the {n} ticks the original ran, but the scenario has {len(port)}: "
              "capture too short, raise its seconds")
        return 1
    if first is None:
        print(f"player identical for all {n} common ticks")
        return 0
    print(f"first player mismatch at tick {first}")
    for i in range(max(0, first - ctx), min(n, first + ctx)):
        mark = ">>" if o_tuple(i) != p_tuple(i) else "  "
        print(f"{mark}{i:4d} orig {o_tuple(i)}  port {p_tuple(i)}")
    return 1


if __name__ == "__main__":
    sys.exit(main())
