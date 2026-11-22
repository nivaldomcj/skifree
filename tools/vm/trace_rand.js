// Ordered rand() call log for spawn_locked.py: caller of rand_range
// (FUN_004020b0) plus modulus and post-call LCG state.
const base3 = Process.getModuleByName('ski32-autoplay.exe').base;
const rngPtr3 = base3.add(0xc16c);
Interceptor.attach(base3.add(0x20b0), {
  onEnter(args) {
    this.n = this.context.ecx.toInt32() & 0xffff;
    this.ra = this.context.esp.readPointer().sub(base3);
  },
  onLeave(ret) {
    send({t: 'r', ra: '0x' + this.ra.add(0x400000).toString(16), n: this.n, s: rngPtr3.readU32()});
  }
});
