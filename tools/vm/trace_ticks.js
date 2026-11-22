// Player state per integrator call (one record per *integrated* tick —
// a crashed skier in state 0xb is not integrated) plus skier collisions.
// Defines `tick2`, used by the other trace scripts.
const base2 = Process.getModuleByName('ski32-autoplay.exe').base;
const integrator2 = base2.add(0x2be0);
const collide2 = base2.add(0x3a00);
const rngPtr2 = base2.add(0xc16c);
let tick2 = 0;
Interceptor.attach(integrator2, {
  onEnter(args) {
    const p = this.context.ecx;
    try {
      if (p.add(0x18).readU32() !== 0) return;
      send({t: 'tick', tick: tick2,
            x: p.add(0x40).readS16(), y: p.add(0x42).readS16(),
            z: p.add(0x44).readS16(), vx: p.add(0x46).readS16(),
            vy: p.add(0x48).readS16(), vz: p.add(0x4a).readS16(),
            state: p.add(0x1c).readU32(), rng: rngPtr2.readU32()});
      tick2++;
    } catch (e) {}
  }
});
Interceptor.attach(collide2, {
  onEnter(args) {
    const a = this.context.ecx, b = this.context.edx;
    let ac, bc, bs, bx, by;
    try { ac = a.add(0x18).readU32(); } catch(e) { return; }
    try { bc = b.add(0x18).readU32(); bs = b.add(0x10).readU32(); bx = b.add(0x40).readS16(); by = b.add(0x42).readS16(); } catch(e) { return; }
    if (ac === 0) {
      send({t: 'col', tick: tick2, bc: bc, bspr: bs, bx: bx, by: by});
    }
  }
});
