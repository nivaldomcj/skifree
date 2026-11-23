#!/usr/bin/env python3
"""Run the port's Windows build in the VM in frame-capture mode.

Usage: port_frames_win.py <vm-host> <ssh-dest> <vm-dir> <scenario.json> <t1,t2,...> <out-dir>

Copies target/x86_64-pc-windows-gnu/release/skifree.exe, assets/ and the
scenario to <vm-dir> (e.g. C:/Users/<you>/Downloads/skifree-port), starts
the EXE through frida-server (which must run in the console session, so
the window gets an OpenGL desktop) with SKIFREE_REPLAY / SKIFREE_FRAMES /
SKIFREE_FRAME_TICKS set, waits for it to exit and copies the frames back
to <out-dir>. Compare with `frames.py <frames-out> --window <out-dir>`.
"""
import os
import subprocess
import sys
import time

import frida

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")


def main():
    if len(sys.argv) != 7:
        sys.exit(__doc__)
    host, ssh, vm_dir, scenario, ticks, out = sys.argv[1:]
    win_dir = vm_dir.replace("/", "\\")
    exe = os.path.join(ROOT, "target", "x86_64-pc-windows-gnu", "release", "skifree.exe")
    subprocess.run(["ssh", "-o", "BatchMode=yes", ssh,
                    f'taskkill /f /im skifree.exe >nul 2>&1 & rmdir /s /q "{win_dir}" 2>nul & mkdir "{win_dir}"'],
                   check=False)
    subprocess.run(["scp", "-q", "-r", exe, os.path.join(ROOT, "assets"), scenario,
                    f"{ssh}:{vm_dir}/"], check=True)
    dev = frida.get_device_manager().add_remote_device(f"{host}:27042")
    name = os.path.basename(scenario)
    pid = dev.spawn([f"{win_dir}\\skifree.exe"], cwd=win_dir, env={
        "SKIFREE_REPLAY": f"{win_dir}\\{name}",
        "SKIFREE_FRAMES": f"{win_dir}\\frames",
        "SKIFREE_FRAME_TICKS": ticks,
    })
    dev.resume(pid)
    for _ in range(600):
        time.sleep(1)
        if not any(p.pid == pid for p in dev.enumerate_processes()):
            break
    else:
        print("port did not exit; killing it", file=sys.stderr)
        dev.kill(pid)
    os.makedirs(out, exist_ok=True)
    subprocess.run(["scp", "-q", f"{ssh}:{vm_dir}/frames/*", out + "/"], check=True)
    print(f"frames in {out}: {sorted(os.listdir(out))}")


if __name__ == "__main__":
    main()
