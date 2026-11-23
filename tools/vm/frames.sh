#!/bin/sh
# Capture frames of the original and the port at the same ticks of a
# scenario, and compare them pixel by pixel.
#
#   tools/vm/frames.sh <scenario.json> <out-dir> <t1,t2,...> [seconds]
#
# Same environment as compare.sh (SKIFREE_VM_HOST, SKIFREE_VM_EXE, ...).
# Writes <out-dir>/orig/frame_<t>.raw (the original's client area after tick
# t), <out-dir>/port_draw.jsonl (the port's draw lists) and, via frames.py,
# orig_<t>.png, port_<t>.png, diff_<t>.png and side_<t>.png.
set -e
cd "$(dirname "$0")/../.."
: "${SKIFREE_VM_HOST:?set SKIFREE_VM_HOST}"
: "${SKIFREE_VM_EXE:?set SKIFREE_VM_EXE}"
SSH_DEST=${SKIFREE_VM_SSH:-$SKIFREE_VM_HOST}
FRIDA_PY=${FRIDA_PY:-"$HOME/Library/Application Support/pipx/venvs/frida-tools/bin/python"}

scenario=$1; out=${2:?out-dir}; ticks=${3:?ticks}; secs=${4:-40}
mkdir -p "$out"
size=$(python3 -c "import json,sys; d=json.load(open(sys.argv[1])); print('%dx%d' % (d.get('screen_w', 640), d.get('screen_h', 480)))" "$scenario")
echo "const CAPTURE_TICKS = [$ticks];" > "$out/ticks.js"
python3 tools/vm/scenario_inputs.py "$scenario" > "$out/inputs.js"
ssh -o BatchMode=yes "$SSH_DEST" 'taskkill /f /im ski32-autoplay.exe' >/dev/null 2>&1 || true
"$FRIDA_PY" tools/vm/spawn_locked.py "$SKIFREE_VM_EXE" "$secs" "$SKIFREE_VM_HOST" --size "$size" \
    --data-dir "$out/orig" \
    "$out/ticks.js" tools/vm/capture_frames.js tools/vm/fixed_clock.js tools/vm/no_dialogs.js \
    "$out/inputs.js" > "$out/orig.txt" 2>&1
cargo build -q --bin replay
./target/debug/replay "$scenario" --draw "$out/port_draw.jsonl" --draw-ticks "$ticks" > /dev/null
python3 tools/vm/frames.py "$out"
