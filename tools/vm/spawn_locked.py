#!/usr/bin/env python3
"""Spawn the original game in the VM under Frida, client area locked to 704x681.

Usage: spawn_locked.py <exe-path-in-vm> <seconds> <vm-host> [extra.js ...]

Loads each extra JS file into the same Frida script (after the window-lock
prelude) and prints every `send()` payload as a Python dict, one per line.
The game is killed when the time is up, so every run is a fresh process —
F2 restarts are NOT equivalent to fresh worlds (see docs/vm-ground-truth.md).
"""
import sys
import time

import frida

LOCK_JS = """
const user32 = Process.getModuleByName('user32.dll');
function fn(name, ret, args) { return new NativeFunction(user32.getExportByName(name), ret, args, 'stdcall'); }
const FindWindowW = fn('FindWindowW', 'pointer', ['pointer', 'pointer']);
const GetClientRect = fn('GetClientRect', 'bool', ['pointer', 'pointer']);
const GetWindowRect = fn('GetWindowRect', 'bool', ['pointer', 'pointer']);
const MoveWindow = fn('MoveWindow', 'bool', ['pointer', 'int', 'int', 'int', 'int', 'bool']);
function lockWindow(targetW, targetH) {
  const hwnd = FindWindowW(NULL, Memory.allocUtf16String('SkiFree'));
  if (hwnd.isNull()) return false;
  const cr = Memory.alloc(16), wr = Memory.alloc(16);
  GetClientRect(hwnd, cr);
  GetWindowRect(hwnd, wr);
  const cw = cr.add(8).readS32(), ch = cr.add(12).readS32();
  const ww = wr.add(8).readS32() - wr.readS32();
  const wh = wr.add(12).readS32() - wr.add(4).readS32();
  const ok = MoveWindow(hwnd, 200, 30, targetW + ww - cw, targetH + wh - ch, 1);
  GetClientRect(hwnd, cr);
  send({t: 'locked', ok: ok, client: [cr.add(8).readS32(), cr.add(12).readS32()]});
  return true;
}
let locked = false;
const iv = setInterval(function () { if (!locked) locked = lockWindow(704, 681); else clearInterval(iv); }, 250);
"""


def main():
    if len(sys.argv) < 4:
        sys.exit(__doc__)
    exe, duration, host = sys.argv[1], float(sys.argv[2]), sys.argv[3]
    extra = "\n".join(open(p).read() for p in sys.argv[4:])
    dev = frida.get_device_manager().add_remote_device(f"{host}:27042")
    pid = dev.spawn([exe])
    session = dev.attach(pid)
    script = session.create_script(LOCK_JS + "\n" + extra)

    def on_message(msg, _data):
        if msg.get("type") == "send":
            print(msg["payload"], flush=True)
        else:
            print("script error:", msg, file=sys.stderr, flush=True)

    script.on("message", on_message)
    script.load()
    dev.resume(pid)
    time.sleep(duration)
    session.detach()
    dev.kill(pid)


if __name__ == "__main__":
    main()
