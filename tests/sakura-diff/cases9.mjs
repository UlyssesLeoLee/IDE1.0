// 日文字符种类 (漢字 / ひらがな / カタカナ / 英数 / 記号 / 空白) 上的单词移动与选词
const SEED = '日本語のテキストabc def、漢字123かな\nカタカナＡＢＣ全角　スペース\n';
const CASES = [];
const add = (id, sk, ours, o = {}) => CASES.push({ id, sk: [sk], ours, seed: SEED, ...o });
for (const c of [0, 1, 3, 4, 8, 11, 12, 15, 16, 18, 21]) add(`JA WORDRIGHT @0,${c}`, 'S_WordRight();', { key: 'Control+ArrowRight' }, { cur: [0, c] });
for (const c of [3, 4, 8, 11, 16, 21, 23]) add(`JA WORDLEFT @0,${c}`, 'S_WordLeft();', { key: 'Control+ArrowLeft' }, { cur: [0, c] });
for (const c of [1, 3, 5, 9, 13, 18, 22]) add(`JA SELECTWORD @0,${c}`, 'S_SelectWord();', { key: 'Control+w' }, { cur: [0, c], tail: 'selcopy' });
for (const c of [0, 4, 7, 10]) add(`JA SELECTWORD @1,${c}`, 'S_SelectWord();', { key: 'Control+w' }, { cur: [1, c], tail: 'selcopy' });
for (const c of [1, 5]) add(`JA WORDDELETE @0,${c}`, 'S_WordDelete();', { key: 'Control+Shift+d' }, { cur: [0, c] });
export default CASES;
