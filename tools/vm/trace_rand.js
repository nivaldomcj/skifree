// Ordered rand() call log for spawn_locked.py: caller of rand_range
// (FUN_004020b0), modulus, post-call LCG state, and the main-tick call index
// (`call` N = the port's tick N; calls made before the first main tick, i.e.
// the world build, get call -1).
const base3 = Process.getModuleByName('ski32-autoplay.exe').base;
const rngPtr3 = base3.add(0xc16c);
let randTick = -1;
Interceptor.attach(base3.add(0x1e50), { onEnter() { randTick++; } });
Interceptor.attach(base3.add(0x20b0), {
  onEnter(args) {
    this.n = this.context.ecx.toInt32() & 0xffff;
    this.ra = this.context.esp.readPointer().sub(base3);
  },
  onLeave(ret) {
    send({t: 'r', call: randTick, ra: '0x' + this.ra.add(0x400000).toString(16), n: this.n, s: rngPtr3.readU32()});
  }
});
