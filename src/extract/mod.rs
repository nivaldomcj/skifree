//! The game's data in memory: the 89 sprites, the status font, the window
//! icon and the nine sounds, however they were obtained.
//!
//! Native builds read them from `assets/` (written by
//! `tools/extract_assets.py`); the private web build embeds those same
//! files; the public web build extracts them in the page from the player's
//! own `ski32.exe` with `from_files` (a port of the parts of
//! `tools/extract_assets.py` the game needs; `tests/extract.rs` checks it
//! against that script's output).

mod bytes;
mod dib;
mod fon;
pub mod pe;
mod zip;

use crate::game::sprites::SpriteSizes;
use pe::{Kind, Name, RT_BITMAP, RT_ICON};

/// An RGBA8 image, rows top-down.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

/// Number of bitmap sprites (`RT_BITMAP` ids 1..=89).
pub const SPRITE_COUNT: usize = 89;
/// Number of game sounds (WAVE ids 1..=9).
pub const SOUND_COUNT: usize = 9;

#[derive(Clone, Debug, Default)]
pub struct AssetSet {
    /// Indexed by sprite id; index 0 is unused.
    pub sprites: Vec<Option<Rgba>>,
    /// The OEM fixed font: printable ASCII from 0x20 in 8x12 cells.
    pub font: Option<Rgba>,
    /// The ICONSKI face, 32x32 (icon resource 3).
    pub icon: Option<Rgba>,
    /// The nine WAVs by sound id - 1, when all nine were found.
    pub sounds: Option<Vec<Vec<u8>>>,
}

impl AssetSet {
    pub fn new() -> Self {
        Self { sprites: vec![None; SPRITE_COUNT + 1], ..Default::default() }
    }

    /// The loaded sprites' sizes, for the game logic (16x16 for ids
    /// without a sprite).
    pub fn sprite_sizes(&self) -> SpriteSizes {
        let mut sizes = SpriteSizes::placeholder();
        for (id, sprite) in self.sprites.iter().enumerate() {
            if let (Some(s), Ok(id)) = (sprite, u16::try_from(id)) {
                sizes.set(id, (s.width as i16, s.height as i16));
            }
        }
        sizes
    }
}

/// The ICONSKI image the window icon uses (16 colours, 32x32).
const ICON_ID: u16 = 3;

/// Sprites, icon and sounds found in one 32-bit exe.
struct ExeAssets {
    sprites: Vec<Option<Rgba>>,
    icon: Option<Rgba>,
    sounds: Option<Vec<Vec<u8>>>,
}

impl ExeAssets {
    fn has_all_sprites(&self) -> bool {
        self.sprites.iter().skip(1).all(Option::is_some)
    }
}

/// Most decoded sprite bytes for a whole drop: one exe's 89 sprites take
/// 360 KB as RGBA, and sprite ids can all point at one huge bitmap.
const MAX_SPRITE_BYTES: usize = 16 * 1024 * 1024;

