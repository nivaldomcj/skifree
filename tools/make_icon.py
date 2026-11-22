#!/usr/bin/env python3
"""Generate the macOS iconset for SkiFree.app from the extracted assets.

Style follows the Big Sur+ app-icon conventions (see CuaDriver/ProtonVPN):
a white squircle (824/1024 of the canvas, centered) with the game's
colorful skier glyph (assets/icons/icon_5.png) centered inside it.

Everything here is computed at build time from assets extracted from the
user's own ski32.exe — no derived icon file is ever committed to the repo.

Usage: python3 tools/make_icon.py <out.iconset-dir>
"""
import os
import struct
import sys
import zlib

GLYPH_PATH = os.path.join("assets", "icons", "icon_3.png")
CANVAS = 1024
SQUIRCLE_SIZE = 824  # 80.5% of canvas, per Apple HIG geometry
SQUIRCLE_RADIUS = 185  # ~22.5% corner radius
GLYPH_SIZE = 660  # ~80% of the squircle

SIZES = [16, 32, 128, 256, 512]


def decode_png(path):
    """Minimal RGBA PNG decoder (stdlib only)."""
    data = open(path, "rb").read()
    pos = 8
    idat = b""
    w = h = ctype = None
    while pos < len(data):
        ln, typ = struct.unpack(">I4s", data[pos:pos + 8])
        if typ == b"IHDR":
            w, h, _depth, ctype = struct.unpack(">IIBB", data[pos + 8:pos + 18])
        elif typ == b"IDAT":
            idat += data[pos + 8:pos + 8 + ln]
        pos += 12 + ln
    if ctype != 6:
        raise SystemExit(f"{path}: expected RGBA PNG (color type 6), got {ctype}")
    raw = zlib.decompress(idat)
    stride = w * 4
    px = bytearray(w * h * 4)
    prev = bytearray(stride)
    p = 0
    for y in range(h):
        f = raw[p]
        p += 1
        line = bytearray(raw[p:p + stride])
        p += stride
        for x in range(stride):
            a = line[x - 4] if x >= 4 else 0
            b = prev[x]
            c = prev[x - 4] if x >= 4 else 0
            if f == 1:
                line[x] = (line[x] + a) & 255
            elif f == 2:
                line[x] = (line[x] + b) & 255
            elif f == 3:
                line[x] = (line[x] + (a + b) // 2) & 255
            elif f == 4:
                pp = a + b - c
                pa, pb, pc = abs(pp - a), abs(pp - b), abs(pp - c)
                pr = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                line[x] = (line[x] + pr) & 255
        px[y * stride:(y + 1) * stride] = line
        prev = line
    return w, h, px


def encode_png(w, h, px):
    """Minimal RGBA PNG encoder."""
    raw = bytearray()
    for y in range(h):
        raw.append(0)
        raw += px[y * w * 4:(y + 1) * w * 4]
    def chunk(typ, payload):
        c = struct.pack(">I", len(payload)) + typ + payload
        return c + struct.pack(">I", zlib.crc32(typ + payload) & 0xFFFFFFFF)
    ihdr = struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr)
            + chunk(b"IDAT", zlib.compress(bytes(raw), 9)) + chunk(b"IEND", b""))


def rounded_rect_dist(x, y, left, top, size, radius):
    """Signed distance from (x, y) to a rounded rect (negative = inside)."""
    cx = min(max(x, left + radius), left + size - radius)
    cy = min(max(y, top + radius), top + size - radius)
    dx = x - cx
    dy = y - cy
    if left + radius <= x <= left + size - radius and top <= y <= top + size:
        return -min(radius, min(x - left, left + size - x))
    if top + radius <= y <= top + size - radius and left <= x <= left + size:
        return -min(radius, min(y - top, top + size - y))
    return (dx * dx + dy * dy) ** 0.5 - radius


def flood_transparent(w, h, px):
    """Make the glyph's connected background transparent: flood from the
    borders over pure-black and already-transparent pixels, so interior
    black outlines survive (the source .ico has an opaque black backdrop).
    """
    from collections import deque
    seen = set()
    q = deque()
    for x in range(w):
        q.append((x, 0))
        q.append((x, h - 1))
    for y in range(h):
        q.append((0, y))
        q.append((w - 1, y))
    while q:
        x, y = q.popleft()
        if (x, y) in seen:
            continue
        i = (y * w + x) * 4
        if px[i + 3] != 0 and (px[i], px[i + 1], px[i + 2]) != (0, 0, 0):
            continue
        seen.add((x, y))
        px[i + 3] = 0
        for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            nx, ny = x + dx, y + dy
            if 0 <= nx < w and 0 <= ny < h and (nx, ny) not in seen:
                q.append((nx, ny))


def render(size, glyph_w, glyph_h, glyph_px):
    """Render the icon at `size` x `size` (supersampled from the 1024 design)."""
    scale = CANVAS / size
    out = bytearray(size * size * 4)
    margin = (CANVAS - SQUIRCLE_SIZE) / 2  # 100
    g_off = (CANVAS - GLYPH_SIZE) / 2  # 256
    for y in range(size):
        for x in range(size):
            # 2x2 supersampling for smooth squircle edges
            alpha_acc = 0.0
            for sy in (0.25, 0.75):
                for sx in (0.25, 0.75):
                    cx = (x + sx) * scale
                    cy = (y + sy) * scale
                    d = rounded_rect_dist(cx, cy, margin, margin, SQUIRCLE_SIZE, SQUIRCLE_RADIUS)
                    alpha_acc += 1.0 if d < 0 else 0.0
            a_sq = alpha_acc / 4.0
            r = g = b = a = 0.0
            if a_sq > 0:
                r, g, b = 255.0, 255.0, 255.0
                a = a_sq
            # Glyph (nearest-neighbor, preserves pixel art)
            gx = (x + 0.5) * scale
            gy = (y + 0.5) * scale
            if g_off <= gx < g_off + GLYPH_SIZE and g_off <= gy < g_off + GLYPH_SIZE:
                sx = int((gx - g_off) / GLYPH_SIZE * glyph_w)
                sy = int((gy - g_off) / GLYPH_SIZE * glyph_h)
                o = (sy * glyph_w + sx) * 4
                ga = glyph_px[o + 3] / 255.0
                if ga > 0:
                    r = glyph_px[o] * ga + r * (1 - ga)
                    g = glyph_px[o + 1] * ga + g * (1 - ga)
                    b = glyph_px[o + 2] * ga + b * (1 - ga)
                    a = ga + a_sq * (1 - ga)
            i = (y * size + x) * 4
            out[i] = int(r + 0.5)
            out[i + 1] = int(g + 0.5)
            out[i + 2] = int(b + 0.5)
            out[i + 3] = int(a * 255 + 0.5)
    return out


def main():
    out_dir = sys.argv[1]
    os.makedirs(out_dir, exist_ok=True)
    gw, gh, gpx = decode_png(GLYPH_PATH)
    flood_transparent(gw, gh, gpx)
    for size in SIZES:
        px = render(size, gw, gh, gpx)
        name = f"icon_{size}x{size}.png"
        with open(os.path.join(out_dir, name), "wb") as f:
            f.write(encode_png(size, size, px))
        # @2x variant of this slot
        px2 = render(size * 2, gw, gh, gpx)
        with open(os.path.join(out_dir, f"icon_{size}x{size}@2x.png"), "wb") as f:
            f.write(encode_png(size * 2, size * 2, px2))
    print(f"iconset written to {out_dir}")


if __name__ == "__main__":
    main()
