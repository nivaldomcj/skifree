// Player snapshot at the end of every main-tick call (FUN_00401e50), keyed
// by call index: call N corresponds to the port's replay tick N. Unlike
// trace_ticks.js this keeps counting while the skier is crashed (state 0xb
// is not integrated) or gone. DAT_0040c72c holds the player actor; it is
// cleared only after the tick, so a player flagged 0x08 (being removed, e.g.
// just eaten) is reported as gone — the port drops it within the tick.
const bP = Process.getModuleByName('ski32-autoplay.exe').base;
let playerCalls = 0;
Interceptor.attach(bP.add(0x1e50), {
  onLeave() {
    const p = bP.add(0xc72c).readPointer();
    const rec = {t: 'p', call: playerCalls++, rng: bP.add(0xc16c).readU32(),
                 score: bP.add(0xc6a8).readS32(),   // DAT_0040c6a8 style points
                 // course clock DAT_0040c944 (ms) and the course-running
                 // flags [freestyle DAT_0040c954, tree slalom DAT_0040c958,
                 // slalom DAT_0040c95c] (FUN_00403180 / 00403250 / 00402c60)
                 cms: bP.add(0xc944).readS32(),
                 runs: [bP.add(0xc954).readU32(), bP.add(0xc958).readU32(), bP.add(0xc95c).readU32()]};
    if (!p.isNull() && (p.add(0x4c).readU32() & 0x08) === 0) {
      rec.x = p.add(0x40).readS16(); rec.y = p.add(0x42).readS16(); rec.z = p.add(0x44).readS16();
      rec.vx = p.add(0x46).readS16(); rec.vy = p.add(0x48).readS16(); rec.vz = p.add(0x4a).readS16();
      rec.state = p.add(0x1c).readU32();
    }
    send(rec);
  }
});
