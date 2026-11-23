//! The resource directory of a 32-bit Windows executable (PE32), as
//! `tools/extract_assets.py` (`parse_pe_resources`) walks it.

use super::bytes::{slice, u16_at, u32_at, Result};

pub const RT_BITMAP: u16 = 2;
pub const RT_ICON: u16 = 3;

/// A resource type or name: a number or a string.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Name {
    Id(u16),
    Str(String),
}

#[derive(Debug)]
pub struct Resource<'a> {
    pub rtype: Name,
    pub name: Name,
    pub data: &'a [u8],
}

/// What kind of executable `data` is, from its headers.
#[derive(Debug, PartialEq, Eq)]
pub enum Kind {
    /// PE: a 32-bit (or later) Windows program.
    Pe,
    /// NE: a 16-bit Windows program, or a .fon font file.
    Ne,
    /// Anything else.
    Other,
}

pub fn kind(data: &[u8]) -> Kind {
    if data.get(0..2) != Some(b"MZ") {
        return Kind::Other;
    }
    let Ok(header) = u32_at(data, 0x3C) else { return Kind::Other };
    let header = header as usize;
    match data.get(header..header + 4) {
        Some(b"PE\0\0") => Kind::Pe,
        Some(sig) if &sig[..2] == b"NE" => Kind::Ne,
        _ => Kind::Other,
    }
}

struct Section {
    vaddr: u32,
    vsize: u32,
    rawsize: u32,
    rawptr: u32,
}

/// Every resource of a PE32 file, in directory order.
///
/// # Errors
///
/// When `data` is not a 32-bit Windows program or its resource directory is
/// cut short or malformed.
pub fn resources(data: &[u8]) -> Result<Vec<Resource<'_>>> {
    if kind(data) != Kind::Pe {
        return Err("not a 32-bit Windows program".to_owned());
    }
    let pe = u32_at(data, 0x3C)? as usize;
    let n_sections = u16_at(data, pe + 6)? as usize;
    let opt_size = u16_at(data, pe + 20)? as usize;
    let opt = pe + 24;
    if u16_at(data, opt)? != 0x10B {
        return Err("not a 32-bit (PE32) Windows program".to_owned());
    }
    let rsrc_rva = u32_at(data, opt + 96 + 2 * 8)?;
    let mut sections = Vec::with_capacity(n_sections);
    for i in 0..n_sections {
        let o = opt + opt_size + i * 40;
        sections.push(Section {
            vsize: u32_at(data, o + 8)?,
            vaddr: u32_at(data, o + 12)?,
            rawsize: u32_at(data, o + 16)?,
            rawptr: u32_at(data, o + 20)?,
        });
    }
    let rva2off = |rva: u32| -> Result<usize> {
        sections
            .iter()
            .find(|s| s.vaddr <= rva && rva - s.vaddr < s.vsize.max(s.rawsize))
            .map(|s| (s.rawptr + (rva - s.vaddr)) as usize)
            .ok_or_else(|| format!("resource address 0x{rva:x} is outside the file"))
    };
    if rsrc_rva == 0 {
        return Ok(Vec::new());
    }
    let rsrc = rva2off(rsrc_rva)?;

    let read_name = |raw: u32| -> Result<Name> {
        if raw & 0x8000_0000 != 0 {
            let o = rsrc + (raw & 0x7FFF_FFFF) as usize;
            let n = u16_at(data, o)? as usize;
            let units: Vec<u16> =
                slice(data, o + 2, n * 2)?.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
            Ok(Name::Str(String::from_utf16_lossy(&units)))
        } else {
            Ok(Name::Id(raw as u16))
        }
    };

    // The directory is three levels deep (type, name, language); a
    // malformed file could loop, so depth is capped.
    let mut out = Vec::new();
    let mut stack: Vec<(usize, Vec<Name>)> = vec![(0, Vec::new())];
    while let Some((dir, path)) = stack.pop() {
        if path.len() > 3 {
            return Err("resource directory is damaged".to_owned());
        }
        let o = rsrc + dir;
        let count = u16_at(data, o + 12)? as usize + u16_at(data, o + 14)? as usize;
        let mut children = Vec::new();
        for i in 0..count {
            let e = o + 16 + i * 8;
            let name = read_name(u32_at(data, e)?)?;
            let target = u32_at(data, e + 4)?;
            let mut child = path.clone();
            child.push(name);
            if target & 0x8000_0000 != 0 {
                children.push(((target & 0x7FFF_FFFF) as usize, child));
            } else {
                let de = rsrc + target as usize;
                let off = rva2off(u32_at(data, de)?)?;
                let size = u32_at(data, de + 4)? as usize;
                if child.len() < 2 {
                    return Err("resource directory is damaged".to_owned());
                }
                let mut it = child.into_iter();
                let (rtype, name) = (it.next().unwrap_or(Name::Id(0)), it.next().unwrap_or(Name::Id(0)));
                out.push(Resource { rtype, name, data: slice(data, off, size)? });
            }
        }
        // Depth-first in directory order, like the Python walker.
        stack.extend(children.into_iter().rev());
    }
    Ok(out)
}
