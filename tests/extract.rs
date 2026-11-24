//! The Rust extractor (`skifree::extract`, used by the public web build)
//! against `tools/extract_assets.py`'s output in assets/: same sprite pixels
//! and sizes, same icon, same font, same WAV bytes, from the exes and from
//! the zips they are distributed in.
//!
//! Needs the user's own originals/ and assets/ (neither is in the repo):
//! each test skips when a file it needs is absent.

use macroquad::prelude::{Image, ImageFormat};
use skifree::extract::{from_files, AssetSet, Rgba};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The named files from originals/, or None (skip) if any is missing.
fn originals(names: &[&str]) -> Option<Vec<(String, Vec<u8>)>> {
    names.iter().map(|n| Some((n.to_string(), std::fs::read(root().join("originals").join(n)).ok()?))).collect()
}

fn png(rel: &str) -> Option<Rgba> {
    let bytes = std::fs::read(root().join("assets").join(rel)).ok()?;
    let img = Image::from_file_with_format(&bytes, Some(ImageFormat::Png)).expect("decodes");
    Some(Rgba { width: u32::from(img.width), height: u32::from(img.height), data: img.bytes })
}

macro_rules! need {
    ($e:expr) => {
        match $e {
            Some(v) => v,
            None => {
                eprintln!("skip: originals/ or assets/ not present");
                return;
            }
        }
    };
}

fn check_sprites(set: &AssetSet) {
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root().join("assets/meta.json")).unwrap()).unwrap();
    let mut dims = std::collections::HashMap::new();
    for r in meta["resources"].as_array().unwrap() {
        if r["type"] == 2 {
            dims.insert(
                r["id"].as_u64().unwrap() as usize,
                (r["width"].as_i64().unwrap() as i16, r["height"].as_i64().unwrap() as i16),
            );
        }
    }
    assert_eq!(dims.len(), 89);
    let sizes = set.sprite_sizes();
    for id in 1..=89 {
        let expected = png(&format!("sprites/bitmap_{id:02}.png")).unwrap();
        assert_eq!(set.sprites[id].as_ref(), Some(&expected), "sprite {id} pixels");
        assert_eq!(sizes.get(id as u16), dims[&id], "sprite {id} size");
    }
}

fn check_icon(set: &AssetSet) {
    assert_eq!(set.icon, png("icons/icon_3.png"), "icon 3");
}

fn check_sounds(set: &AssetSet) {
    let wavs = set.sounds.as_ref().expect("sounds");
    // The extractor must still find them; only the comparison needs
    // tools/extract_assets.py's output.
    if !root().join("assets/sounds").is_dir() {
        eprintln!("skip sound comparison: assets/sounds/ not present");
        return;
    }
    for (i, wav) in wavs.iter().enumerate() {
        let expected = std::fs::read(root().join(format!("assets/sounds/sound_{}.wav", i + 1))).unwrap();
        assert!(wav == &expected, "sound {} bytes", i + 1);
    }
}

#[test]
fn ski32_exe_matches_python_extraction() {
    let files = need!(originals(&["ski32.exe"]));
    need!(png("sprites/bitmap_01.png"));
    let set = from_files(&files).unwrap();
    check_sprites(&set);
    check_icon(&set);
    assert!(set.sounds.is_none(), "the shipped ski32.exe has no sounds");
    assert!(set.font.is_none());
}

#[test]
fn full_set_from_exes_matches_python_extraction() {
    let files = need!(originals(&["ski32.exe", "ski32sounds.exe", "vgaoem.fon"]));
    need!(png("fonts/oem_fixed.png"));
    let set = from_files(&files).unwrap();
    check_sprites(&set);
    check_icon(&set);
    check_sounds(&set);
    assert_eq!(set.font, png("fonts/oem_fixed.png"), "font");
}

#[test]
fn full_set_from_zips_matches_python_extraction() {
    let files = need!(originals(&["ski32.zip", "ski32sounds.zip", "vgaoem.fon"]));
    need!(png("fonts/oem_fixed.png"));
    let set = from_files(&files).unwrap();
    check_sprites(&set);
    check_icon(&set);
    check_sounds(&set);
    assert_eq!(set.font, png("fonts/oem_fixed.png"), "font");
}

#[test]
fn ski32sounds_alone_is_enough() {
    let files = need!(originals(&["ski32sounds.zip"]));
    need!(png("sprites/bitmap_01.png"));
    let set = from_files(&files).unwrap();
    check_sprites(&set);
    check_sounds(&set);
}

#[test]
fn sixteen_bit_ski_exe_is_refused() {
    let files = need!(originals(&["SKI.EXE"]));
    let err = from_files(&files).unwrap_err();
    assert!(err.contains("16-bit"), "{err}");
}

#[test]
fn other_files_are_refused_without_panicking() {
    let junk: Vec<(String, Vec<u8>)> = vec![
        ("notes.txt".into(), b"hello".to_vec()),
        ("empty.exe".into(), Vec::new()),
        ("mz.exe".into(), b"MZ\0\0".to_vec()),
        ("pk.zip".into(), b"PK\x03\x04garbage".to_vec()),
    ];
    for file in &junk {
        assert!(from_files(std::slice::from_ref(file)).is_err(), "{}", file.0);
    }
    assert!(from_files(&[]).is_err());
    // Truncated copies of a real exe must fail cleanly too.
    if let Some(files) = originals(&["ski32.exe"]) {
        let exe = &files[0].1;
        for len in [64, 512, 4096, exe.len() / 2, exe.len() - 1] {
            let _ = from_files(&[("ski32.exe".into(), exe[..len].to_vec())]);
        }
    }
}
