#!/bin/sh
# Print cargo's target directory (absolute): where builds land, honouring
# CARGO_TARGET_DIR and build.target-dir. Run from the repo root, as the
# tools that call it do.
set -e
cargo metadata --format-version 1 --no-deps --locked |
    python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])'
