//! Just enough ZIP to open ski32.zip and ski32sounds.zip: the central
//! directory, stored and deflated members.

use super::bytes::{slice, u16_at, u32_at, Result};

/// Largest member unpacked (the exes are 116 KB and 248 KB).
const MAX_MEMBER: usize = 16 * 1024 * 1024;

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
        p += 46 + skip;

        let base = name.rsplit(['/', '\\']).next().unwrap_or("").to_owned();
        if base.is_empty() || size > MAX_MEMBER {
            continue; // a folder, or too big to be ours
        }
        if flags & 1 != 0 {
            return Err(format!("{name} in the zip is encrypted"));
        }
        if slice(data, local, 4)? != b"PK\x03\x04" {
            return Err("damaged zip file".to_owned());
        }
        let start = local + 30 + u16_at(data, local + 26)? as usize + u16_at(data, local + 28)? as usize;
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
