// Grab the original's client area (sprites and the status display) as it
// is on screen after main tick N, for every N in CAPTURE_TICKS (defined by
// the prelude tools/vm/frames.sh writes). Each frame is sent as
// {t: 'frame', tick, w, h} with the raw pixels (32-bit BGRX, top-down) as
// the message data; spawn_locked.py --data-dir saves them.
//
// The frame after tick N is read on entry to main-tick call N + 1
// (FUN_00401e50): call N's FUN_00401000 has updated (401e50), repainted
// (401060) and maybe repainted the status display (401b80) by then. Load this
// script before the scenario's inputs file, so its hook runs before that
// call's injected keys (FUN_00406170 can repaint on a key).
const capBase = Process.getModuleByName('ski32-autoplay.exe').base;
const capU32 = Process.getModuleByName('user32.dll');
const capGdi = Process.getModuleByName('gdi32.dll');
function capFn(mod, name, ret, args) {
  return new NativeFunction(mod.getExportByName(name), ret, args, 'stdcall');
}
const capGetDC = capFn(capU32, 'GetDC', 'pointer', ['pointer']);
const capReleaseDC = capFn(capU32, 'ReleaseDC', 'int', ['pointer', 'pointer']);
const capGetClientRect = capFn(capU32, 'GetClientRect', 'bool', ['pointer', 'pointer']);
const capCreateDC = capFn(capGdi, 'CreateCompatibleDC', 'pointer', ['pointer']);
const capCreateDIB = capFn(capGdi, 'CreateDIBSection', 'pointer',
  ['pointer', 'pointer', 'uint', 'pointer', 'pointer', 'uint']);
const capSelect = capFn(capGdi, 'SelectObject', 'pointer', ['pointer', 'pointer']);
const capBitBlt = capFn(capGdi, 'BitBlt', 'bool',
  ['pointer', 'int', 'int', 'int', 'int', 'pointer', 'int', 'int', 'uint']);
const capDeleteObject = capFn(capGdi, 'DeleteObject', 'bool', ['pointer']);
const capDeleteDC = capFn(capGdi, 'DeleteDC', 'bool', ['pointer']);
const capGdiFlush = capFn(capGdi, 'GdiFlush', 'bool', []);

function capGrab(tick) {
  const hwnd = capBase.add(0xc6c8).readPointer();  // DAT_0040c6c8: main window
  const rc = Memory.alloc(16);
  capGetClientRect(hwnd, rc);
  const w = rc.add(8).readS32(), h = rc.add(12).readS32();
  const bmi = Memory.alloc(40);  // BITMAPINFOHEADER, 32 bpp, top-down
  bmi.writeU32(40); bmi.add(4).writeS32(w); bmi.add(8).writeS32(-h);
  bmi.add(12).writeU16(1); bmi.add(14).writeU16(32); bmi.add(16).writeU32(0);
  const bitsPtr = Memory.alloc(Process.pointerSize);
  const src = capGetDC(hwnd);
  const mem = capCreateDC(src);
  const dib = capCreateDIB(src, bmi, 0, bitsPtr, NULL, 0);
  const old = capSelect(mem, dib);
  capBitBlt(mem, 0, 0, w, h, src, 0, 0, 0x00cc0020);  // SRCCOPY
  capGdiFlush();
  const pixels = bitsPtr.readPointer().readByteArray(w * h * 4);
  capSelect(mem, old);
  capDeleteObject(dib);
  capDeleteDC(mem);
  capReleaseDC(hwnd, src);
  send({t: 'frame', tick: tick, w: w, h: h}, pixels);
}

const capWanted = new Set(CAPTURE_TICKS);
let capCalls = 0;
Interceptor.attach(capBase.add(0x1e50), {
  onEnter() {
    if (capWanted.has(capCalls - 1)) {
      try { capGrab(capCalls - 1); } catch (e) { send({t: 'frame_error', tick: capCalls - 1, err: '' + e}); }
    }
    capCalls++;
  }
});
