// Deterministic game clock. The original reads GetTickCount at three call
// sites: the timer callback FUN_00401000 (into DAT_0040c698, right before
// each main tick), new game FUN_00404970 and timer setup FUN_00404ad0.
// Each returns T0 + 40 * (timer callbacks so far), so wall-clock logic
// (monster eating sequence, course timers) advances exactly 40 ms per tick
// like the port's replay, however fast the VM actually runs the timer.
// (Hooking the kernel32 export itself does not work under the VM's x86
// emulation; instruction hooks in the game module do.)
//
// The timer callback's read is overwritten by a hook (it never runs inside
// another hook). The other two can run inside the injector's hooks (an F2
// new game, a resume after F3 or a focus change), where Frida fires no
// hooks, so their `call [GetTickCount]` is patched in place to read the
// fixed time from a cell instead (mov eax, [cell]; nop).
const bT = Process.getModuleByName('ski32-autoplay.exe').base;
const T0 = 1000000;
let timerCalls = 0;
const nowCell = Memory.alloc(4);
nowCell.writeU32(T0);
Interceptor.attach(bT.add(0x1000), { onEnter() { timerCalls++; nowCell.writeU32(T0 + 40 * timerCalls); } });
Interceptor.attach(bT.add(0x1006), { onEnter() { this.context.eax = ptr(T0 + 40 * timerCalls); } });
{
  const a = nowCell.toUInt32();
  for (const site of [0x4971, 0x4af5]) {
    Memory.patchCode(bT.add(site), 6, (c) => {
      c.writeByteArray([0xa1, a & 0xff, (a >>> 8) & 0xff, (a >>> 16) & 0xff, a >>> 24, 0x90]);
    });
  }
}
