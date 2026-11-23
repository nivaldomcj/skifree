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

    /// Sprite sizes indexed by id, laid out like
    /// `GameState::load_sprite_dims` (16x16 for ids without a sprite).
    pub fn sprite_dims(&self) -> Vec<(i16, i16)> {
        let mut dims = vec![(16i16, 16i16); SPRITE_COUNT + 1];
        for (id, sprite) in self.sprites.iter().enumerate() {
            if let (Some(s), Some(d)) = (sprite, dims.get_mut(id)) {
                *d = (s.width as i16, s.height as i16);
            }
        }
        dims
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

/// Most decoded sprite bytes kept from one exe: the game's 89 sprites take
/// 360 KB as RGBA, and sprite ids can all point at one huge bitmap.
const MAX_SPRITE_BYTES: usize = 16 * 1024 * 1024;

fn read_exe(data: &[u8]) -> bytes::Result<ExeAssets> {
    let mut found = ExeAssets { sprites: vec![None; SPRITE_COUNT + 1], icon: None, sounds: None };
    let mut wavs: Vec<Option<Vec<u8>>> = vec![None; SOUND_COUNT];
    let mut sprite_bytes = 0usize;
    for r in pe::resources(data)? {
        match (&r.rtype, &r.name) {
            (Name::Id(RT_BITMAP), Name::Id(id)) if (1..=SPRITE_COUNT).contains(&(*id as usize)) => {
                let sprite = dib::decode_sprite(r.data)?;
                sprite_bytes += sprite.data.len();
                if sprite_bytes > MAX_SPRITE_BYTES {
                    return Err("its pictures are far too large".to_owned());
                }
                found.sprites[*id as usize] = Some(sprite);
            }
            (Name::Id(RT_ICON), Name::Id(ICON_ID)) => found.icon = Some(dib::decode_icon(r.data)?),
            // FUN_004052d0 loads WAVE resources 1-9 (ski32sounds.exe has
            // two more that are never played).
            (Name::Str(t), Name::Id(id))
                if t == "WAVE"
                    && (1..=SOUND_COUNT).contains(&(*id as usize))
                    && r.data.starts_with(b"RIFF")
                    && r.data.get(8..12) == Some(b"WAVE") =>
            {
                wavs[*id as usize - 1] = Some(r.data.to_vec());
            }
            _ => {}
        }
    }
    // Sound only with all nine, like build.rs.
    found.sounds = wavs.into_iter().collect();
    Ok(found)
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
    for (name, data) in &inputs {
        match pe::kind(data) {
            Kind::Pe => match read_exe(data) {
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
