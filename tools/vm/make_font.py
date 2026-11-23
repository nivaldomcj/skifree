#!/usr/bin/env python3
"""Turn export_font.js's capture into the port's status-display font.

Usage: make_font.py <spawn output.txt> <data-dir> [out.png]

Writes assets/fonts/oem_fixed.png (default): printable ASCII 0x20-0x7e in
one row of 8x12 cells, glyph pixels black and opaque, background
transparent — the stock OEM fixed font ("Terminal") as Windows renders it.
"""
import ast
import os
import sys

import numpy as np
from PIL import Image


def main():
    meta = next(ast.literal_eval(l.strip()) for l in open(sys.argv[1]) if l.startswith("{'t': 'font'"))
    raw = open(os.path.join(sys.argv[2], meta["file"]), "rb").read()
    px = np.frombuffer(raw, np.uint8).reshape(meta["h"], meta["w"], 4)
    cw, ch = meta["tmAveCharWidth"], meta["tmHeight"]
    assert meta["tmMaxCharWidth"] == cw, "not a fixed-pitch font"
    cells = px[:ch, : cw * meta["count"], :3]
    ink = np.all(cells == 0, axis=2)
    assert np.all(ink | np.all(cells == 255, axis=2)), "expected pure black on white"
    rgba = np.zeros((ch, cw * meta["count"], 4), np.uint8)
    rgba[ink] = (0, 0, 0, 255)
    out = sys.argv[3] if len(sys.argv) > 3 else os.path.join(
        os.path.dirname(__file__), "..", "..", "assets", "fonts", "oem_fixed.png")
    os.makedirs(os.path.dirname(out), exist_ok=True)
    Image.fromarray(rgba, "RGBA").save(out)
    print(f"{out}: {meta['count']} glyphs {cw}x{ch} (ascent {meta['tmAscent']}), first 0x{meta['first']:02x}")


if __name__ == "__main__":
    main()
