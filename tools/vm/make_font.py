#!/usr/bin/env python3
"""Turn export_font.js's capture into the port's status-display font.

Usage: make_font.py <spawn output.txt> <data-dir> [out.png]

Renders the stock OEM fixed font ("Terminal") as Windows draws it:
printable ASCII 0x20-0x7e in one row of 8x12 cells, glyph pixels black and
opaque, background transparent. Without out.png it cross-checks the render
pixel by pixel against assets/fonts/oem_fixed.png (what extract_assets.py
cut from vgaoem.fon) and exits non-zero on any difference; with out.png it
writes the render there.
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
    print(f"{meta['count']} glyphs {cw}x{ch} (ascent {meta['tmAscent']}), first 0x{meta['first']:02x}")
    if len(sys.argv) > 3:
        out = sys.argv[3]
        os.makedirs(os.path.dirname(out) or ".", exist_ok=True)
        Image.fromarray(rgba, "RGBA").save(out)
        print(f"wrote {out}")
        return 0
    ref = os.path.join(os.path.dirname(__file__), "..", "..", "assets", "fonts", "oem_fixed.png")
    want = np.asarray(Image.open(ref).convert("RGBA"))
    if want.shape != rgba.shape:
        print(f"size differs: render {rgba.shape[1]}x{rgba.shape[0]}, {ref} {want.shape[1]}x{want.shape[0]}")
        return 1
    diff = int(np.count_nonzero(np.any(want != rgba, axis=2)))
    print(f"{diff} pixels differ from {ref}")
    return 1 if diff else 0


if __name__ == "__main__":
    sys.exit(main())
