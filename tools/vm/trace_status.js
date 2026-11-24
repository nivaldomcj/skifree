// Every repaint of the status display (FUN_00401b80) with the value strings
// it draws (TextOutA in FUN_00401e20), filed under a main-tick call: the
// call it happened in, or — for a repaint between ticks (the first paint
// when the window shows, a resize moving the status window) — the next
// call, matching the port's records (`after` keeps its historical name).
// Compare with the port's TickRecord `status` field (tools/vm/status_align.py).
//
// The call index counts FUN_00401000 entries (the timer callback, the only
// caller of FUN_00401e50) instead of hooking FUN_00401e50 itself: under the
// VM's x86 emulation, with four listeners on FUN_00401e50 the last one's
// onLeave (scenario_inputs.py's window messages) stopped running.
const bS = Process.getModuleByName('ski32-autoplay.exe').base;
//
// Repaints made while another Frida hook runs (window messages the
// injector sends: a resize repaints the status window synchronously from
// WM_SIZE) are invisible to these hooks. They still stamp DAT_0040c5dc, so
// a stamp change nobody saw is reported at the next tick's start as
// {t: 'status_hidden', after: N} (timing only, no strings).
let statusCalls = 0, statusCur = null, statusInTick = false, statusStamp = null;
Interceptor.attach(bS.add(0x1000), {
  onEnter() {
    const stamp = bS.add(0xc5dc).readU32();
    if (statusStamp !== null && stamp !== statusStamp) send({t: 'status_hidden', after: statusCalls});
    statusStamp = stamp;
    statusCalls++; statusInTick = true;
  },
  onLeave() { statusInTick = false; }
});
Interceptor.attach(bS.add(0x1b80), {
  onEnter() { statusCur = []; },
  onLeave() {
    send({t: 'status', after: statusInTick ? statusCalls - 1 : statusCalls, values: statusCur});
    statusStamp = bS.add(0xc5dc).readU32();
    statusCur = null;
  }
});
Interceptor.attach(bS.add(0x1e36), {  // call TextOutA
  onEnter() {
    if (statusCur === null) return;  // labels drawn by the WM_PAINT handler
    const sp = this.context.esp;
    statusCur.push(sp.add(12).readPointer().readAnsiString(sp.add(16).readS32()));
  }
});
