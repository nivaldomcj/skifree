// Auto-dismiss the original's modal message boxes so long runs don't block.
// Each `call [MessageBoxA]` (6 bytes) in the game module is patched to
// `push 1; pop eax; add esp, 0x10`: returns IDOK and pops the four stdcall
// arguments, like a box the user closed at once. Sites: FUN_00401270 (assert
// / error), FUN_00402ec0 (high-score message on course finish), FUN_00404950.
// The high-score code still reads/writes entpack.ini in the VM; nothing it
// does feeds back into the game state.
const bD = Process.getModuleByName('ski32-autoplay.exe').base;
for (const site of [0x1276, 0x3118, 0x4963]) {
  const p = bD.add(site);
  Memory.patchCode(p, 6, code => {
    code.writeByteArray([0x6a, 0x01, 0x58, 0x83, 0xc4, 0x10]);
  });
}
