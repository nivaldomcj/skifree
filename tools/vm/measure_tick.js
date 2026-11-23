// Real tick rate of the original: no fixed_clock.js. At every timer
// callback (FUN_00401000, SetTimer(hwnd, 666, 40, ...)) record the time
// with QueryPerformanceCounter, and at every main tick the frame delta the
// game itself computed from GetTickCount (DAT_0040c5f4). Sends batches of
// 100 callbacks, plus the system timer resolution (NtQueryTimerResolution,
// 100 ns units) at the start.
const mBase = Process.getModuleByName('ski32-autoplay.exe').base;
const mK32 = Process.getModuleByName('kernel32.dll'), mNt = Process.getModuleByName('ntdll.dll');
const mQPC = new NativeFunction(mK32.getExportByName('QueryPerformanceCounter'), 'bool', ['pointer'], 'stdcall');
const mQPF = new NativeFunction(mK32.getExportByName('QueryPerformanceFrequency'), 'bool', ['pointer'], 'stdcall');
const mNtRes = new NativeFunction(mNt.getExportByName('NtQueryTimerResolution'), 'uint', ['pointer', 'pointer', 'pointer'], 'stdcall');
const mBuf = Memory.alloc(8);
mQPF(mBuf);
const mFreq = mBuf.readU64().toNumber();
const r = Memory.alloc(12);
mNtRes(r, r.add(4), r.add(8));
send({t: 'res', coarsest: r.readU32(), finest: r.add(4).readU32(), current: r.add(8).readU32(), qpf: mFreq});
let mTimes = [], mDeltas = [];
Interceptor.attach(mBase.add(0x1000), { onEnter() {
  mQPC(mBuf);
  mTimes.push(mBuf.readU64().toNumber());
  if (mTimes.length === 101) {
    const iv = [];
    for (let i = 1; i < mTimes.length; i++) iv.push((mTimes[i] - mTimes[i - 1]) * 1000 / mFreq);
    send({t: 'batch', ms: iv, game_deltas: mDeltas});
    mTimes = [mTimes[100]]; mDeltas = [];
  }
}});
Interceptor.attach(mBase.add(0x1e50), { onEnter() { mDeltas.push(mBase.add(0xc5f4).readU32()); } });
