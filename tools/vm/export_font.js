// One-off: render printable ASCII (0x20-0x7e) with the stock OEM fixed font
// (GetStockObject(OEM_FIXED_FONT), what the status display uses) into a
// 32-bit DIB, black on white, and send the pixels with the text metrics.
// Run with spawn_locked.py --data-dir; tools/vm/make_font.py turns the
// result into assets/fonts/oem_fixed.png.
const fGdi = Process.getModuleByName('gdi32.dll');
function fFn(name, ret, args) { return new NativeFunction(fGdi.getExportByName(name), ret, args, 'stdcall'); }
const fCreateDC = fFn('CreateCompatibleDC', 'pointer', ['pointer']);
const fCreateDIB = fFn('CreateDIBSection', 'pointer', ['pointer', 'pointer', 'uint', 'pointer', 'pointer', 'uint']);
const fSelect = fFn('SelectObject', 'pointer', ['pointer', 'pointer']);
const fStock = fFn('GetStockObject', 'pointer', ['int']);
const fTextOut = fFn('TextOutA', 'bool', ['pointer', 'int', 'int', 'pointer', 'int']);
const fPatBlt = fFn('PatBlt', 'bool', ['pointer', 'int', 'int', 'int', 'int', 'uint']);
const fMetrics = fFn('GetTextMetricsA', 'bool', ['pointer', 'pointer']);
const fFlush = fFn('GdiFlush', 'bool', []);
const fBase = Process.getModuleByName('ski32-autoplay.exe').base;
let fDone = false;
Interceptor.attach(fBase.add(0x1e50), { onEnter() {
  if (fDone) return; fDone = true;
  let text = '';
  for (let c = 0x20; c < 0x7f; c++) text += String.fromCharCode(c);
  const W = 8 * 120, H = 32;
  const dc = fCreateDC(NULL);
  const bmi = Memory.alloc(40);
  bmi.writeU32(40); bmi.add(4).writeS32(W); bmi.add(8).writeS32(-H);
  bmi.add(12).writeU16(1); bmi.add(14).writeU16(32); bmi.add(16).writeU32(0);
  const bits = Memory.alloc(Process.pointerSize);
  const dib = fCreateDIB(dc, bmi, 0, bits, NULL, 0);
  fSelect(dc, dib);
  fSelect(dc, fStock(10));            // OEM_FIXED_FONT
  fPatBlt(dc, 0, 0, W, H, 0x00ff0062);  // WHITENESS
  const str = Memory.allocAnsiString(text);
  fTextOut(dc, 0, 0, str, text.length);
  fFlush();
  const tm = Memory.alloc(64);
  fMetrics(dc, tm);
  send({t: 'font', tick: 0, w: W, h: H, first: 0x20, count: text.length,
        tmHeight: tm.readS32(), tmAscent: tm.add(4).readS32(), tmDescent: tm.add(8).readS32(),
        tmAveCharWidth: tm.add(20).readS32(), tmMaxCharWidth: tm.add(24).readS32(),
        tmCharSet: tm.add(52).readU8()}, bits.readPointer().readByteArray(W * H * 4));
}});
