// 矩形选择: Sakura = S_BeginBoxSelect + S_Down_BoxSel / S_Right_BoxSel; 我们 = t.colSel
const SEED = 'abcdefghij\n0123456789\nxy\nABCDEFGHIJ\nlast line\n';
const CASES = [];
const box = (r, c, dr, dc) => [`S_MoveCursor(${r + 1}, ${c + 1}, 0);`, 'S_BeginBoxSelect();', ...Array(dr).fill('S_Down_BoxSel();'), ...Array(Math.abs(dc)).fill(dc < 0 ? 'S_Left_BoxSel();' : 'S_Right_BoxSel();')];
const pre = (r, c, dr, dc) => `const t=getActiveTab(); t.sel=null; t.selAnchor=null; t.cursor={row:${r+dr},col:${c+dc}}; t.colSel={r0:${r},c0:${c},r1:${r+dr},c1:${c+dc}}; t.colActiveRow=${r+dr}; t.colActiveCol=${c+dc}; renderVim(t);`;
const add = (id, r, c, dr, dc, op, skop, o = {}) => CASES.push({ id, seed: SEED, cur: [r, c], sk: [...box(r, c, dr, dc), ...skop], ours: { pre: pre(r, c, dr, dc), ...op }, ...o });
add('BOX copy 3x3', 0, 2, 2, 3, { key: 'F8' }, ['S_Copy();'], { tail: 'paste' });
add('BOX copy over short line', 1, 4, 2, 3, { key: 'F8' }, ['S_Copy();'], { tail: 'paste' });
add('BOX cut', 0, 2, 1, 3, { key: 'F7' }, ['S_Cut();'], { tail: 'paste' });
add('BOX delete', 0, 2, 1, 3, { key: 'Delete' }, ['S_Delete();']);
add('BOX delete over short line', 1, 4, 2, 3, { key: 'Delete' }, ['S_Delete();']);
add('BOX backspace', 0, 2, 1, 3, { key: 'Backspace' }, ['S_DeleteBack();']);
add('BOX zero-width down', 0, 2, 2, 0, { key: 'F8' }, ['S_Copy();'], { tail: 'paste' });
add('BOX leftward', 0, 5, 1, -2, { key: 'F8' }, ['S_Copy();'], { tail: 'paste' });
export default CASES;
