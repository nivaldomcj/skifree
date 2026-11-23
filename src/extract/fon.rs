//! The status display's font from a Windows `.fon` (an NE file of FNT
//! resources): the 12-pixel face of `vgaoem.fon` ("Terminal", the stock
//! OEM_FIXED_FONT), as `tools/extract_assets.py` (`extract_oem_font`) cuts
//! it: printable ASCII (0x20..0x7E) in one strip of fixed-width cells,
//! black glyph pixels on transparent.

use super::bytes::{slice, u16_at, u32_at, u8_at, Result};
use super::Rgba;

const RT_FONT: u16 = 8;

/// The FNT resources of an NE file.
fn fonts(data: &[u8]) -> Result<Vec<&[u8]>> {
    let ne = u32_at(data, 0x3C)? as usize;
    if slice(data, ne, 2)? != b"NE" {
        return Err("not an NE file".to_owned());
    }
    let table = ne + u16_at(data, ne + 0x24)? as usize;
    let shift = u16_at(data, table)? as u32;
    if shift > 16 {
        return Err("font file is damaged".to_owned());
    }
    let mut p = table + 2;
    let mut out = Vec::new();
    while u16_at(data, p)? != 0 {
        let (rtype, count) = (u16_at(data, p)?, u16_at(data, p + 2)? as usize);
        p += 8;
        for _ in 0..count {
            let off = (u16_at(data, p)? as usize) << shift;
            let len = (u16_at(data, p + 2)? as usize) << shift;
            if rtype == 0x8000 | RT_FONT {
                out.push(slice(data, off, len)?);
            }
            p += 12;
        }
    }
    Ok(out)
}

/// One raster FNT (v2/v3) face as a glyph strip, if it is `height` pixels
/// high and fixed-pitch over 0x20..0x7E.
fn strip(fnt: &[u8], want_height: usize) -> Result<Option<Rgba>> {
    let version = u16_at(fnt, 0)?;
    let height = u16_at(fnt, 88)? as usize;
    let (first, last) = (u8_at(fnt, 95)? as usize, u8_at(fnt, 96)? as usize);
    if height != want_height || first > 0x20 || last < 0x7E {
        return Ok(None);
    }
    let (table, entry) = if version == 0x300 { (148, 6) } else { (118, 4) };
    let glyph = |code: usize| -> Result<(usize, usize)> {
        let e = table + (code - first) * entry;
        let width = u16_at(fnt, e)? as usize;
        let off = if entry == 6 { u32_at(fnt, e + 2)? as usize } else { u16_at(fnt, e + 2)? as usize };
        Ok((width, off))
    };
    let cell = glyph(0x20)?.0;
    if cell == 0 {
        return Ok(None);
    }
    let codes = 0x20..0x7F;
    let mut data = vec![0u8; codes.len() * cell * height * 4];
    let row_len = codes.len() * cell;
    for (i, code) in codes.enumerate() {
        let (width, off) = glyph(code)?;
        if width != cell {
            return Err("not a fixed-pitch font".to_owned());
        }
        for y in 0..height {
            for x in 0..width {
                let byte = u8_at(fnt, off + (x / 8) * height + y)?;
                let o = (y * row_len + i * cell + x) * 4;
                data[o + 3] = if (byte >> (7 - x % 8)) & 1 == 1 { 255 } else { 0 };
            }
        }
    }
    Ok(Some(Rgba { width: row_len as u32, height: height as u32, data }))
}

/// The 12-pixel face of a `.fon`, or None when it has none.
pub fn oem_font(data: &[u8]) -> Result<Option<Rgba>> {
    for fnt in fonts(data)? {
        if let Some(img) = strip(fnt, 12)? {
            return Ok(Some(img));
        }
    }
    Ok(None)
}