/// One exe's assets. Duplicate resources are only recorded during the walk
/// and decoded once after it (the last one wins, as in `extract_assets.py`),
/// so repeating an icon thousands of times costs nothing. `sprite_budget`
/// is shared by every exe in the drop.
fn read_exe(data: &[u8], sprite_budget: &mut usize) -> bytes::Result<ExeAssets> {
    let mut bitmaps: Vec<Option<&[u8]>> = vec![None; SPRITE_COUNT + 1];
    let mut icon: Option<&[u8]> = None;
    let mut wavs: Vec<Option<&[u8]>> = vec![None; SOUND_COUNT];
    for r in pe::resources(data)? {
        match (&r.rtype, &r.name) {
            (Name::Id(RT_BITMAP), Name::Id(id)) if (1..=SPRITE_COUNT).contains(&(*id as usize)) => {
                bitmaps[*id as usize] = Some(r.data);
            }
            (Name::Id(RT_ICON), Name::Id(ICON_ID)) => icon = Some(r.data),
            // FUN_004052d0 loads WAVE resources 1-9 (ski32sounds.exe has
            // two more that are never played).
            (Name::Str(t), Name::Id(id))
                if t == "WAVE"
                    && (1..=SOUND_COUNT).contains(&(*id as usize))
                    && r.data.starts_with(b"RIFF")
                    && r.data.get(8..12) == Some(b"WAVE") =>
            {
                wavs[*id as usize - 1] = Some(r.data);
            }
            _ => {}
        }
    }
    // Charged to the drop's budget only if the whole exe loads: a file
    // that fails partway must not use up the budget of a good one.
    let mut budget = *sprite_budget;
    let mut sprites = vec![None; SPRITE_COUNT + 1];
    for (slot, bitmap) in sprites.iter_mut().zip(&bitmaps) {
        if let Some(bitmap) = bitmap {
            let sprite = dib::decode_sprite(bitmap)?;
            budget = budget.checked_sub(sprite.data.len()).ok_or("its pictures are far too large")?;
            *slot = Some(sprite);
        }
    }
    let assets = ExeAssets {
        sprites,
        icon: icon.map(dib::decode_icon).transpose()?,
        // Sound only with all nine, like build.rs.
        sounds: wavs.into_iter().map(|w| w.map(<[u8]>::to_vec)).collect(),
    };
    *sprite_budget = budget;
    Ok(assets)
}

/// Build the asset set from the player's files: `ski32.exe` (sprites and
/// icon), optionally `ski32sounds.exe` (it has the sprites too, plus the
/// sounds) and `vgaoem.fon` (the status font), each as is or inside a .zip.
/// Files are recognised by content, not name.
///
/// # Errors
///
/// When the files hold no usable `ski32.exe`: sentences for the page that
/// say what was wrong with each file.
pub fn from_files(files: &[(String, Vec<u8>)]) -> Result<AssetSet, String> {
    if files.is_empty() {
        return Err("No files given.".to_owned());
    }
    let mut inputs: Vec<(String, std::borrow::Cow<[u8]>)> = Vec::new();
    // A zip that can't be read is one more problem, not the end: the other
    // files may still hold the game.
    let mut problems = Vec::new();
    for (name, data) in files {
        if zip::is_zip(data) {
            match zip::members(data) {
                Ok(members) => inputs.extend(members.into_iter().map(|(n, d)| (format!("{name}/{n}"), d.into()))),
                Err(e) => problems.push(format!("{name}: {e}.")),
            }
        } else {
            inputs.push((name.clone(), data.as_slice().into()));
        }
    }

    let mut exes: Vec<(String, ExeAssets)> = Vec::new();
    let mut font = None;
    let mut sprite_budget = MAX_SPRITE_BYTES;
    for (name, data) in &inputs {
        match pe::kind(data) {
            Kind::Pe => match read_exe(data, &mut sprite_budget) {
                Ok(found) => exes.push((name.clone(), found)),
                Err(e) => problems.push(format!("{name}: {e}.")),
            },
            Kind::Ne => match fon::oem_font(data) {
                Ok(Some(img)) => font = font.or(Some(img)),
                Ok(None) if name.to_ascii_lowercase().ends_with(".fon") => {
                    problems.push(format!("{name} has no 12-pixel font."));
                }
                _ => problems.push(format!(
                    "{name} is a 16-bit Windows program (like SKI.EXE, the Windows 3 version); \
                     this page needs the 32-bit ski32.exe."
                )),
            },
            Kind::Other => {}
        }
    }

    // Sprites from ski32.exe when given (what tools/extract_assets.py
    // prefers), else from any exe that has all 89.
    let is_ski32 = |n: &str| n.rsplit('/').next().is_some_and(|b| b.eq_ignore_ascii_case("ski32.exe"));
    // By position, not name: two inputs can share a name.
    let main = exes
        .iter()
        .enumerate()
        .filter(|(_, (_, e))| e.has_all_sprites())
        .min_by_key(|(_, (n, _))| !is_ski32(n))
        .map(|(i, _)| i);
    let Some(main) = main else {
        if let Some((n, _)) = exes.first() {
            problems.insert(0, format!("{n} is not SkiFree's ski32.exe (its pictures are missing)."));
        }
        if problems.is_empty() {
            let names: Vec<&str> = inputs.iter().map(|(n, _)| n.as_str()).collect();
            problems.push(format!("No ski32.exe found in {}.", names.join(", ")));
        }
        return Err(problems.join(" "));
    };

    let mut set = AssetSet::new();
    let mut sounds = None;
    for (i, (_, found)) in exes.into_iter().enumerate() {
        if sounds.is_none() {
            sounds.clone_from(&found.sounds);
        }
        if i == main {
            set.sprites = found.sprites;
            set.icon = found.icon;
        }
    }
    set.sounds = sounds;
    set.font = font;
    Ok(set)
}

