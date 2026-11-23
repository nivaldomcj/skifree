//! Build-time asset gate.
//!
//! This port ships no copyrighted material: the sprites, icons and string
//! tables are extracted from a `ski32.exe` the user owns (see README.md,
//! "Getting the assets"). If `assets/` is missing or incomplete, fail the
//! build with instructions instead of producing a broken binary.
//!
//! Web builds (wasm32) don't read `assets/` at run time:
//! the public one needs no assets at all (players supply their own exe in
//! the page), and the private one (`--features embed-assets`) embeds them,
//! through `$OUT_DIR/web_assets.rs`.
//!
//! `SKIFREE_NO_ASSETS` skips the gate, for `cargo clippy`, `cargo check`
//! and `cargo test` where nobody has the game files (CI):
//! - `=1` builds like a player without the sounds: no audio code at all.
//! - `=sound` also compiles the audio code, with empty stand-in WAVs
//!   (`cfg(skifree_no_assets)`), so it is linted too; that build never
//!   opens a player, so it stays silent.
//!
//! The binary still loads `assets/` at run time, so it runs the game where
//! they exist, only without the build-time checks and embedding.

use std::path::Path;

const EXTRACT_HINT: &str = "\
SkiFree assets are missing or incomplete.
Place a ski32.exe you own in originals/ and run:

    python3 tools/extract_assets.py

See README.md, \"Getting the assets\".";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-check-cfg=cfg(skifree_sound)");
    println!("cargo:rustc-check-cfg=cfg(skifree_no_assets)");
    println!("cargo:rerun-if-env-changed=SKIFREE_NO_ASSETS");
    // Any wasm32 target builds the web platform layer: wasm32-unknown-unknown
    // is the browser; wasm32-wasip1 only runs the replay tests
    // (tools/test_wasi.sh), for which cargo builds, but never runs, the game.
    let web = std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32");
    let embed = std::env::var_os("CARGO_FEATURE_EMBED_ASSETS").is_some();
    if web && !embed {
        println!("cargo:warning=public web build: no game data embedded (players supply their own ski32.exe)");
        return;
    }
    if embed && !web {
        println!("cargo:warning=embed-assets only applies to the web build (wasm32)");
    }
    if let Some(mode) = std::env::var_os("SKIFREE_NO_ASSETS") {
        assert!(!web, "SKIFREE_NO_ASSETS cannot build the private web version (embed-assets)");
        match mode.to_str() {
            Some("1") => {}
            Some("sound") => {
                println!("cargo:rustc-cfg=skifree_sound");
                println!("cargo:rustc-cfg=skifree_no_assets");
            }
            _ => panic!("SKIFREE_NO_ASSETS must be 1 (no sound code) or sound (sound code, silent)"),
        }
        println!("cargo:warning=SKIFREE_NO_ASSETS: assets not checked, for linting and tests only");
        return;
    }

    check_assets();
    if web {
        embed_web_assets();
        return;
    }

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
    // Watch assets/ as a whole (Cargo scans a directory recursively): a
    // missing assets/sounds would count as changed on every build.
    println!("cargo:rerun-if-changed=assets");
    let sounds = sound_count();
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

fn sound_count() -> usize {
    (1..=9).filter(|i| Path::new(&format!("assets/sounds/sound_{i}.wav")).exists()).count()
}

fn check_assets() {
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
    let icons_ok = std::fs::read_dir("assets/icons").is_ok_and(|mut d| d.next().is_some());

    assert!(
        sprite_files >= 89 && missing == 0 && strings_ok && icons_ok,
        "{EXTRACT_HINT}\n\n(details: {sprite_files} sprite files, {missing} missing files, strings.json: {strings_ok}, icons/: {icons_ok})"
    );

    println!("cargo:warning=assets OK: {sprite_files} sprite files verified");
}

/// The private web build: `$OUT_DIR/web_assets.rs` embeds the 89 sprite
/// PNGs, the font and icon 3 when present, and the sounds when all nine
/// are (`platform::assets` decodes them like the native build's files).
fn embed_web_assets() {
    // All of assets/: the optional fonts/ and sounds/ may not exist.
    println!("cargo:rerun-if-changed=assets");
    let include = |rel: &str| -> String {
        let path = Path::new("assets").join(rel).canonicalize().expect("asset checked above");
        format!("include_bytes!({:?})", path.to_string_lossy().replace('\\', "/"))
    };
    let optional = |rel: &str| -> String {
        if Path::new("assets").join(rel).exists() {
            format!("Some({})", include(rel))
        } else {
            "None".to_owned()
        }
    };
    let sprites: Vec<String> = (1..=89).map(|id| include(&format!("sprites/bitmap_{id:02}.png"))).collect();
    let sounds = if sound_count() == 9 {
        let wavs: Vec<String> = (1..=9).map(|i| include(&format!("sounds/sound_{i}.wav"))).collect();
        format!("Some([{}])", wavs.join(", "))
    } else {
        "None".to_owned()
    };
    let code = format!(
        "pub static SPRITES: [&[u8]; 89] = [{}];\n\
         pub static FONT: Option<&[u8]> = {};\n\
         pub static ICON: Option<&[u8]> = {};\n\
         pub static SOUNDS: Option<[&[u8]; 9]> = {};\n",
        sprites.join(", "),
        optional("fonts/oem_fixed.png"),
        optional("icons/icon_3.png"),
        sounds,
    );
    let out = Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("web_assets.rs");
    std::fs::write(out, code).expect("write web_assets.rs");
    println!(
        "cargo:warning=private web build: assets embedded{} (never publish this build)",
        if sound_count() == 9 { ", sound included" } else { "" }
    );
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
