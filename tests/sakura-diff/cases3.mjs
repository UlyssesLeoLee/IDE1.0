// 第三批: 検索 / 置換 / 対括弧 / ブックマーク / 指定行ジャンプ。 flags 位: 1=单词 2=区分大小写 4=正则 32=先頭(末尾)から再検索 128=選択範囲
export const SEED = 'foo Foo foobar\nbar foo FOO\nfoo.bar foo(1)\nx (a [b] {c}) y\nlast foo\n';
const CASES = [];
const add = (id, sk, ours, o = {}) => CASES.push({ id, sk: Array.isArray(sk) ? sk : [sk], ours, seed: SEED, ...o });
// 我们这边直接设置 FIND_STATE 后调用查找 / 替换 (flags 与 Sakura 同位)
const st = (needle, flags, repl) => `FIND_STATE.needle=${JSON.stringify(needle)}; FIND_STATE.wholeWord=!!(${flags}&1); FIND_STATE.matchCase=!!(${flags}&2); FIND_STATE.useRegex=!!(${flags}&4); FIND_STATE.wrap=!!(${flags}&32);` + (repl === undefined ? '' : `FIND_STATE.scope=(${flags}&128)?'sel':'all'; { let el=document.getElementById('repl-input'); if(!el){ el=document.createElement('input'); el.id='repl-input'; el.style.display='none'; document.body.appendChild(el); } el.value=${JSON.stringify(repl)}; }`);

add('SearchNext foo (ignore case)', 'S_SearchNext("foo", 0);', { js: st('foo', 0) + 'findNext();' }, { cur: [0, 0] });
add('SearchNext foo from mid', 'S_SearchNext("foo", 0);', { js: st('foo', 0) + 'findNext();' }, { cur: [0, 1] });
add('SearchNext case-sensitive Foo', 'S_SearchNext("Foo", 2);', { js: st('Foo', 2) + 'findNext();' }, { cur: [0, 5] });
add('SearchNext word', 'S_SearchNext("foo", 1);', { js: st('foo', 1) + 'findNext();' }, { cur: [0, 1] });
add('SearchNext regex', 'S_SearchNext("fo+\\w*", 4);', { js: st('fo+\\w*', 4) + 'findNext();' }, { cur: [0, 1] });
add('SearchNext no-wrap at end', 'S_SearchNext("foo", 0);', { js: st('foo', 0) + 'findNext();' }, { cur: [4, 6] });
add('SearchNext wrap at end', 'S_SearchNext("foo", 32);', { js: st('foo', 32) + 'findNext();' }, { cur: [4, 6] });
add('SearchPrev foo', 'S_SearchPrev("foo", 0);', { js: st('foo', 0) + 'findPrev();' }, { cur: [4, 7] });
add('SearchPrev case', 'S_SearchPrev("foo", 2);', { js: st('foo', 2) + 'findPrev();' }, { cur: [2, 4] });
add('SearchPrev wrap', 'S_SearchPrev("foo", 32);', { js: st('foo', 32) + 'findPrev();' }, { cur: [0, 0] });
add('SearchNext skip current sel', ['S_SearchNext("foo", 0);', 'S_SearchNext("foo", 0);'], { js: st('foo', 0) + 'findNext(); findNext();' }, { cur: [0, 0] });
add('ReplaceAll foo→X', 'S_ReplaceAll("foo", "X", 0);', { js: st('foo', 0, 'X') + 'replaceAll();' }, { cur: [0, 0] });
add('ReplaceAll case', 'S_ReplaceAll("foo", "X", 2);', { js: st('foo', 2, 'X') + 'replaceAll();' }, { cur: [0, 0] });
add('ReplaceAll word', 'S_ReplaceAll("foo", "X", 1);', { js: st('foo', 1, 'X') + 'replaceAll();' }, { cur: [0, 0] });
add('ReplaceAll regex groups', 'S_ReplaceAll("(f)(o+)", "[$2$1]", 4);', { js: st('(f)(o+)', 4, '[$2$1]') + 'replaceAll();' }, { cur: [0, 0] });
add('ReplaceAll regex anchors', 'S_ReplaceAll("^foo", "", 4);', { js: st('^foo', 4, '') + 'replaceAll();' }, { cur: [0, 0] });
add('ReplaceAll in selection', 'S_ReplaceAll("foo", "X", 128);', { js: st('foo', 128, 'X') + 'replaceAll();' }, { sel: [1, 0, 2, 3] });
add('ReplaceAll longer', 'S_ReplaceAll("foo", "foofoo", 0);', { js: st('foo', 0, 'foofoo') + 'replaceAll();' }, { cur: [0, 0] });
add('BracketPair (', 'S_BracketPair();', { js: "dispatchCommand('bracket-pair')" }, { cur: [3, 2] });
add('BracketPair ) right-of-cursor', 'S_BracketPair();', { js: "dispatchCommand('bracket-pair')" }, { cur: [3, 10] });
add('BracketPair [', 'S_BracketPair();', { js: "dispatchCommand('bracket-pair')" }, { cur: [3, 6] });
add('BracketPair none', 'S_BracketPair();', { js: "dispatchCommand('bracket-pair')" }, { cur: [0, 1] });
add('BookmarkSet+Next', ['S_MoveCursor(3, 1, 0);', 'S_BookmarkSet();', 'S_GoFileTop();', 'S_BookmarkNext();'], { js: 'BOOKMARKS.set(bookmarkKey(getActiveTab()), [{row:2,col:0}]); getActiveTab().cursor={row:0,col:0}; bookmarkNext();' }, { cur: [0, 0] });
add('Jump line 3', 'S_Jump(3, 1);', { js: "window.prompt=()=>'3'; openGotoLine()" }, { cur: [0, 0] });
add('Jump line beyond', 'S_Jump(99, 1);', { js: "window.prompt=()=>'99'; openGotoLine()" }, { cur: [0, 0] });
export default CASES;
