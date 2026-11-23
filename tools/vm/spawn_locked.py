#!/usr/bin/env python3
"""Spawn the original game in the VM under Frida with an exact client size.

Usage: spawn_locked.py <exe-path-in-vm> <seconds> <vm-host> [--size WxH]
                       [--data-dir DIR] [extra.js ...]

The client area is forced to WxH (default 704x681) *before the world is
built*: the prelude hooks the world build FUN_00404b50 and, on entry, resizes
the window on the game thread (MoveWindow sends WM_SIZE synchronously, so the
game's client-rect globals are updated before the build reads them). The
window is created at a size derived from the screen, and its client size
varies between launches, so resizing later would leave the world built at a
different size than the one played at. Match the port's scenario
screen_w/screen_h to the size used here.

Loads each extra JS file into the same Frida script (after the prelude) and
prints every `send()` payload as a Python dict, one per line. Messages that
carry binary data (capture_frames.js) are saved under --data-dir as
`frame_<tick>.raw`, and the printed payload gets the file name. The game is
killed when the time is up, so every run is a fresh process — F2 restarts are
NOT equivalent to fresh worlds (see docs/vm-ground-truth.md).
"""
import os
import sys
import threading
import time

import frida

LOCK_JS = """
const bL = Process.getModuleByName('ski32-autoplay.exe').base;
const user32 = Process.getModuleByName('user32.dll');
function fn(name, ret, args) { return new NativeFunction(user32.getExportByName(name), ret, args, 'stdcall'); }
const GetClientRect = fn('GetClientRect', 'bool', ['pointer', 'pointer']);
const GetWindowRect = fn('GetWindowRect', 'bool', ['pointer', 'pointer']);
const MoveWindow = fn('MoveWindow', 'bool', ['pointer', 'int', 'int', 'int', 'int', 'bool']);
const ClientToScreen = fn('ClientToScreen', 'bool', ['pointer', 'pointer']);
const GetSystemMetrics = fn('GetSystemMetrics', 'int', ['int']);
const SetWindowPos = fn('SetWindowPos', 'bool', ['pointer', 'pointer', 'int', 'int', 'int', 'int', 'uint']);
const LOCK_W = %d, LOCK_H = %d;
function fitClient() {
  const hwnd = bL.add(0xc6c8).readPointer();  // DAT_0040c6c8: main window
  const cr = Memory.alloc(16), wr = Memory.alloc(16);
  for (let i = 0; i < 4; i++) {
    GetClientRect(hwnd, cr); GetWindowRect(hwnd, wr);
    const cw = cr.add(8).readS32(), ch = cr.add(12).readS32();
    if (cw === LOCK_W && ch === LOCK_H) break;
    const ww = wr.add(8).readS32() - wr.readS32(), wh = wr.add(12).readS32() - wr.add(4).readS32();
    MoveWindow(hwnd, 200, 30, LOCK_W + ww - cw, LOCK_H + wh - ch, 1);
  }
  // Keep the whole client area on screen: GDI does not paint rows that
  // hang off the screen, so frame captures (capture_frames.js) would show
  // them white. Moving keeps the client size (no WM_SIZE).
  const pt = Memory.alloc(8);
  pt.writeS32(0); pt.add(4).writeS32(0); ClientToScreen(hwnd, pt);
  const over = pt.add(4).readS32() + LOCK_H - GetSystemMetrics(1);  // SM_CYSCREEN
  if (over > 0) {
    GetWindowRect(hwnd, wr);
    SetWindowPos(hwnd, NULL, wr.readS32(), wr.add(4).readS32() - over, 0, 0, 0x15);  // NOSIZE|NOZORDER|NOACTIVATE
  }
  GetClientRect(hwnd, cr);
  return [cr.add(8).readS32(), cr.add(12).readS32()];
}
Interceptor.attach(bL.add(0x4b50), {
  onEnter() { send({t: 'locked', client: fitClient()}); }
});
"""


def main():
    if len(sys.argv) < 4:
        sys.exit(__doc__)
    exe, duration, host = sys.argv[1], float(sys.argv[2]), sys.argv[3]
    rest = sys.argv[4:]
    size = (704, 681)
    if len(rest) >= 2 and rest[0] == "--size":
        size = tuple(int(v) for v in rest[1].lower().split("x"))
        rest = rest[2:]
    data_dir = None
    if len(rest) >= 2 and rest[0] == "--data-dir":
        data_dir = rest[1]
        os.makedirs(data_dir, exist_ok=True)
        rest = rest[2:]
    extra = "\n".join(open(p).read() for p in rest)
    dev = frida.get_device_manager().add_remote_device(f"{host}:27042")
    pid = dev.spawn([exe])
    session = dev.attach(pid)
    script = session.create_script(LOCK_JS % size + "\n" + extra)

    def on_message(msg, _data):
        if msg.get("type") == "send":
            payload = msg["payload"]
            if _data is not None and data_dir is not None:
                name = f"{payload.get('t', 'data')}_{payload.get('tick', 0)}.raw"
                with open(os.path.join(data_dir, name), "wb") as f:
                    f.write(_data)
                payload["file"] = name
            print(payload, flush=True)
        else:
            print("script error:", msg, file=sys.stderr, flush=True)

    script.on("message", on_message)
    script.load()
    dev.resume(pid)
    time.sleep(duration)

    # Frida's detach/kill can hang against the VM; give them a few seconds,
    # then exit regardless (the next run kills leftovers anyway).
    def teardown():
        try:
            session.detach()
            dev.kill(pid)
        except Exception as exc:  # noqa: BLE001 - best effort
            print(f"teardown: {exc}", file=sys.stderr)

    t = threading.Thread(target=teardown, daemon=True)
    t.start()
    t.join(10)
    sys.stdout.flush()
    sys.stderr.flush()
    os._exit(0)


if __name__ == "__main__":
    main()
