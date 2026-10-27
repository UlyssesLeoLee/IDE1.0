import { runSakura } from './sakura.mjs';
const seed = 'hello world foo\n  indented line\n\nthird line here\n';
const v = {
  A_flag1: ['S_GoFileTop();', 'S_MoveCursor(1, 3, 0);', 'S_MoveCursor(2, 4, 1);', 'S_Copy();', 'S_GoFileEnd();', 'S_Paste();'],
  B_begin: ['S_GoFileTop();', 'S_MoveCursor(1, 3, 0);', 'S_BeginSelect();', 'S_MoveCursor(2, 4, 0);', 'S_Copy();', 'S_GoFileEnd();', 'S_Paste();'],
  C_begin_flag1: ['S_GoFileTop();', 'S_MoveCursor(1, 3, 0);', 'S_BeginSelect();', 'S_MoveCursor(2, 4, 1);', 'S_Copy();', 'S_GoFileEnd();', 'S_Paste();'],
  D_sel_moves: ['S_GoFileTop();', 'S_MoveCursor(1, 3, 0);', 'S_BeginSelect();', 'S_Down();', 'S_Right();', 'S_Copy();', 'S_GoFileEnd();', 'S_Paste();'],
};
for (const [k, m] of Object.entries(v)) {
  const r = await runSakura(seed, m);
  console.log(k, r.finished, JSON.stringify(r.text.slice(seed.length)));
}
