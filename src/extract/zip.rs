//! Just enough ZIP to open ski32.zip and ski32sounds.zip: the central
//! directory, stored and deflated members.

use super::bytes::{at, slice, u16_at, u32_at, Result};

/// Largest member unpacked (the exes are 116 KB and 248 KB).
const MAX_MEMBER: usize = 16 * 1024 * 1024;
/// Most bytes unpacked from one zip: entries can share data, so a small
/// crafted zip could otherwise unpack its 16 MB member thousands of times.
const MAX_TOTAL: usize = 64 * 1024 * 1024;

pub fn is_zip(data: &[u8]) -> bool {
    data.starts_with(b"PK\x03\x04") || data.starts_with(b"PK\x05\x06")
}

/// The files in a ZIP: (name without folders, contents).
pub fn members(data: &[u8]) -> Result<Vec<(String, Vec<u8>)>> {
    // End of central directory: 22 bytes plus a comment of up to 64 KB.
    let min = data.len().saturating_sub(22 + 0xFFFF);
    let eocd = (min..data.len().saturating_sub(21))
        .rev()
        .find(|&o| data[o..].starts_with(b"PK\x05\x06"))
        .ok_or_else(|| "damaged zip file".to_owned())?;
    let count = u16_at(data, eocd + 10)? as usize;
    let mut p = u32_at(data, eocd + 16)? as usize;
    let mut out = Vec::new();
    let mut total = 0usize;
    for _ in 0..count {
        if slice(data, p, 4)? != b"PK\x01\x02" {
            return Err("damaged zip file".to_owned());
        }
        let flags = u16_at(data, p + 8)?;
        let method = u16_at(data, p + 10)?;
        let packed = u32_at(data, p + 20)? as usize;
        let size = u32_at(data, p + 24)? as usize;
        let name_len = u16_at(data, p + 28)? as usize;
        let skip = name_len + u16_at(data, p + 30)? as usize + u16_at(data, p + 32)? as usize;
        let local = u32_at(data, p + 42)? as usize;
        let name = String::from_utf8_lossy(slice(data, p + 46, name_len)?).into_owned();
        p = at(p, 46 + skip)?;

        let base = name.rsplit(['/', '\\']).next().unwrap_or("").to_owned();
        if base.is_empty() || size > MAX_MEMBER {
            continue; // a folder, or too big to be ours
        }
        total += size;
        if total > MAX_TOTAL {
            return Err("the zip holds too much data to be SkiFree's".to_owned());
        }
        if flags & 1 != 0 {
            return Err(format!("{name} in the zip is encrypted"));
        }
        if slice(data, local, 4)? != b"PK\x03\x04" {
            return Err("damaged zip file".to_owned());
        }
        let start = at(local, 30 + u16_at(data, local + 26)? as usize + u16_at(data, local + 28)? as usize)?;
        let raw = slice(data, start, packed)?;
        let bytes = match method {
            0 => raw.to_vec(),
            8 => miniz_oxide::inflate::decompress_to_vec_with_limit(raw, MAX_MEMBER)
                .map_err(|_| format!("{name} in the zip is damaged"))?,
            _ => return Err(format!("{name} in the zip uses an unsupported compression")),
        };
        if bytes.len() != size {
            return Err(format!("{name} in the zip is damaged"));
        }
        out.push((base, bytes));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A zip whose `copies` central-directory entries all point at one
    /// deflated member of `size` zero bytes.
    fn overlapping_zip(size: usize, copies: u16) -> Vec<u8> {
        let packed = miniz_oxide::deflate::compress_to_vec(&vec![0u8; size], 9);
        let mut f = Vec::new();
        f.extend_from_slice(b"PK\x03\x04");
        f.extend_from_slice(&[0; 22]);
        f.extend_from_slice(&5u16.to_le_bytes()); // name length
        f.extend_from_slice(&0u16.to_le_bytes());
        f.extend_from_slice(b"a.exe");
        f.extend_from_slice(&packed);
        let central = f.len();
        for _ in 0..copies {
            let mut e = vec![0u8; 46];
            e[..4].copy_from_slice(b"PK\x01\x02");
            e[10..12].copy_from_slice(&8u16.to_le_bytes()); // deflated
            e[20..24].copy_from_slice(&(packed.len() as u32).to_le_bytes());
            e[24..28].copy_from_slice(&(size as u32).to_le_bytes());
            e[28..30].copy_from_slice(&5u16.to_le_bytes());
            f.extend_from_slice(&e);
            f.extend_from_slice(b"a.exe");
        }
        let mut eocd = vec![0u8; 22];
        eocd[..4].copy_from_slice(b"PK\x05\x06");
        eocd[10..12].copy_from_slice(&copies.to_le_bytes());
        eocd[16..20].copy_from_slice(&(central as u32).to_le_bytes());
        f.extend_from_slice(&eocd);
        f
    }

    #[test]
    fn small_zip_unpacks() {
        let members = members(&overlapping_zip(1000, 3)).unwrap();
        assert_eq!(members.len(), 3);
        assert!(members.iter().all(|(n, d)| n == "a.exe" && d.len() == 1000));
    }

    #[test]
    fn overlapping_member_bomb_is_refused() {
        // 1000 entries x 16 MB = 16 GB unpacked from a file of a few KB.
        let f = overlapping_zip(MAX_MEMBER, 1000);
        assert!(f.len() < 128 * 1024);
        assert_eq!(members(&f).map(|m| m.len()), Err("the zip holds too much data to be SkiFree's".to_owned()));
    }
}
