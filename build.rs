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
}
