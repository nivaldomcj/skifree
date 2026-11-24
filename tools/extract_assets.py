#!/usr/bin/env python3
"""Extract assets (sprites, icons, strings) from ski32.exe PE resources.

Stdlib only. Usage: python3 tools/extract_assets.py [path-to-exe]
Default input: auto-discovers originals/*.exe (not committed — supply your own copy).
Writes assets/{sprites,icons}/, assets/meta.json and assets/strings.json.
If originals/vgaoem.fon (Windows' "Terminal" font, the stock OEM_FIXED_FONT
the status display uses; copy it from C:\\Windows\\Fonts) is present, also
writes assets/fonts/oem_fixed.png: printable ASCII in 8x12 cells. Without
it the port draws the status display with a fallback font.
If the exe has WAVE resources (foone's ski32sounds.exe does; the shipped
ski32.exe doesn't), writes the nine the game plays to
assets/sounds/sound_1.wav .. sound_9.wav. When the chosen exe has none,
another originals/*.exe that has them is used for the sounds only. A
build with assets/sounds/ has sound; delete the folder for a silent one.
Idempotent: same input produces identical output bytes.
"""

import hashlib
import json
import struct
import sys
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "assets"

RT_BITMAP = 2
RT_ICON = 3
RT_STRING = 6
RT_GROUP_ICON = 14


# ---------------------------------------------------------------- PE parsing

def parse_pe_resources(data):
    """Return list of resources: (type, name, lang, offset, size)."""
    u16 = lambda o: struct.unpack_from("<H", data, o)[0]
    u32 = lambda o: struct.unpack_from("<I", data, o)[0]

    if data[0:2] != b"MZ":
        sys.exit("error: not an MZ executable (missing 'MZ' DOS header)")
    pe = u32(0x3C)
    sig = data[pe:pe + 2]
    if sig == b"NE":
        sys.exit(
            "error: 16-bit NE executable (Windows 3.x) has no PE resources.\n"
            "  This tool extracts from the 32-bit build — use originals/ski32.exe."
        )
    if sig == b"LE" or sig == b"LX":
        sys.exit("error: LE/LX executable (OS/2 / VxD) is not supported; use ski32.exe.")
    if data[pe:pe + 4] != b"PE\0\0":
        sys.exit("error: not a PE file (unrecognized executable format); use ski32.exe.")
    n_sections = u16(pe + 6)
    opt_size = u16(pe + 20)
    opt = pe + 24
    assert u16(opt) == 0x10B, "expected PE32"
    rsrc_rva = u32(opt + 96 + 2 * 8)

    sections = []
    sec_off = opt + opt_size
    for i in range(n_sections):
        o = sec_off + i * 40
        sections.append({
            "name": data[o:o + 8].rstrip(b"\0").decode("ascii", "replace"),
            "vsize": u32(o + 8), "vaddr": u32(o + 12),
            "rawsize": u32(o + 16), "rawptr": u32(o + 20),
        })

    def rva2off(rva):
        for s in sections:
            if s["vaddr"] <= rva < s["vaddr"] + max(s["vsize"], s["rawsize"]):
                return s["rawptr"] + (rva - s["vaddr"])
        raise ValueError(f"RVA 0x{rva:x} not mapped")

    rsrc_off = rva2off(rsrc_rva)

    def read_name(raw):
        if raw & 0x80000000:
            o = rsrc_off + (raw & 0x7FFFFFFF)
            n = u16(o)
            return ("string", data[o + 2:o + 2 + n * 2].decode("utf-16le"))
        return ("id", raw & 0xFFFF)

    resources = []

    def walk(dir_rel, path):
        o = rsrc_off + dir_rel
        n_named, n_ids = u16(o + 12), u16(o + 14)
        entries = o + 16
        for i in range(n_named + n_ids):
            e = entries + i * 8
            name = read_name(u32(e))
            target = u32(e + 4)
            if target & 0x80000000:
                walk(target & 0x7FFFFFFF, path + [name])
            else:
                de = rsrc_off + target
                data_rva, size = u32(de), u32(de + 4)
                resources.append((path[0], path[1], path[2] if len(path) > 2 else ("id", 0),
                                  rva2off(data_rva), size))

    walk(0, [])
    return resources


# ---------------------------------------------------------------- sounds

# FUN_004052d0 loads WAVE resources 1-9; others (10, 11 in ski32sounds.exe)
# are never played.
GAME_SOUNDS = range(1, 10)


