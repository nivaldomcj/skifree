#!/usr/bin/env python3
"""Build the port's bundled fallback font for the status display.

Usage: make_fallback_font.py <path to Bm437_IBM_VGA_8x14.FON>

The FON comes from The Ultimate Oldschool PC Font Pack by VileR
(https://int10h.org/oldschool-pc-fonts/, CC BY-SA 4.0). Writes
resources/fonts/ibm_vga_8x12.png in the same layout as the extracted
Terminal font (assets/fonts/oem_fixed.png): printable ASCII 0x20-0x7e in
one row of 8x12 cells, black glyph pixels on transparent.

SkiFree's status display uses Terminal (8x12 cells, 11 rows of ink); the
IBM VGA 8x14 glyphs also use 11 rows, so each glyph keeps rows 1-12 of its
14-row cell. '^' and '`' keep rows 0-11 instead (their ink starts at row
0); '$' is 13 rows tall and loses its top pixel. Stdlib only.
"""
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tools"))
import extract_assets as ea  # noqa: E402


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    faces = ea.parse_fon_fonts(Path(sys.argv[1]).read_bytes())
    info, glyphs = ea.fnt_glyph_rows(faces[0])
    assert info["height"] == 14, info
    cells = []
    for code in range(0x20, 0x7F):
        g = glyphs[code]
        assert len(g[0]) == 8
        top = 1
        if any(g[0]) and not any(g[12]) and not any(g[13]):
            top = 0
        cells.append(g[top:top + 12])
    rows = [[(0, 0, 0, 255) if c[y][x] else (0, 0, 0, 0) for c in cells for x in range(8)]
            for y in range(12)]
    out = ROOT / "resources" / "fonts" / "ibm_vga_8x12.png"
    out.parent.mkdir(parents=True, exist_ok=True)
    ea.write_png(out, rows)
    print(f"{out}: {len(cells)} glyphs 8x12")


if __name__ == "__main__":
    main()
