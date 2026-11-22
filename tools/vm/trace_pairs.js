// Every collision-handler call (FUN_00403a00, ecx = first actor, edx =
// second) per main-tick call, with both actors' class/sprite/flags. Edit
// FROM/TO to pick the window.
const bQ = Process.getModuleByName('ski32-autoplay.exe').base;
const PAIR_FROM = 0, PAIR_TO = 1e9;
let pairCalls = 0;
Interceptor.attach(bQ.add(0x1e50), { onLeave() { pairCalls++; } });
Interceptor.attach(bQ.add(0x3a00), {
  onEnter() {
    if (pairCalls < PAIR_FROM || pairCalls > PAIR_TO) return;
    const a = this.context.ecx, b = this.context.edx;
    const d = p => [p.add(0x18).readU32(), p.add(0x10).readU32(), p.add(0x1c).readU32(), p.add(0x4c).readU32(),
                    p.add(0x40).readS16(), p.add(0x42).readS16(), p.add(0x44).readS16()];
    send({t: 'pair', tick: pairCalls, a: d(a), b: d(b)});
  }
});
