//! Sprite sizes by resource id: the original's sprite table (width at
//! `+0xe`, height at `+0x10`, filled from the bitmaps at startup by
//! `FUN_004052d0`). Collisions, culling, the density sum and drawing all
//! read it, so a run's outcome depends on it: the caller passes the sizes
//! of the sprites it actually loaded (see `crate::assets`).

/// The size assumed for an id without a sprite.
const FALLBACK: (i16, i16) = (16, 16);

/// Sizes indexed by sprite id (index 0, the empty entry, included).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpriteSizes(Vec<(i16, i16)>);

impl SpriteSizes {
    pub fn new(sizes: Vec<(i16, i16)>) -> Self {
        Self(sizes)
    }

    /// Every sprite 16x16: for unit tests that don't depend on the real
    /// sizes (and run without the extracted assets, as CI does).
    pub fn placeholder() -> Self {
        Self(vec![FALLBACK; crate::extract::SPRITE_COUNT + 1])
    }

    /// The size of sprite `id`, or 16x16 for an id past the table.
    pub fn get(&self, id: u16) -> (i16, i16) {
        self.lookup(id).unwrap_or(FALLBACK)
    }

    /// The size of sprite `id`, if the table has it.
    pub fn lookup(&self, id: u16) -> Option<(i16, i16)> {
        self.0.get(usize::from(id)).copied()
    }

    /// Change one entry (tests).
    pub fn set(&mut self, id: u16, size: (i16, i16)) {
        if let Some(entry) = self.0.get_mut(usize::from(id)) {
            *entry = size;
        }
    }
}
