// Actor-loop updates (FUN_004028e0) per main-tick call, for diffing against
// the port's SKIFREE_LOG_UPDATES output. Edit FROM/TO to pick the window.
const bU = Process.getModuleByName('ski32-autoplay.exe').base;
const UPD_FROM = 0, UPD_TO = 1e9;
let updCalls = 0;
Interceptor.attach(bU.add(0x1e50), { onLeave() { updCalls++; } });
Interceptor.attach(bU.add(0x28e0), {
  onEnter() {
    if (updCalls < UPD_FROM || updCalls > UPD_TO) return;
    const a = this.context.ecx;
    send({t: 'upd', tick: updCalls, cls: a.add(0x18).readU32(), st: a.add(0x1c).readU32(),
          spr: a.add(0x10).readU32(), x: a.add(0x40).readS16(), y: a.add(0x42).readS16(), z: a.add(0x44).readS16()});
  }
});
