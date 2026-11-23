#!/usr/bin/env bash
# Run the replay tests compiled to WebAssembly (wasm32-wasip1, run by
# wasmtime): the same ticks, rand() calls, status values and sounds as the
# native run prove that the web build computes the same game.
#
#   tools/test_wasi.sh [cargo test args...]
#
# Needs `rustup target add wasm32-wasip1` and wasmtime. The tests read
# replays/ through absolute paths and assets/meta.json (sprite sizes, which
# the goldens depend on) relative to the repo root: both are mapped in.
set -euo pipefail
cd "$(dirname "$0")/.."
command -v wasmtime >/dev/null || { echo "wasmtime not found (https://wasmtime.dev)" >&2; exit 1; }
[ -f assets/meta.json ] || { echo "assets/ missing: run tools/extract_assets.py" >&2; exit 1; }
# The array form keeps a repo path with spaces in one argument (the env
# var form would split it).
exec cargo test --config "target.wasm32-wasip1.runner=[\"wasmtime\", \"run\", \"--dir=.\", \"--dir=$PWD::$PWD\"]" \
    --target wasm32-wasip1 --test replay "$@"
