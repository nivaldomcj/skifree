#!/usr/bin/env python3
"""Turn a replay scenario's inputs into a Frida script for the original.

Usage: scenario_inputs.py <scenario.json> [tick-offset] > inputs.js

The port applies a scenario input at tick N right before `game.tick()` for
that tick. The generated script reproduces that in the original: it counts
calls to the main tick `FUN_00401e50` and, on entry to call N + offset,
invokes the game's own input handlers directly on the game thread:

- keyboard handler `FUN_00406170` (`ecx` = virtual-key code), the routine
  the window procedure calls for WM_KEYDOWN;
- mouse-click handler `FUN_004066d0()` (no arguments), for `click`;
- mouse-move handler `FUN_00406550` (`ecx` = x, `edx` = y, client
  coordinates), the routine behind WM_MOUSEMOVE, for `mouse`.

Calling the handlers at a fixed point of the tick, instead of posting window
messages, makes the timing exact and repeatable. Mapping (same order as
`skier::steer_keyboard`: down, up, left, right):

  steer          -> VK_DOWN 0x28, VK_UP 0x26, VK_LEFT 0x25, VK_RIGHT 0x27
  steer_absolute -> 1: End 0x23, 3: Home 0x24, 4: PgDn 0x22, 6: PgUp 0x21
  jump           -> VK_INSERT 0x2d (keyboard jump)
  click          -> FUN_004066d0()
  mouse {x, y}   -> FUN_00406550(x, y)
  restart        -> VK_F2 0x71 (new game: FUN_00406500 -> FUN_00404970 ...)
  cheat {key}    -> WM_CHAR handler FUN_00406780 (`ecx` = the character):
                    x/X/y/Y teleport, r repaint

`fast_forward` (the f cheat) has no deterministic mapping and is rejected.

Seed: ski32-autoplay.exe's srand is patched to seed 0 (`xor eax,eax; nop;
nop` at 0x406cd0). A scenario with another seed (0-127) re-patches it to
`push seed; pop eax; nop`, so every new game seeds like the port's.

Posted ticks: a tick with a `t` cheat (FUN_00406780 runs the timer
callback FUN_00401000 itself) is delivered as posted window messages
(PostMessageA) after timer callback N-1: WM_KEYDOWN, WM_CHAR, WM_MOUSEMOVE,
WM_LBUTTONDOWN, in order. The game's message loop then runs them outside
any Frida hook, so the stepped tick is traced as call N like a timer tick
(the port's record N is the stepped tick too), and an F3 pause around it
goes through KillTimer and SetTimer as it would for a player.

Window-message ticks: a tick that contains a `focus` input is delivered
differently — all of that tick's inputs become real window messages sent
with SendMessageA (synchronous) right after timer callback N-1 returns
(main tick, paint and status check), so
they go through the original's window procedure and its input gating
(DAT_0040c67c): focus -> WM_ACTIVATE (WA_ACTIVE / WA_INACTIVE), keys ->
WM_KEYDOWN (pause = VK_F3), click -> WM_LBUTTONDOWN, mouse -> WM_MOUSEMOVE.
Sending them between ticks matches the port, where a focus change applies
before that tick runs. A `resize {w, h}` input is also a window-message
tick: MoveWindow to the size whose client area is w x h (WM_SIZE is sent
synchronously, so FUN_00405fa0 re-lays out and the status window moves
and repaints before the next tick). While the window is inactive the original runs no
ticks at all, so the port's tick N is the original's next main-tick call.
"""
import json
import sys

ABSOLUTE_VK = {1: 0x23, 3: 0x24, 4: 0x22, 6: 0x21}