#[cfg(test)]
mod tests {
    use super::pe::testing::{build, Ty};
    use super::*;

    /// A 32x32, 16-colour icon image (XOR bitmap and AND mask, so the header
    /// says 64 rows); `truncated` keeps only the header and palette.
    fn icon(truncated: bool) -> Vec<u8> {
        let mut d = vec![0u8; if truncated { 40 + 64 } else { 40 + 64 + 512 + 128 }];
        d[..4].copy_from_slice(&40u32.to_le_bytes());
        d[4..8].copy_from_slice(&32i32.to_le_bytes());
        d[8..12].copy_from_slice(&64i32.to_le_bytes());
        d[12..14].copy_from_slice(&1u16.to_le_bytes());
        d[14..16].copy_from_slice(&4u16.to_le_bytes());
        d
    }

    #[test]
    fn duplicate_icons_decode_once_and_the_last_wins() {
        // A damaged first copy no longer fails the exe: only the last copy
        // is decoded, as extract_assets.py keeps the last one.
        let (bad, good) = (icon(true), icon(false));
        let exe = build(&[(Ty::Id(RT_ICON), ICON_ID, &bad), (Ty::Id(RT_ICON), ICON_ID, &good)]);
        let found = read_exe(&exe, &mut MAX_SPRITE_BYTES.clone()).unwrap();
        assert_eq!(found.icon.map(|i| (i.width, i.height)), Some((32, 32)));
    }

    #[test]
    fn many_duplicate_icons_cost_one_decode() {
        let good = icon(false);
        // 2000 copies: each takes two directory entries, within MAX_ENTRIES.
        let entries: Vec<_> = (0..2000).map(|_| (Ty::Id(RT_ICON), ICON_ID, good.as_slice())).collect();
        let found = read_exe(&build(&entries), &mut MAX_SPRITE_BYTES.clone()).unwrap();
        assert!(found.icon.is_some());
    }

    #[test]
    fn the_sprite_budget_is_shared_by_the_drop() {
        let mut budget = 10;
        let mut bmp = icon(false);
        bmp[8..12].copy_from_slice(&32i32.to_le_bytes()); // a 32x32 sprite: 4 KB as RGBA
        let exe = build(&[(Ty::Id(RT_BITMAP), 1, &bmp)]);
        assert!(read_exe(&exe, &mut budget).is_err());
        let mut budget = MAX_SPRITE_BYTES;
        read_exe(&exe, &mut budget).unwrap();
        assert_eq!(budget, MAX_SPRITE_BYTES - 32 * 32 * 4);
        // An exe that fails after decoding a sprite is charged nothing.
        let broken = build(&[(Ty::Id(RT_BITMAP), 1, &bmp), (Ty::Id(RT_BITMAP), 2, &bmp[..20])]);
        let before = budget;
        assert!(read_exe(&broken, &mut budget).is_err());
        assert_eq!(budget, before);
    }
}
