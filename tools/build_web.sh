#!/usr/bin/env bash
# Build the browser version (wasm32-unknown-unknown + miniquad's gl.js).
#
#   tools/build_web.sh --private [--serve] [--port N]
#       Your extracted assets/ embedded in the .wasm -> dist/web-private/.
#       For your own play-testing only: never publish or commit this build.
#   tools/build_web.sh --public [--serve] [--port N]
#       No game data; players drop their own ski32.exe -> dist/web/.
#
# --serve serves the result on every network interface (your LAN too) at
# port 8000 (or --port), with caching off so a rebuild shows on reload.
set -euo pipefail
cd "$(dirname "$0")/.."

usage() {
    sed -n '2,11p' "$0" | sed 's/^# \{0,1\}//'
    exit 2
}

mode=""
serve=0
port=8000
while [ $# -gt 0 ]; do
    case "$1" in
        --private) mode=private ;;
        --public) mode=public ;;
        --serve) serve=1 ;;
        --port) port="${2:?--port needs a number}"; shift ;;
        *) usage ;;
    esac
    shift
done
[ -n "$mode" ] || usage

if [ "$mode" = private ]; then
    out=dist/web-private
    features=(--features embed-assets)
else
    out=dist/web
    features=()
fi

cargo build --locked --release --target wasm32-unknown-unknown ${features[@]+"${features[@]}"}
# Where cargo put it: CARGO_TARGET_DIR and build.target-dir move target/.
target_dir=$(tools/target_dir.sh)
wasm="$target_dir/wasm32-unknown-unknown/release/skifree.wasm"
[ -f "$wasm" ] || { echo "no $wasm after the build" >&2; exit 1; }

# gl.js from the miniquad version in Cargo.lock (its loader must match the
# crate), with its licence.
miniquad_dir=$(cargo metadata --format-version 1 --locked | python3 -c '
import json, os, sys
meta = json.load(sys.stdin)
paths = [p["manifest_path"] for p in meta["packages"] if p["name"] == "miniquad"]
if len(paths) != 1:
    sys.exit("expected exactly one miniquad package, found %d" % len(paths))
print(os.path.dirname(paths[0]))
')

rm -rf "$out"
mkdir -p "$out"
cp web/index.html web/skifree.js "$out/"
cp "$miniquad_dir/js/gl.js" "$out/gl.js"
cp "$miniquad_dir/LICENSE-MIT" "$out/gl.js.LICENSE-MIT"
cp "$miniquad_dir/LICENSE-APACHE" "$out/gl.js.LICENSE-APACHE"
# The fallback status font is CC BY-SA 4.0: its credit and licence ship too.
mkdir -p "$out/fonts"
cp resources/fonts/README.md resources/fonts/LICENSE.txt "$out/fonts/"
if command -v wasm-opt >/dev/null 2>&1; then
    # strip = true drops the target_features section, so name the features
    # rustc uses for wasm32-unknown-unknown; wasm-opt doesn't detect them.
    wasm-opt -O2 --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
        --enable-mutable-globals --enable-reference-types --enable-multivalue \
        "$wasm" -o "$out/skifree.wasm"
else
    cp "$wasm" "$out/skifree.wasm"
fi

if [ "$mode" = private ]; then
    # Replay scenarios for frame-capture mode (?replay=<name>&ticks=...).
    mkdir -p "$out/replays"
    cp replays/*.json "$out/replays/"
    echo "This build embeds copyrighted game data. Never publish or commit it." \
        > "$out/PRIVATE-DO-NOT-PUBLISH.txt"
fi

echo "built $out ($(wc -c < "$out/skifree.wasm" | tr -d ' ') bytes of wasm)"

if [ "$serve" = 1 ]; then
    lan=$( (ipconfig getifaddr en0 || hostname -I | awk '{print $1}') 2>/dev/null || true)
    echo "serving on http://localhost:$port/${lan:+ and http://$lan:$port/ (LAN)}; Ctrl-C stops"
    exec python3 -c '
import functools, http.server, sys

class NoCache(http.server.SimpleHTTPRequestHandler):
    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        super().end_headers()

handler = functools.partial(NoCache, directory=sys.argv[2])
http.server.ThreadingHTTPServer(("0.0.0.0", int(sys.argv[1])), handler).serve_forever()
' "$port" "$out"
fi
