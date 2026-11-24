//! Bounds-checked little-endian reads: the player can drop any file, so a
//! malformed one must give an error, not a panic.

pub type Result<T> = std::result::Result<T, String>;

pub fn slice(data: &[u8], off: usize, len: usize) -> Result<&[u8]> {
    off.checked_add(len).and_then(|end| data.get(off..end)).ok_or_else(|| "file is truncated or damaged".to_owned())
}

pub fn u8_at(data: &[u8], off: usize) -> Result<u8> {
    Ok(slice(data, off, 1)?[0])
}

pub fn u16_at(data: &[u8], off: usize) -> Result<u16> {
    let b = slice(data, off, 2)?;
    Ok(u16::from_le_bytes([b[0], b[1]]))
}

pub fn u32_at(data: &[u8], off: usize) -> Result<u32> {
    let b = slice(data, off, 4)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// `base + off`, or the damaged-file error when the sum overflows (header
/// fields are attacker-chosen, and `usize` is 32 bits on wasm32).
pub fn at(base: usize, off: usize) -> Result<usize> {
    base.checked_add(off).ok_or_else(|| "file is truncated or damaged".to_owned())
}

pub fn i32_at(data: &[u8], off: usize) -> Result<i32> {
    Ok(u32_at(data, off)? as i32)
}
