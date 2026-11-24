//! The resource directory of a 32-bit Windows executable (PE32), as
//! `tools/extract_assets.py` (`parse_pe_resources`) walks it.

use super::bytes::{at, slice, u16_at, u32_at, Result};

pub const RT_BITMAP: u16 = 2;
/// Most directory entries walked: ski32sounds.exe, the largest, has about
/// 130. Directories can share subdirectories, so without a budget a small
/// crafted file expands into billions of entries.
const MAX_ENTRIES: usize = 4096;
/// Longest resource name read, in UTF-16 units: the game matches only the
/// type "WAVE", and ski32.exe's other names are ICONSKI and ICONSKI2. Every
/// entry's path copies its names, so without a cap the entry budget could
/// still hold gigabytes of names.
const MAX_NAME: usize = 64;
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
            .ok_or_else(|| format!("resource address 0x{rva:x} is outside the file"))
            .and_then(|s| at(s.rawptr as usize, (rva - s.vaddr) as usize))
    };
    if rsrc_rva == 0 {
        return Ok(Vec::new());
    }
    let rsrc = rva2off(rsrc_rva)?;

    let read_name = |raw: u32| -> Result<Name> {
        if raw & 0x8000_0000 != 0 {
            let o = at(rsrc, (raw & 0x7FFF_FFFF) as usize)?;
            let n = u16_at(data, o)? as usize;
            if n > MAX_NAME {
                return Err("resource directory is damaged".to_owned());
            }
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
    let mut entries = 0usize;
    while let Some((dir, path)) = stack.pop() {
        if path.len() > 3 {
            return Err("resource directory is damaged".to_owned());
        }
        let o = at(rsrc, dir)?;
        let count = u16_at(data, o + 12)? as usize + u16_at(data, o + 14)? as usize;
        entries += count;
        if entries > MAX_ENTRIES {
            return Err("resource directory is damaged".to_owned());
        }
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
                let de = at(rsrc, target as usize)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A PE32 whose resource directory is three levels of `n` entries that
    /// all share their subdirectory: `n`³ resources from a few KB.
    fn shared_directory_pe(entries: u32) -> Vec<u8> {
        let (pe, rsrc) = (0x40usize, 0x200usize);
        let dir_len = 16 + 8 * entries as usize;
        let (names, langs, leaf) = (dir_len, 2 * dir_len, 3 * dir_len);
        let mut f = vec![0u8; rsrc + leaf + 16];
        let put16 = |f: &mut Vec<u8>, o: usize, v: u16| f[o..o + 2].copy_from_slice(&v.to_le_bytes());
        let put32 = |f: &mut Vec<u8>, o: usize, v: u32| f[o..o + 4].copy_from_slice(&v.to_le_bytes());
        f[..2].copy_from_slice(b"MZ");
        put32(&mut f, 0x3C, pe as u32);
        f[pe..pe + 4].copy_from_slice(b"PE\0\0");
        put16(&mut f, pe + 6, 1); // one section
        put16(&mut f, pe + 20, 0xE0); // optional header size
        let opt = pe + 24;
        put16(&mut f, opt, 0x10B);
        put32(&mut f, opt + 112, 0x1000); // resource directory RVA
        let sec = opt + 0xE0;
        let size = (f.len() - rsrc) as u32;
        put32(&mut f, sec + 8, size);
        put32(&mut f, sec + 12, 0x1000);
        put32(&mut f, sec + 16, size);
        put32(&mut f, sec + 20, rsrc as u32);
        for (dir, target) in
            [(0, 0x8000_0000 | names as u32), (names, 0x8000_0000 | langs as u32), (langs, leaf as u32)]
        {
            put16(&mut f, rsrc + dir + 14, entries as u16);
            for i in 0..entries as usize {
                put32(&mut f, rsrc + dir + 16 + i * 8, i as u32 + 1);
                put32(&mut f, rsrc + dir + 20 + i * 8, target);
            }
        }
        put32(&mut f, rsrc + leaf, 0x1000 + leaf as u32); // the data entry points at itself
        put32(&mut f, rsrc + leaf + 4, 4);
        f
    }

    #[test]
    fn small_shared_directory_parses() {
        assert_eq!(resources(&shared_directory_pe(4)).map(|r| r.len()), Ok(64));
    }

    #[test]
    fn builder_round_trips() {
        use super::testing::{build, Ty};
        let exe = build(&[(Ty::Id(2), 1, b"bmp"), (Ty::Str("WAVE"), 4, b"wav")]);
        let r = resources(&exe).unwrap();
        assert_eq!(r.len(), 2);
        assert!(matches!((&r[0].rtype, &r[0].name, r[0].data), (Name::Id(2), Name::Id(1), b"bmp")));
        assert!(matches!((&r[1].rtype, &r[1].name), (Name::Str(t), Name::Id(4)) if t == "WAVE"));
    }

    #[test]
    fn long_resource_names_are_refused() {
        use super::testing::{build, Ty};
        let long = "W".repeat(MAX_NAME + 1);
        assert!(resources(&build(&[(Ty::Str(&long), 1, b"x")])).is_err());
        assert!(resources(&build(&[(Ty::Str(&long[..MAX_NAME]), 1, b"x")])).is_ok());
    }

    #[test]
    fn shared_directory_bomb_is_refused() {
        // 200³ = 8 million resources without the entry budget.
        let f = shared_directory_pe(200);
        assert!(f.len() < 8 * 1024);
        assert_eq!(resources(&f).map(|r| r.len()), Err("resource directory is damaged".to_owned()));
    }
}

/// A minimal PE32 builder for tests: a resource directory with the given
/// (type, name id, data) entries, duplicates allowed, in order.
#[cfg(test)]
pub(crate) mod testing {
    /// A resource type: a numeric id, or a string such as "WAVE".
    #[derive(PartialEq)]
    pub enum Ty<'a> {
        Id(u16),
        Str(&'a str),
    }

    fn put16(f: &mut [u8], o: usize, v: u16) {
        f[o..o + 2].copy_from_slice(&v.to_le_bytes());
    }

    fn put32(f: &mut [u8], o: usize, v: u32) {
        f[o..o + 4].copy_from_slice(&v.to_le_bytes());
    }

    fn dir(r: &mut Vec<u8>, named: usize, ids: usize) -> usize {
        let o = r.len();
        r.resize(o + 16 + 8 * (named + ids), 0);
        put16(r, o + 12, named as u16);
        put16(r, o + 14, ids as u16);
        o
    }

    /// One resource type and its (name id, data) entries, in order.
    type Group<'a, 'b> = (&'a Ty<'b>, Vec<(u16, &'a [u8])>);

    pub fn build(resources: &[(Ty<'_>, u16, &[u8])]) -> Vec<u8> {
        let mut types: Vec<Group<'_, '_>> = Vec::new();
        for (ty, id, data) in resources {
            match types.iter_mut().find(|(t, _)| *t == ty) {
                Some((_, items)) => items.push((*id, data)),
                None => types.push((ty, vec![(*id, data)])),
            }
        }
        let named = types.iter().filter(|(t, _)| matches!(t, Ty::Str(_))).count();
        let mut r = Vec::new();
        let root = dir(&mut r, named, types.len() - named);
        for (ti, (ty, items)) in types.iter().enumerate() {
            let type_dir = dir(&mut r, 0, items.len());
            let raw = match ty {
                Ty::Id(id) => u32::from(*id),
                Ty::Str(s) => {
                    let o = r.len();
                    let units: Vec<u16> = s.encode_utf16().collect();
                    r.extend_from_slice(&(units.len() as u16).to_le_bytes());
                    for u in units {
                        r.extend_from_slice(&u.to_le_bytes());
                    }
                    0x8000_0000 | o as u32
                }
            };
            put32(&mut r, root + 16 + ti * 8, raw);
            put32(&mut r, root + 20 + ti * 8, 0x8000_0000 | type_dir as u32);
            for (ii, (id, data)) in items.iter().enumerate() {
                let lang = dir(&mut r, 0, 1);
                put32(&mut r, type_dir + 16 + ii * 8, u32::from(*id));
                put32(&mut r, type_dir + 20 + ii * 8, 0x8000_0000 | lang as u32);
                let entry = r.len();
                r.resize(entry + 16, 0);
                put32(&mut r, lang + 16, 0x409);
                put32(&mut r, lang + 20, entry as u32);
                let at = r.len();
                r.extend_from_slice(data);
                put32(&mut r, entry, 0x1000 + at as u32);
                put32(&mut r, entry + 4, data.len() as u32);
            }
        }
        let (pe, rsrc) = (0x40usize, 0x200usize);
        let mut f = vec![0u8; rsrc];
        f[..2].copy_from_slice(b"MZ");
        put32(&mut f, 0x3C, pe as u32);
        f[pe..pe + 4].copy_from_slice(b"PE\0\0");
        put16(&mut f, pe + 6, 1);
        put16(&mut f, pe + 20, 0xE0);
        let opt = pe + 24;
        put16(&mut f, opt, 0x10B);
        put32(&mut f, opt + 112, 0x1000);
        let sec = opt + 0xE0;
        put32(&mut f, sec + 8, r.len() as u32);
        put32(&mut f, sec + 12, 0x1000);
        put32(&mut f, sec + 16, r.len() as u32);
        put32(&mut f, sec + 20, rsrc as u32);
        f.extend_from_slice(&r);
        f
    }
}
