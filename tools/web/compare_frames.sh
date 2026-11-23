#!/usr/bin/env bash
# Pixel check of the web build against the native one: both draw the same
# replay frames and every pair must be byte-identical. The native frames
# already match the original (docs/vm-ground-truth.md, "Pixel comparison").
#
#   tools/web/compare_frames.sh [<scenario> <t1,t2,...>] [out-dir]
#
# Without a scenario, runs the default set below. Needs a private web build
# (tools/build_web.sh --private), Node 22+ and Chrome (see
# capture_frames.mjs). Frames go to out-dir (default: a temp dir).
set -euo pipefail
cd "$(dirname "$0")/../.."

DEFAULT_SET="compare_vm 1,100,300,600
compare_steer 100,250,400,600
compare_size_640x480 100,300,600
compare_size_1024x700 100,300,600
compare_freestyle 200,600,1000,1240
compare_monster 400,900,1300
compare_slalom 300,800,1240
compare_mouse 300,900"

if [ $# -ge 2 ]; then
    set_list="$1 $2"
    out="${3:-}"
else
    set_list="$DEFAULT_SET"
    out="${1:-}"
fi
out="${out:-$(mktemp -d -t skifree-frames)}"

[ -f dist/web-private/skifree.wasm ] || { echo "no private web build: run tools/build_web.sh --private" >&2; exit 1; }
cargo build --quiet
fail=0
total=0
while read -r scenario ticks; do
    [ -n "$scenario" ] || continue
    SKIFREE_REPLAY="replays/$scenario.json" SKIFREE_FRAMES="$out/native/$scenario" \
        SKIFREE_FRAME_TICKS="$ticks" ./target/debug/skifree >/dev/null 2>&1
    node tools/web/capture_frames.mjs dist/web-private "$scenario" "$ticks" "$out/web/$scenario" >/dev/null
    for tick in ${ticks//,/ }; do
        total=$((total + 1))
        native=$(ls "$out/native/$scenario"/frame_"$tick"_*.rgba 2>/dev/null || true)
        web="$out/web/$scenario/$(basename "${native:-none}")"
        if [ -z "$native" ] || [ ! -f "$web" ]; then
            echo "MISSING $scenario tick $tick"
            fail=$((fail + 1))
        elif ! cmp -s "$native" "$web"; then
            echo "DIFFERS $scenario tick $tick"
            fail=$((fail + 1))
        fi
    done
    echo "$scenario: done"
done <<< "$set_list"

echo "$((total - fail))/$total frames identical (frames in $out)"
[ "$fail" = 0 ]
