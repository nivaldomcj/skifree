// Deterministic game clock. The original reads GetTickCount at three call
// sites: the timer callback FUN_00401000 (into DAT_0040c698, right before
// each main tick), new game FUN_00404970 and timer setup FUN_00404ad0.
// Overwrite eax right after each call with T0 + 40 * (timer callbacks so
// far), so wall-clock logic (monster eating sequence, course timers)
// advances exactly 40 ms per tick like the port's replay, however fast the
// VM actually runs the timer. (Hooking the kernel32 export itself does not
// work under the VM's x86 emulation; instruction hooks in the game module
// do.)
const bT = Process.getModuleByName('ski32-autoplay.exe').base;
const T0 = 1000000;
let timerCalls = 0;
Interceptor.attach(bT.add(0x1000), { onEnter() { timerCalls++; } });
for (const after of [0x1006, 0x4977, 0x4afb]) {
  Interceptor.attach(bT.add(after), { onEnter() { this.context.eax = ptr(T0 + 40 * timerCalls); } });
}