def wave_resources(data):
    """{id: RIFF bytes} for the exe's WAVE resources (the game's nine)."""
    waves = {}
    for rtype, rname, _lang, off, size in parse_pe_resources(data):
        if rtype == ("string", "WAVE") and rname[0] == "id" and rname[1] in GAME_SOUNDS:
            payload = data[off:off + size]
            if payload[:4] == b"RIFF" and payload[8:12] == b"WAVE":
                waves[rname[1]] = payload
    return waves


def extract_sounds(exe, data):
    """Write assets/sounds/sound_<id>.wav from `exe`, or from another
    originals/*.exe when it has no WAVE resources."""
    waves, source = wave_resources(data), exe
    if not waves:
        originals = ROOT / "originals"
        others = sorted(p for p in originals.glob("*")
                        if p.suffix.lower() == ".exe" and p.resolve() != exe.resolve()) \
            if originals.is_dir() else []
        for other in others:
            try:
                found = wave_resources(other.read_bytes())
            except (SystemExit, ValueError, AssertionError, struct.error):
                continue  # not a PE32 SkiFree exe
            if found:
                waves, source = found, other
                break
    if not waves:
        print("sounds:  none (no exe with WAVE resources; the build is silent)")
        return
    out = OUT / "sounds"
    out.mkdir(parents=True, exist_ok=True)
    for rid, payload in sorted(waves.items()):
        (out / f"sound_{rid}.wav").write_bytes(payload)
    missing = [i for i in GAME_SOUNDS if i not in waves]
    note = f", missing {missing}" if missing else ""
    print(f"sounds:  {len(waves)} WAVE resources from {source.name}{note}")


# ---------------------------------------------------------------- DIB decode

