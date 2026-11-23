#!/bin/sh
# Compare a replay scenario against the original ski32.exe running in the VM.
#
#   tools/vm/compare.sh <scenario.json> <out-prefix> [seconds]
#   tools/vm/compare.sh --suite <out-dir>
#
# Environment (required):
#   SKIFREE_VM_HOST  host running frida-server on :27042 (and SSH)
#   SKIFREE_VM_EXE   path of ski32-autoplay.exe inside the VM
# Optional:
#   SKIFREE_VM_SSH   ssh destination for killing leftovers (default: $SKIFREE_VM_HOST)
#   FRIDA_PY         python with the frida module (default: frida-tools' pipx venv)
#
# The original runs as a fresh process with its client area forced to the
# scenario's screen_w x screen_h before the world build, the scenario's inputs
# injected at the matching main-tick calls, and a fixed 40 ms/tick clock.
# Prints the first diverging rand() call, player tick, status repaint and sound.
set -e
cd "$(dirname "$0")/../.."
: "${SKIFREE_VM_HOST:?set SKIFREE_VM_HOST}"
: "${SKIFREE_VM_EXE:?set SKIFREE_VM_EXE}"
SSH_DEST=${SKIFREE_VM_SSH:-$SKIFREE_VM_HOST}
FRIDA_PY=${FRIDA_PY:-"$HOME/Library/Application Support/pipx/venvs/frida-tools/bin/python"}

if [ "$1" = "--suite" ]; then
    out=${2:?out-dir}; mkdir -p "$out"
    # scenario:seconds (seconds cover the scenario's ticks at ~19 ticks/s
    # under Frida; the whole suite takes ~16 minutes)
    for entry in compare_vm:32 compare_steer:32 compare_size_640x480:34 \
                 compare_size_1024x700:34 compare_slalom:66 \
                 compare_tree_slalom:66 compare_slalom_timed:68 \
                 compare_tree_slalom_timed:68 compare_freestyle:68 \
                 compare_mouse:68 compare_restart:40 compare_focus:40 compare_resize:42 \
                 compare_monster:72 compare_sounds:42 \
                 compare_long:240; do
        name=${entry%%:*}; secs=${entry#*:}
        echo "== $name"
        "$0" "replays/$name.json" "$out/$name" "$secs"
    done
    exit 0
fi

scenario=$1; prefix=$2; secs=${3:-30}
size=$(python3 -c "import json,sys; d=json.load(open(sys.argv[1])); print('%dx%d' % (d.get('screen_w', 640), d.get('screen_h', 480)))" "$scenario")
python3 tools/vm/scenario_inputs.py "$scenario" > "$prefix.inputs.js"
ssh -o BatchMode=yes "$SSH_DEST" 'taskkill /f /im ski32-autoplay.exe' >/dev/null 2>&1 || true
"$FRIDA_PY" tools/vm/spawn_locked.py "$SKIFREE_VM_EXE" "$secs" "$SKIFREE_VM_HOST" --size "$size" \
    tools/vm/trace_rand.js tools/vm/trace_player.js tools/vm/trace_status.js tools/vm/trace_sounds.js \
    tools/vm/fixed_clock.js tools/vm/no_dialogs.js "$prefix.inputs.js" > "$prefix.orig.txt" 2>&1
cargo build -q --features rng_sites --bin replay
SKIFREE_LOG_RNG=1 ./target/debug/replay "$scenario" 2> "$prefix.port_rng.txt" > "$prefix.port.jsonl"
python3 tools/vm/rng_align.py "$prefix.orig.txt" "$prefix.port_rng.txt" 10
python3 tools/vm/player_align.py "$prefix.orig.txt" "$prefix.port.jsonl" 6
python3 tools/vm/status_align.py "$prefix.orig.txt" "$prefix.port.jsonl" || true
python3 tools/vm/sound_align.py "$prefix.orig.txt" "$prefix.port.jsonl" || true
