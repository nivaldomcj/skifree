#!/usr/bin/env python3
"""Compare the status display repaints of the original and the port.

Usage: status_align.py <orig.txt> <port.jsonl>

<orig.txt> holds trace_status.js records ({'t': 'status', 'after': N,
'values': [...]}: repainted during main-tick call N, or between ticks right
before it); <port.jsonl> is the replay's output, whose records carry
`status` when the port repainted it during or right before that tick.
Repaints the original made where the hooks couldn't see them ('status_hidden',
e.g. a resize) are compared by tick only. Both lists of
(tick, values) must match up to the shorter run's last tick; the original's
last tick is its last fully traced main-tick call ('p' records).
"""
import ast
import json
import sys


def main():
    orig, calls = [], []
    for line in open(sys.argv[1]):
        if line.startswith("{'t': 'status"):
            m = ast.literal_eval(line.strip())
            orig.append((m["after"], m.get("values")))
        elif line.startswith("{'t': 'p'"):
            calls.append(ast.literal_eval(line.strip())["call"])
    port, last = [], -1
    for line in open(sys.argv[2]):
        if not line.strip():
            continue
        r = json.loads(line)
        last = r["tick"]
        if r.get("status"):
            port.append((r["tick"], r["status"]))
    if not orig:
        # An empty capture (a hook that never fired, a run cut short) is a
        # failed trace, not a match.
        print("status: the original's trace has no repaints")
        return 1
    # The last call may have been cut off before its repaint ran.
    end = min(max(calls) - 1 if calls else orig[-1][0], last)
    o = [p for p in orig if p[0] <= end]
    q = [p for p in port if p[0] <= end]
    for i, (a, b) in enumerate(zip(o, q)):
        if a[1] is None:  # hidden repaint: tick only
            a, b = a[0], b[0]
        if a != b:
            print(f"status: first difference at repaint {i}: original {a}, port {b}")
            return 1
    if len(o) != len(q):
        i = min(len(o), len(q))
        print(f"status: repaint count differs ({len(o)} vs {len(q)}); next: "
              f"original {o[i] if i < len(o) else None}, port {q[i] if i < len(q) else None}")
        return 1
    print(f"status: identical for all {len(o)} repaints up to tick {end}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