class Dib:
    def __init__(self, buf):
        (self.hdr_size,) = struct.unpack_from("<I", buf, 0)
        self.width = struct.unpack_from("<i", buf, 4)[0]
        self.height = struct.unpack_from("<i", buf, 8)[0]
        self.planes, self.bpp = struct.unpack_from("<HH", buf, 12)
        self.compression = struct.unpack_from("<I", buf, 16)[0]
        assert self.compression == 0, "compressed DIB unsupported"
        n_pal = 0
        if self.bpp <= 8:
            n_pal = struct.unpack_from("<I", buf, 32)[0] if self.hdr_size >= 40 else 0
            if n_pal == 0:
                n_pal = 1 << self.bpp
        self.palette = []
        for i in range(n_pal):
            b, g, r, _ = struct.unpack_from("<BBBB", buf, self.hdr_size + i * 4)
            self.palette.append((r, g, b))
        self.pixels_off = self.hdr_size + n_pal * 4
        self.buf = buf
        self.top_down = self.height < 0
        self.height = abs(self.height)
        self.row_bytes = ((self.width * self.bpp + 31) // 32) * 4

    def pixel(self, x, y):
        """Palette index at logical (x, y), y=0 at top."""
        row = y if self.top_down else self.height - 1 - y
        base = self.pixels_off + row * self.row_bytes
        if self.bpp == 1:
            byte = self.buf[base + x // 8]
            return (byte >> (7 - x % 8)) & 1
        if self.bpp == 4:
            byte = self.buf[base + x // 2]
            return (byte >> 4) & 0xF if x % 2 == 0 else byte & 0xF
        if self.bpp == 8:
            return self.buf[base + x]
        raise ValueError(f"unsupported bpp {self.bpp}")

    def to_rgba(self, transparent=None):
        """Rows of (r,g,b,a). transparent: (r,g,b) color key made fully transparent."""
        out = []
        for y in range(self.height):
            row = []
            for x in range(self.width):
                idx = self.pixel(x, y)
                r, g, b = self.palette[idx] if idx < len(self.palette) else (0, 0, 0)
                a = 0 if transparent is not None and (r, g, b) == transparent else 255
                row.append((r, g, b, a))
            out.append(row)
        return out

    def and_mask_alpha(self):
        """Icon AND mask (1bpp, stored after the XOR rows). True = transparent."""
        row_bytes = ((self.width + 31) // 32) * 4
        base = self.pixels_off + self.height * self.row_bytes
        out = []
        for y in range(self.height):
            row = y if self.top_down else self.height - 1 - y
            o = base + row * row_bytes
            line = []
            for x in range(self.width):
                line.append((self.buf[o + x // 8] >> (7 - x % 8)) & 1 == 1)
            out.append(line)
        return out

    def to_rgba_icon(self):
        rows = self.to_rgba()
        mask = self.and_mask_alpha()
        return [[(r, g, b, 0 if mask[y][x] else a)
                 for x, (r, g, b, a) in enumerate(row)]
                for y, row in enumerate(rows)]


# ------------------------------------------------------------ FON (NE) fonts

RT_FONT = 8


def parse_fon_fonts(data):
    """FNT resources of a Windows .fon (NE executable): list of bytes."""
    u16 = lambda o: struct.unpack_from("<H", data, o)[0]
    ne = struct.unpack_from("<I", data, 0x3C)[0]
    if data[ne:ne + 2] != b"NE":
        raise SystemExit("not an NE .fon file")
    rt = ne + u16(ne + 0x24)
    shift = u16(rt)
    p = rt + 2
    fonts = []
    while u16(p):
        rtype, count = u16(p), u16(p + 2)
        p += 8
        for _ in range(count):
            off, length = u16(p) << shift, u16(p + 2) << shift
            if rtype == 0x8000 | RT_FONT:
                fonts.append(data[off:off + length])
            p += 12
    return fonts


def fnt_glyph_rows(fnt):
    """Decode a raster FNT (v2/v3): metrics and {char: rows of 0/1}."""
    u16 = lambda o: struct.unpack_from("<H", fnt, o)[0]
    version = u16(0)
    height, first, last = u16(88), fnt[95], fnt[96]
    ascent = u16(74)
    table, entry = (148, 6) if version == 0x300 else (118, 4)
    glyphs = {}
    for i, code in enumerate(range(first, last + 1)):
        e = table + i * entry
        width = u16(e)
        off = struct.unpack_from("<I" if entry == 6 else "<H", fnt, e + 2)[0]
        rows = []
        for y in range(height):
            row = []
            for x in range(width):
                byte = fnt[off + (x // 8) * height + y]
                row.append((byte >> (7 - x % 8)) & 1)
            rows.append(row)
        glyphs[code] = rows
    return {"height": height, "ascent": ascent, "first": first, "last": last}, glyphs


def extract_oem_font(fon_path):
    """assets/fonts/oem_fixed.png from the 12-pixel face of vgaoem.fon."""
    fonts = parse_fon_fonts(fon_path.read_bytes())
    faces = [fnt_glyph_rows(f) for f in fonts]
    face = next((f for f in faces if f[0]["height"] == 12), None)
    if face is None:
        print(f"{fon_path.name}: no 12-pixel face, font skipped")
        return
    info, glyphs = face
    cells = [glyphs[c] for c in range(0x20, 0x7F)]
    width = len(cells[0][0])
    assert all(len(g[0]) == width for g in cells), "not a fixed-pitch face"
    rows = [[(0, 0, 0, 255) if g[y][x] else (0, 0, 0, 0) for g in cells for x in range(width)]
            for y in range(info["height"])]
    (OUT / "fonts").mkdir(parents=True, exist_ok=True)
    write_png(OUT / "fonts" / "oem_fixed.png", rows)
    print(f"font:    {fon_path.name} -> fonts/oem_fixed.png ({len(cells)} glyphs "
          f"{width}x{info['height']}, ascent {info['ascent']})")


def write_png(path, rows):
    h, w = len(rows), len(rows[0])
    raw = b"".join(b"\0" + b"".join(bytes(px) for px in row) for row in rows)

    def chunk(tag, payload):
        c = struct.pack(">I", len(payload)) + tag + payload
        return c + struct.pack(">I", zlib.crc32(tag + payload) & 0xFFFFFFFF)

    png = (b"\x89PNG\r\n\x1a\n"
           + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
           + chunk(b"IDAT", zlib.compress(raw, 9))
           + chunk(b"IEND", b""))
    path.write_bytes(png)


# ---------------------------------------------------------------- extraction

def sha256(b):
    return hashlib.sha256(b).hexdigest()


def find_exe() -> Path:
    """Resolve the input exe: explicit argv path, else auto-discover originals/*.exe."""
    if len(sys.argv) > 1:
        exe = Path(sys.argv[1])
        if not exe.is_file():
            sys.exit(f"error: no such file: {exe}")
        return exe

    originals = ROOT / "originals"
    candidates = sorted(
        p for p in originals.glob("*") if p.suffix.lower() == ".exe"
    ) if originals.is_dir() else []

    if not candidates:
        sys.exit(
            "error: no .exe found in originals/\n"
            "  Put your own ski32.exe (or any SkiFree exe) in the originals/ folder,\n"
            "  or pass a path: python3 tools/extract_assets.py /path/to/ski32.exe"
        )
    if len(candidates) == 1:
        return candidates[0]
    # Prefer ski32.exe when several exes are present.
    for p in candidates:
        if p.name.lower() == "ski32.exe":
            return p
    names = "\n".join(f"  {p.name}" for p in candidates)
    sys.exit(f"error: multiple .exe files in originals/, pick one explicitly:\n{names}")


def main():
    exe = find_exe()
    data = exe.read_bytes()
    resources = parse_pe_resources(data)
    print(f"{exe.name}: {len(resources)} resources")

    (OUT / "sprites").mkdir(parents=True, exist_ok=True)
    (OUT / "icons").mkdir(parents=True, exist_ok=True)

    meta = {"source": exe.name, "source_sha256": sha256(data), "resources": []}
    bitmaps = {}
    icons = {}
    groups = []

    for rtype, rname, _lang, off, size in resources:
        payload = data[off:off + size]
        entry = {"type": rtype[1] if rtype[0] == "id" else rtype[1],
                 "id": rname[1], "offset": off, "size": size, "sha256": sha256(payload)}
        # Sprites and icons are written under their numeric id: a string
        # name would go into a file path unchecked (`icon_../../x.png`).
        if rtype == ("id", RT_BITMAP) and rname[0] == "id":
            dib = Dib(payload)
            entry.update(width=dib.width, height=dib.height, bpp=dib.bpp)
            bitmaps[rname[1]] = dib
        elif rtype == ("id", RT_ICON) and rname[0] == "id":
            dib = Dib(payload)
            dib.height //= 2  # icon DIBs store XOR+AND stacked
            entry.update(width=dib.width, height=dib.height, bpp=dib.bpp)
            icons[rname[1]] = (payload, dib)
        elif rtype == ("id", RT_GROUP_ICON):
            groups.append(payload)
        elif rtype == ("id", RT_STRING):
            strings = []
            p, end = 0, len(payload)
            i = 0
            while p < end:
                (ln,) = struct.unpack_from("<H", payload, p)
                p += 2
                strings.append(payload[p:p + ln * 2].decode("utf-16le"))
                p += ln * 2
                i += 1
            entry["strings"] = strings
        meta["resources"].append(entry)

    # Sprites: white (255,255,255) is the game's transparency color key.
    # FUN_00405ab0 builds a monochrome mask per bitmap via color->mono BitBlt
    # (white maps to mask 0) and FUN_00401540 draws with SRCPAINT+SRCAND.
    for iid in sorted(bitmaps):
        dib = bitmaps[iid]
        rows = dib.to_rgba(transparent=(255, 255, 255))
        write_png(OUT / "sprites" / f"bitmap_{iid:02d}.png", rows)
    for e in meta["resources"]:
        if e.get("type") == RT_BITMAP and "bpp" in e:
            e["file"] = f"sprites/bitmap_{e['id']:02d}.png"
            e["transparent_color"] = [255, 255, 255]

    # Icons: .ico per group + PNG of each individual icon (AND mask -> alpha)
    for iid, (payload, dib) in sorted(icons.items()):
        rows = dib.to_rgba_icon()
        write_png(OUT / "icons" / f"icon_{iid}.png", rows)
    for n, grp in enumerate(groups, 1):
        count = struct.unpack_from("<H", grp, 4)[0]
        entries = []
        blobs = []
        blob_off = 6 + count * 16
        for i in range(count):
            e = 6 + i * 14
            w, h, cc, _res, planes, bpp, _size, rid = struct.unpack_from("<BBBBHHIH", grp, e)
            payload, _ = icons[rid]
            entries.append(struct.pack("<BBBBHHII", w, h, cc, 0, planes, bpp, len(payload), blob_off))
            blobs.append(payload)
            blob_off += len(payload)
        ico = struct.pack("<HHH", 0, 1, count) + b"".join(entries) + b"".join(blobs)
        (OUT / "icons" / f"app_{n}.ico").write_bytes(ico)

    (OUT / "meta.json").write_text(json.dumps(meta, indent=2) + "\n")

    strings_out = {str(e["id"]): e["strings"] for e in meta["resources"] if "strings" in e}
    (OUT / "strings.json").write_text(json.dumps(strings_out, indent=2, ensure_ascii=False) + "\n")

    n_pairs = len(bitmaps)
    print(f"bitmaps: {len(bitmaps)} 4bpp sprites (white color key -> alpha)")
    print(f"icons:   {len(icons)} icons, {len(groups)} groups")
    fon = ROOT / "originals" / "vgaoem.fon"
    if fon.exists():
        extract_oem_font(fon)
    extract_sounds(exe, data)
    print(f"output:  {OUT}")


if __name__ == "__main__":
    main()
