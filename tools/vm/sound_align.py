#!/usr/bin/env python3
"""Compare the sounds the original and the port play.

Usage: sound_align.py <orig.txt> <port.jsonl>

<orig.txt> holds trace_sounds.js records ({'t': 'sound', 'tick': N,
'id': resource}); <port.jsonl> is the replay's output, whose records carry
`sounds` (resource ids in call order) when the port played any in that
tick. Both lists of (tick, id) must match up to the shorter run's last
tick; the original's last tick is its last trace_player.js ('p') record.
"""
import ast
import json
import sys
from collections import Counter


def main():
    orig, orig_last = [], -1
    for line in open(sys.argv[1]):
        if line.startswith("{'t': 'sound'"):
            m = ast.literal_eval(line.strip())
            orig.append((m["tick"], m["id"]))
        elif line.startswith("{'t': 'p'"):
            orig_last = max(orig_last, ast.literal_eval(line.strip())["call"])
    port, last = [], -1
    for line in open(sys.argv[2]):
        if not line.strip():
            continue
        r = json.loads(line)
        last = r["tick"]
        port.extend((r["tick"], s) for s in r.get("sounds", []))
    if orig_last < 0:
        # No main-tick records: the capture is unusable, not a match.
        print("sounds: the original's trace has no main-tick records")
        return 1
    end = min(orig_last, last)
    o = [p for p in orig if p[0] <= end]
    q = [p for p in port if p[0] <= end]
    for i, (a, b) in enumerate(zip(o, q)):
        if a != b:
            print(f"sounds: first difference at call {i}: original {a}, port {b}")
            return 1
    if len(o) != len(q):
        i = min(len(o), len(q))
        print(f"sounds: call count differs ({len(o)} vs {len(q)}); next: "
              f"original {o[i] if i < len(o) else None}, port {q[i] if i < len(q) else None}")
        return 1
    ids = ", ".join(f"{k}x{v}" for k, v in sorted(Counter(s for _, s in o).items()))
    print(f"sounds: identical for all {len(o)} calls up to tick {end} ({ids or 'none'})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
