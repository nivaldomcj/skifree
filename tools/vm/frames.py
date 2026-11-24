#!/usr/bin/env python3
"""Compare the original's captured frames with the port's draw lists.

Usage: frames.py <out-dir> [--window DIR]

Reads <out-dir>/orig/frame_<t>.raw (+ the sizes logged in orig.txt) and
<out-dir>/port_draw.jsonl, composites the port's frame with the same sprite
PNGs the port draws (white background, white = transparent), and writes
orig_<t>.png, port_<t>.png, diff_<t>.png (differing pixels in red over a
faded original) and side_<t>.png (original | port). Prints, per tick, how
many pixels differ and where.

--window DIR also compares the real renderer's frames, saved by the game
itself in frame-capture mode (SKIFREE_REPLAY=<scenario> SKIFREE_FRAMES=DIR
SKIFREE_FRAME_TICKS=<t1,t2,...> skifree) as raw RGBA: win_<t>.png and
diffwin_<t>.png, against the original and against the composite.
"""
import glob
import re
import ast
import json
import os
import sys

import numpy as np
from PIL import Image

ROOT = os.path.join(os.path.dirname(__file__), "..", "..")
SPRITES = os.path.join(ROOT, "assets", "sprites")


def load_orig(out):
    frames = {}
    for line in open(os.path.join(out, "orig.txt")):
        if not line.startswith("{"):
            continue
        try:
            msg = ast.literal_eval(line.strip())
        except (ValueError, SyntaxError):
            continue
        if msg.get("t") == "frame" and "file" in msg:
            raw = open(os.path.join(out, "orig", msg["file"]), "rb").read()
            bgrx = np.frombuffer(raw, np.uint8).reshape(msg["h"], msg["w"], 4)
            frames[msg["tick"]] = Image.fromarray(bgrx[:, :, 2::-1].copy(), "RGB")
        elif msg.get("t") == "frame_error":
            print("capture error:", msg, file=sys.stderr)
    return frames


def render_port(frame, cache={}):
    img = Image.new("RGB", (frame["w"], frame["h"]), (255, 255, 255))
    for sprite, left, top in frame["items"]:
        if sprite not in cache:
            cache[sprite] = Image.open(os.path.join(SPRITES, f"bitmap_{sprite:02}.png")).convert("RGBA")
        spr = cache[sprite]
        img.paste(spr, (left, top), spr)
    return img


def load_window(path):
    frames = {}
    for f in glob.glob(os.path.join(path, "frame_*_*x*.rgba")):
        tick, w, h = map(int, re.search(r"frame_(\d+)_(\d+)x(\d+)\.rgba$", f).groups())
        rgba = np.frombuffer(open(f, "rb").read(), np.uint8).reshape(h, w, 4)
        frames[tick] = Image.fromarray(rgba[:, :, :3].copy(), "RGB")
    return frames


def compare(a_img, b_img, out_png):
    a, b = np.asarray(a_img), np.asarray(b_img)
    diff = np.any(a != b, axis=2)
    faded = (a.astype(np.uint16) + 2 * 255) // 3
    faded[diff] = (255, 0, 0)
    Image.fromarray(faded.astype(np.uint8)).save(out_png)
    ys, xs = np.nonzero(diff)
    box = f" in x {xs.min()}..{xs.max()}, y {ys.min()}..{ys.max()}" if len(xs) else ""
    return f"{int(diff.sum())} pixels differ{box}"


def main():
    out = sys.argv[1]
    if len(sys.argv) not in (2, 4) or (len(sys.argv) == 4 and sys.argv[2] != "--window"):
        sys.exit(__doc__)
    window_dir = sys.argv[3] if len(sys.argv) == 4 else None
    window = load_window(window_dir) if window_dir else {}
    orig = load_orig(out)
    port = {}
    for line in open(os.path.join(out, "port_draw.jsonl")):
        frame = json.loads(line)
        port[frame["tick"]] = render_port(frame)
    for tick in sorted(set(orig) | set(port)):
        if tick not in orig or tick not in port:
            print(f"tick {tick}: missing {'original' if tick not in orig else 'port'} frame")
            continue
        o, p = orig[tick], port[tick]
        o.save(os.path.join(out, f"orig_{tick}.png"))
        p.save(os.path.join(out, f"port_{tick}.png"))
        if o.size != p.size:
            print(f"tick {tick}: size differs: original {o.size}, port {p.size}")
            continue
        side = Image.new("RGB", (o.width * 2 + 8, o.height), (128, 128, 128))
        side.paste(o, (0, 0))
        side.paste(p, (o.width + 8, 0))
        side.save(os.path.join(out, f"side_{tick}.png"))
        print(f"tick {tick}: {compare(o, p, os.path.join(out, f'diff_{tick}.png'))}")
        if window_dir is None:
            continue
        if tick not in window:
            print("  window: missing window frame")
            continue
        win = window[tick]
        win.save(os.path.join(out, f"win_{tick}.png"))
        if win.size != o.size:
            print(f"  window: size {win.size}")
            continue
        print(f"  window vs original:  {compare(o, win, os.path.join(out, f'diffwin_{tick}.png'))}")
        print(f"  window vs composite: {compare(p, win, os.path.join(out, f'diffwc_{tick}.png'))}")


if __name__ == "__main__":
    main()