def actions_for(event):
    kind = event["type"]
    if kind == "steer":
        out = []
        for key, vk in (("down", 0x28), ("up", 0x26), ("left", 0x25), ("right", 0x27)):
            if event.get(key):
                out.append(["key", vk])
        return out
    if kind == "steer_absolute":
        return [["key", ABSOLUTE_VK[event["state"]]]]
    if kind == "jump":
        return [["key", 0x2D]]
    if kind == "click":
        return [["click", 0]]
    if kind == "restart":
        return [["key", 0x71]]
    if kind == "pause":
        return [["key", 0x72]]
    if kind == "focus":
        return [["focus", 1 if event["active"] else 0]]
    if kind == "mouse":
        return [["mouse", event["x"], event["y"]]]
    if kind == "resize":
        return [["resize", event["w"], event["h"]]]
    if kind == "cheat" and event["key"] in "xXyYrt" and len(event["key"]) == 1:
        return [["char", ord(event["key"])]]
    sys.exit(f"unsupported input for the original: {kind}")


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    scenario = json.load(open(sys.argv[1]))
    offset = int(sys.argv[2]) if len(sys.argv) > 2 else 0
    schedule = {}
    for ev in scenario.get("inputs", []):
        schedule.setdefault(ev["tick"] + offset, []).extend(actions_for(ev))
    # Ticks with a focus change go through the window procedure, delivered
    # after the previous main tick.
    # F3 (pause) stops the original's timer like a focus loss: a tick may
    # pause only if it also resumes, or the original runs no more ticks
    # and every comparison would pass on an empty tail. Keys are dropped
    # while the window is inactive (DAT_0040c67c); F2 unpauses.
    for tick, acts in sorted(schedule.items()):
        active, paused = True, False
        for act in acts:
            if act[0] == "focus":
                active = act[1] == 1
            elif act[0] == "key" and active:
                if act[1] == 0x72:
                    paused = not paused
                elif act[1] == 0x71:
                    paused = False
        if paused:
            sys.exit(f"tick {tick}: a pause must be undone in the same tick (F3 stops the original's timer)")
    posted = {}
    for tick in [t for t, acts in schedule.items() if ["char", ord("t")] in acts]:
        if schedule[tick].count(["char", ord("t")]) > 1:
            sys.exit(f"tick {tick}: one t step per tick (each step is a tick of its own)")
        if tick == 0:
            sys.exit("a t step needs tick >= 1")
        if any(a[0] in ("focus", "resize") for a in schedule[tick]):
            sys.exit(f"tick {tick}: a t step can't share its tick with a focus change or a resize")
        posted[tick - 1] = schedule.pop(tick)
    messages = {}
    for tick in [t for t, acts in schedule.items() if any(a[0] in ("focus", "resize") for a in acts)]:
        if tick == 0:
            sys.exit("focus changes and resizes need tick >= 1")
        # Losing the focus stops the original's timer, so no later main tick
        # would run to deliver the regain: it has to come in the same tick.
        focus = [a[1] for a in schedule[tick] if a[0] == "focus"]
        if focus and focus[-1] == 0:
            sys.exit(f"tick {tick}: a focus loss must be regained in the same tick (the timer stops)")
        messages[tick - 1] = schedule.pop(tick)
    seed = scenario.get("seed", 0)
    if not 0 <= seed < 128:
        sys.exit(f"seed {seed}: the original's srand patch takes 0-127")
    print(f"""// Generated by tools/vm/scenario_inputs.py from {sys.argv[1]}
const bIn = Process.getModuleByName('ski32-autoplay.exe').base;
if ({seed} !== 0) Memory.patchCode(bIn.add(0x6cd0), 4, (c) => c.writeByteArray([0x6a, {seed}, 0x58, 0x90]));
const keyHandler = new NativeFunction(bIn.add(0x6170), 'void', ['int', 'int'], 'fastcall');
const clickHandler = new NativeFunction(bIn.add(0x66d0), 'void', []);
const mouseHandler = new NativeFunction(bIn.add(0x6550), 'void', ['int', 'int'], 'fastcall');
const charHandler = new NativeFunction(bIn.add(0x6780), 'void', ['int'], 'fastcall');
const SCHEDULE = {json.dumps(schedule)};
const MESSAGES = {json.dumps(messages)};
const POSTED = {json.dumps(posted)};
const u32In = Process.getModuleByName('user32.dll');
const SendMessageA = new NativeFunction(u32In.getExportByName('SendMessageA'), 'int', ['pointer', 'uint', 'uint', 'int'], 'stdcall');
const PostMessageA = new NativeFunction(u32In.getExportByName('PostMessageA'), 'int', ['pointer', 'uint', 'uint', 'int'], 'stdcall');
const GetClientRectIn = new NativeFunction(u32In.getExportByName('GetClientRect'), 'bool', ['pointer', 'pointer'], 'stdcall');
const GetWindowRectIn = new NativeFunction(u32In.getExportByName('GetWindowRect'), 'bool', ['pointer', 'pointer'], 'stdcall');
const MoveWindowIn = new NativeFunction(u32In.getExportByName('MoveWindow'), 'bool', ['pointer', 'int', 'int', 'int', 'int', 'bool'], 'stdcall');
// Resize so the client area is w x h, keeping the window's top-left.
function resizeClient(hwnd, w, h) {{
  const cr = Memory.alloc(16), wr = Memory.alloc(16);
  for (let i = 0; i < 4; i++) {{
    GetClientRectIn(hwnd, cr); GetWindowRectIn(hwnd, wr);
    const cw = cr.add(8).readS32(), ch = cr.add(12).readS32();
    if (cw === w && ch === h) break;
    const ww = wr.add(8).readS32() - wr.readS32(), wh = wr.add(12).readS32() - wr.add(4).readS32();
    MoveWindowIn(hwnd, wr.readS32(), wr.add(4).readS32(), w + ww - cw, h + wh - ch, 1);
  }}
  GetClientRectIn(hwnd, cr);
  send({{t: 'resized', client: [cr.add(8).readS32(), cr.add(12).readS32()]}});
}}
function sendWindowMessages(acts) {{
  const hwnd = bIn.add(0xc6c8).readPointer();
  for (const [what, a, b] of acts) {{
    if (what === 'resize') resizeClient(hwnd, a, b);                     // MoveWindow -> WM_SIZE
    else if (what === 'focus') SendMessageA(hwnd, 0x0006, a, 0);       // WM_ACTIVATE
    else if (what === 'key') SendMessageA(hwnd, 0x0100, a, 0);         // WM_KEYDOWN
    else if (what === 'mouse') SendMessageA(hwnd, 0x0200, 0, (b << 16) | (a & 0xffff)); // WM_MOUSEMOVE
    else SendMessageA(hwnd, 0x0201, 1, 0);                              // WM_LBUTTONDOWN
  }}
}}
// Inputs run inside a Frida hook, where hooks don't fire. fixed_clock.js
// patches the new-game and resume GetTickCount reads in place, so they
// already return the fixed time there; this rewrite of DAT_0040c698 is a
// safeguard that keeps the fixed clock's time of this tick.
function restoreFixedNow() {{
  if (typeof T0 !== 'undefined') bIn.add(0xc698).writeU32(T0 + 40 * timerCalls);
}}
let mainTickCalls = 0;
Interceptor.attach(bIn.add(0x1e50), {{
  onEnter() {{
    const acts = SCHEDULE[mainTickCalls];
    if (acts) {{
      for (const [what, a, b] of acts) {{
        if (what === 'key') keyHandler(a, 0);
        else if (what === 'mouse') mouseHandler(a, b);
        else if (what === 'char') charHandler(a);
        else clickHandler();
      }}
      restoreFixedNow();
      send({{t: 'input', call: mainTickCalls, acts: acts}});
    }}
    mainTickCalls++;
  }},
}});
// Window messages arrive after the whole timer callback FUN_00401000 (main
// tick, paint, 327 ms status check), where a real message loop delivers
// them, not between the main tick and its paint.
Interceptor.attach(bIn.add(0x1000), {{
  onLeave() {{
    const post = POSTED[mainTickCalls - 1];
    if (post) {{
      const hwnd = bIn.add(0xc6c8).readPointer();
      for (const [what, a, b] of post) {{
        if (what === 'key') PostMessageA(hwnd, 0x0100, a, 0);              // WM_KEYDOWN
        else if (what === 'char') PostMessageA(hwnd, 0x0102, a, 0);        // WM_CHAR
        else if (what === 'mouse') PostMessageA(hwnd, 0x0200, 0, (b << 16) | (a & 0xffff));
        else PostMessageA(hwnd, 0x0201, 1, 0);                             // WM_LBUTTONDOWN
      }}
      send({{t: 'posted', after: mainTickCalls - 1, acts: post}});
    }}
    const acts = MESSAGES[mainTickCalls - 1];
    if (!acts) return;
    // A resize repaints the status window inside this hook (invisible to
    // trace_status.js) and may re-stamp DAT_0040c5dc with the value it
    // already holds. A sentinel tells a re-stamp from none.
    const stampAt = bIn.add(0xc5dc), stamp = stampAt.readU32();
    stampAt.writeU32(0);
    sendWindowMessages(acts);
    if (stampAt.readU32() === 0) stampAt.writeU32(stamp);
    else if (typeof statusStamp !== 'undefined') statusStamp = -1;  // trace_status.js reports it next call
    restoreFixedNow();
    send({{t: 'winmsg', after: mainTickCalls - 1, acts: acts}});
  }}
}});""")


if __name__ == "__main__":
    main()
