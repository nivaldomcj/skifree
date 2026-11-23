// Every sound the original plays (FUN_00402ba0, ecx = the sound's slot),
// filed under the main-tick call it happened in. FUN_00402ba0 is called at
// every event even in the shipped ski32.exe, whose WAVE resources are
// missing: the null check comes after the call, so this works on
// ski32-autoplay.exe. Compare with the port's TickRecord `sounds` field
// (tools/vm/sound_align.py).
//
// Load after trace_status.js: it reuses that script's main-tick counter
// (statusCalls, counted on FUN_00401000) instead of adding another listener.
const sndBase = Process.getModuleByName('ski32-autoplay.exe').base;
// Slot address -> WAVE resource id (FUN_004052d0's FUN_00405640 calls).
const sndSlots = {0xc6c0: 1, 0xc768: 2, 0xc5d0: 3, 0xc718: 4, 0xc750: 5,
                  0xc628: 6, 0xc6e0: 7, 0xc608: 8, 0xc6f0: 9};
Interceptor.attach(sndBase.add(0x2ba0), {
  onEnter() {
    const slot = this.context.ecx.sub(sndBase).toInt32();
    send({t: 'sound', tick: statusInTick ? statusCalls - 1 : statusCalls,
          id: sndSlots[slot] || -slot});
  }
});
