#!/usr/bin/env python3
"""Run the port's Windows build in the VM in frame-capture mode.

Usage: port_frames_win.py <ssh-dest> <vm-dir> <scenario.json> <t1,t2,...> <out-dir>

Copies target/x86_64-pc-windows-gnu/release/skifree.exe, assets/ and the
scenario to <vm-dir> (e.g. C:/Users/<you>/Downloads/skifree-port), starts
the EXE in the console session (so the window gets an OpenGL desktop)
through a one-shot interactive scheduled task running a .cmd that sets
SKIFREE_REPLAY / SKIFREE_FRAMES / SKIFREE_FRAME_TICKS (the VM's x86
frida-server can't spawn x64 programs), waits for it to exit and copies
the frames back to <out-dir>. PORT_EXTRA_ENV="NAME=value ..." adds
variables (e.g. SKIFREE_REPLAY_DIALOGS=1 to show the high-score box). Compare with `frames.py <frames-out> --window <out-dir>`.
"""
import os
import subprocess
import sys
import time


ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")


def main():
    if len(sys.argv) != 6:
        sys.exit(__doc__)
    ssh, vm_dir, scenario, ticks, out = sys.argv[1:]
    win_dir = vm_dir.replace("/", "\\")
    exe = os.path.join(ROOT, "target", "x86_64-pc-windows-gnu", "release", "skifree.exe")
    subprocess.run(["ssh", "-o", "BatchMode=yes", ssh,
                    f'taskkill /f /im skifree.exe >nul 2>&1 & rmdir /s /q "{win_dir}" 2>nul & mkdir "{win_dir}"'],
                   check=False)
    subprocess.run(["scp", "-q", "-r", exe, os.path.join(ROOT, "assets"), scenario,
                    f"{ssh}:{vm_dir}/"], check=True)
    name = os.path.basename(scenario)
    cmd = (f'@echo off\r\ncd /d "{win_dir}"\r\n'
           f'set SKIFREE_REPLAY={win_dir}\\{name}\r\n'
           f'set SKIFREE_FRAMES={win_dir}\\frames\r\n'
           f'set SKIFREE_FRAME_TICKS={ticks}\r\n'
           + ''.join(f'set {kv}\r\n' for kv in os.environ.get("PORT_EXTRA_ENV", "").split())
           +
           f'skifree.exe > run.log 2>&1\r\necho done >> run.log\r\n')
    local_cmd = os.path.join(out + ".run.cmd")
    os.makedirs(os.path.dirname(os.path.abspath(local_cmd)), exist_ok=True)
    with open(local_cmd, "w", newline="") as f:
        f.write(cmd)
    subprocess.run(["scp", "-q", local_cmd, f"{ssh}:{vm_dir}/run.cmd"], check=True)
    ssh_run = lambda c: subprocess.run(["ssh", "-o", "BatchMode=yes", ssh, c],
                                       capture_output=True, text=True)
    r = ssh_run(f'schtasks /create /f /tn SkiFreePortFrames /it /sc once /st 00:00 '
                f'/tr "{win_dir}\\run.cmd" & schtasks /run /tn SkiFreePortFrames')
    print(r.stdout.strip(), r.stderr.strip())
    for _ in range(600):
        time.sleep(1)
        if "done" in ssh_run(f'type "{win_dir}\\run.log" 2>nul').stdout:
            break
    else:
        print("port did not finish; killing it", file=sys.stderr)
        ssh_run("taskkill /f /im skifree.exe")
    print(ssh_run(f'type "{win_dir}\\run.log"').stdout.strip())
    ssh_run("schtasks /delete /f /tn SkiFreePortFrames")
    os.makedirs(out, exist_ok=True)
    subprocess.run(["scp", "-q", f"{ssh}:{vm_dir}/frames/*", out + "/"], check=True)
    print(f"frames in {out}: {sorted(os.listdir(out))}")


if __name__ == "__main__":
    main()
