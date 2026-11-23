//! Build-time asset gate.
//!
//! This port ships no copyrighted material: the sprites, icons and string
//! tables are extracted from a `ski32.exe` the user owns (see README.md,
//! "Getting the assets"). If `assets/` is missing or incomplete, fail the
//! build with instructions instead of producing a broken binary.

use std::path::Path;

const EXTRACT_HINT: &str = "\
SkiFree assets are missing or incomplete.
Place a ski32.exe you own in originals/ and run:

    python3 tools/extract_assets.py

See README.md, \"Getting the assets\".";

fn main() {
    let meta_path = Path::new("assets/meta.json");
    println!("cargo:rerun-if-changed=assets/meta.json");
    println!("cargo:rerun-if-changed=assets/strings.json");

    let meta = std::fs::read_to_string(meta_path).unwrap_or_else(|_| panic!("{EXTRACT_HINT}"));

    // Every resource with a "file" key must exist on disk. The meta.json
    // schema is simple enough for a minimal scan ("file": "path").
    let mut missing = 0usize;
    let mut sprite_files = 0usize;
    let mut rest = meta.as_str();
    while let Some(pos) = rest.find("\"file\"") {
        rest = &rest[pos + 6..];
        let Some(colon) = rest.find(':') else { break };
        let after = rest[colon + 1..].trim_start();
        let Some(stripped) = after.strip_prefix('"') else { continue };
        let Some(end) = stripped.find('"') else { break };
        let rel = &stripped[..end];
        rest = &stripped[end..];
        if rel.starts_with("sprites/") {
            sprite_files += 1;
        }
        if !Path::new("assets").join(rel).exists() {
            missing += 1;
        }
    }

    // The extractor always produces the 89 bitmap sprites, the string
    // tables and the icons.
    let strings_ok = Path::new("assets/strings.json").exists();
    let icons_ok = std::fs::read_dir("assets/icons")
        .map(|mut d| d.next().is_some())
        .unwrap_or(false);

    if sprite_files < 89 || missing > 0 || !strings_ok || !icons_ok {
        panic!(
            "{EXTRACT_HINT}\n\n(details: {} sprite files, {} missing files, strings.json: {}, icons/: {})",
            sprite_files, missing, strings_ok, icons_ok
        );
    }

    println!("cargo:warning=assets OK: {} sprite files verified", sprite_files);

    // Windows: embed the original's ICONSKI group (assets/icons/app_1.ico,
    // extracted from the user's exe) as the exe's icon resource, so
    // Explorer shows the SkiFree face on skifree.exe like on ski32.exe.
    // The window icon is set at runtime (platform::window).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_windows_icon();
    }

    // Sound is optional: with all nine extracted WAVs (from an exe that has
    // them, e.g. ski32sounds.exe) the build embeds them and compiles the
    // audio code; otherwise it has no sound at all, like the shipped game.
    println!("cargo:rustc-check-cfg=cfg(skifree_sound)");
    println!("cargo:rerun-if-changed=assets/sounds");
    let sounds = (1..=9)
        .filter(|i| Path::new(&format!("assets/sounds/sound_{i}.wav")).exists())
        .count();
    if sounds == 9 {
        println!("cargo:rustc-cfg=skifree_sound");
        println!("cargo:warning=sounds OK: 9 WAVs embedded, sound enabled");
    } else if sounds > 0 {
        println!(
            "cargo:warning=only {sounds} of 9 sounds in assets/sounds/: building without sound \
             (re-run tools/extract_assets.py with ski32sounds.exe in originals/)"
        );
    }
}

fn embed_windows_icon() {
    let ico = Path::new("assets/icons/app_1.ico");
    println!("cargo:rerun-if-changed=assets/icons/app_1.ico");
    let ico = ico.canonicalize().expect("assets/icons/app_1.ico (checked above)");
    let out = std::env::var("OUT_DIR").expect("OUT_DIR");
    let rc = Path::new(&out).join("skifree.rc");
    // The resource name the original uses (LoadIconA(hInstance, "iconSki")).
    let path = ico.to_string_lossy().replace('\\', "/");
    std::fs::write(&rc, format!("iconSki ICON \"{path}\"\n")).expect("write skifree.rc");
    if let Err(e) = embed_resource::compile(&rc, embed_resource::NONE).manifest_optional() {
        println!("cargo:warning=no exe icon (resource compiler failed: {e})");
    }
}
