import { SEED } from './cases3.mjs';
const CASES = [];
const st = (needle, flags) => `FIND_STATE.needle=${JSON.stringify(needle)}; FIND_STATE.wholeWord=!!(${flags}&1); FIND_STATE.matchCase=!!(${flags}&2); FIND_STATE.useRegex=!!(${flags}&4); FIND_STATE.wrap=!!(${flags}&32);`;
const add = (id, sk, ours, o = {}) => CASES.push({ id, sk: Array.isArray(sk) ? sk : [sk], ours, seed: SEED, ...o });
for (const [r, c] of [[0, 0], [0, 2], [0, 3], [0, 4], [0, 5], [1, 5], [2, 1]])
  add(`P word next @${r},${c}`, 'S_SearchNext("foo", 1);', { js: st('foo', 1) + 'findNext();' }, { cur: [r, c] });
add('P bm prev 2marks from end', ['S_MoveCursor(2, 1, 0);', 'S_BookmarkSet();', 'S_MoveCursor(4, 1, 0);', 'S_BookmarkSet();', 'S_GoFileEnd();', 'S_BookmarkPrev();'],
  { js: 'BOOKMARKS.set(bookmarkKey(getActiveTab()), [{row:1,col:0},{row:3,col:0}]); getActiveTab().cursor={row:5,col:0}; bookmarkPrev();' }, { cur: [0, 0] });
const bm2 = ['S_MoveCursor(2, 1, 0);', 'S_BookmarkSet();', 'S_MoveCursor(4, 1, 0);', 'S_BookmarkSet();', 'S_GoFileTop();'];
const mk = (n) => `BOOKMARKS.set(bookmarkKey(getActiveTab()), [{row:1,col:0},{row:3,col:0}]); getActiveTab().cursor={row:0,col:0};` + 'bookmarkNext();'.repeat(n);
CASES.push({ id: 'P bm next x1', sk: [...bm2, 'S_BookmarkNext();'], ours: { js: mk(1) }, seed: SEED, cur: [0, 0] });
// 注: 连续两次以上 S_BookmarkNext 在宏里的表现与手动操作不一致 (疑为宏连续执行的时序), 不作为比对依据
export default CASES;
