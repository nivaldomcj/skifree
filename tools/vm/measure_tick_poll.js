// Hook-free variant of measure_tick.js: nothing runs on the game thread.
// Frida's own thread polls DAT_0040c698 (GetTickCount at the latest timer
// callback) and DAT_0040c708 (the one before) every 2 ms and records each
// new pair, so every tick's delta is seen as long as ticks are > 2 ms
// apart. Sends batches of 100 deltas (ms).
const pBase = Process.getModuleByName('ski32-autoplay.exe').base;
let pLast = 0, pDeltas = [];
setInterval(() => {
  const now = pBase.add(0xc698).readU32(), prev = pBase.add(0xc708).readU32();
  if (now !== pLast && prev !== 0) {
    if (pLast !== 0 && prev !== pLast) pDeltas.push(-1);  // missed a tick
    pDeltas.push(now - prev);
    pLast = now;
    if (pDeltas.length >= 100) { send({t: 'poll', ms: pDeltas}); pDeltas = []; }
  }
}, 2);
