//! Device-independent bitmaps (BITMAPINFOHEADER, uncompressed, 1/4/8 bpp):
//! the game's RT_BITMAP sprites and RT_ICON images, decoded as
//! `tools/extract_assets.py` (`Dib`) decodes them.

use super::bytes::{i32_at, slice, u16_at, u32_at, u8_at, Result};
use super::Rgba;

struct Dib<'a> {
    buf: &'a [u8],
    width: usize,
    height: usize,
    bpp: usize,
    top_down: bool,
    palette: Vec<[u8; 3]>,
    pixels: usize,
    row_bytes: usize,
}

impl<'a> Dib<'a> {
    fn parse(buf: &'a [u8]) -> Result<Self> {
        let hdr = u32_at(buf, 0)? as usize;
        if hdr < 40 {
            return Err("unsupported bitmap header".to_owned());
        }
        let width = i32_at(buf, 4)?;
        let height = i32_at(buf, 8)?;
        let bpp = u16_at(buf, 14)? as usize;
        if u32_at(buf, 16)? != 0 {
            return Err("compressed bitmap".to_owned());
        }
        if !matches!(bpp, 1 | 4 | 8) || width <= 0 || width > 4096 || height == 0 || height.abs() > 4096 {
            return Err(format!("unsupported bitmap ({width}x{height}, {bpp} bpp)"));
        }
        let mut n_pal = u32_at(buf, 32)? as usize;
        if n_pal == 0 {
            n_pal = 1 << bpp;
        }
        let palette = (0..n_pal.min(256))
            .map(|i| {
                let o = hdr + i * 4;
                Ok([u8_at(buf, o + 2)?, u8_at(buf, o + 1)?, u8_at(buf, o)?])
            })
            .collect::<Result<Vec<_>>>()?;
        let width = width as usize;
        Ok(Dib {
            buf,
            width,
            height: height.unsigned_abs() as usize,
            bpp,
            top_down: height < 0,
            palette,
            pixels: hdr + n_pal * 4,
            row_bytes: (width * bpp).div_ceil(32) * 4,
        })
    }

    /// Stored row of logical row `y` (0 = top).
    fn row(&self, y: usize) -> usize {
        if self.top_down {
            y
        } else {
            self.height - 1 - y
        }
    }

    fn index(&self, x: usize, y: usize) -> Result<usize> {
        let base = self.pixels + self.row(y) * self.row_bytes;
        Ok(match self.bpp {
            1 => (u8_at(self.buf, base + x / 8)? >> (7 - x % 8)) as usize & 1,
            4 => {
                let b = u8_at(self.buf, base + x / 2)?;
                (if x.is_multiple_of(2) { b >> 4 } else { b & 0xF }) as usize
            }
            _ => u8_at(self.buf, base + x)? as usize,
        })
    }

    /// RGBA rows top-down; pixels of colour `transparent` get alpha 0.
    fn to_rgba(&self, transparent: Option<[u8; 3]>) -> Result<Rgba> {
        let mut data = Vec::with_capacity(self.width * self.height * 4);
        for y in 0..self.height {
            for x in 0..self.width {
                let rgb = self.palette.get(self.index(x, y)?).copied().unwrap_or([0, 0, 0]);
                let a = if Some(rgb) == transparent { 0 } else { 255 };
                data.extend_from_slice(&[rgb[0], rgb[1], rgb[2], a]);
            }
        }
        Ok(Rgba { width: self.width as u32, height: self.height as u32, data })
    }
}

/// A sprite: white (255,255,255) is the game's transparency colour key
/// (FUN_00405ab0 builds its masks from it).
pub fn decode_sprite(buf: &[u8]) -> Result<Rgba> {
    Dib::parse(buf)?.to_rgba(Some([255, 255, 255]))
}

/// An icon image: the XOR bitmap and AND mask are stacked, so the header
/// height is doubled; mask bit 1 = transparent.
pub fn decode_icon(buf: &[u8]) -> Result<Rgba> {
    let mut dib = Dib::parse(buf)?;
    dib.height /= 2;
    let mut img = dib.to_rgba(None)?;
    let mask_row = dib.width.div_ceil(32) * 4;
    let base = dib.pixels + dib.height * dib.row_bytes;
    slice(buf, base, mask_row * dib.height)?;
    for y in 0..dib.height {
        let o = base + dib.row(y) * mask_row;
        for x in 0..dib.width {
            if (buf[o + x / 8] >> (7 - x % 8)) & 1 == 1 {
                img.data[(y * dib.width + x) * 4 + 3] = 0;
            }
        }
    }
    Ok(img)
}
