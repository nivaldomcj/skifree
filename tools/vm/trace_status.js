// Every repaint of the status display (FUN_00401b80) with the value strings
// it draws (TextOutA in FUN_00401e20) and the main-tick call it happened in:
// `after` = the last main-tick call that has started (-1 before the first).
// Compare with the port's TickRecord `hud` field (tools/vm/status_align.py).
//
// The call index counts FUN_00401000 entries (the timer callback, the only
// caller of FUN_00401e50) instead of hooking FUN_00401e50 itself: under the
// VM's x86 emulation, with four listeners on FUN_00401e50 the last one's
// onLeave (scenario_inputs.py's window messages) stopped running.
const bS = Process.getModuleByName('ski32-autoplay.exe').base;
let statusCalls = 0, statusCur = null;
Interceptor.attach(bS.add(0x1000), { onEnter() { statusCalls++; } });
Interceptor.attach(bS.add(0x1b80), {
  onEnter() { statusCur = []; },
  onLeave() {
    send({t: 'status', after: statusCalls - 1, values: statusCur});
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
