// 有选区 / 整行模式剪贴板时的粘贴, 有选区时的回车 / 输入
const SEED = 'hello world foo\n  indented line\nthird\n';
const CASES = [];
const add = (id, sk, ours, o = {}) => CASES.push({ id, sk, ours, seed: SEED, ...o });
// 无选区复制 (整行模式) 再粘贴: 光标在行中 / 行首 / 有选区
add('LINEPASTE mid', ['S_Copy();', 'S_Down();', 'S_Paste();'], { keys: ['Control+c', 'ArrowDown', 'Control+v'] }, { cur: [0, 3] });
add('LINEPASTE bol', ['S_Copy();', 'S_Down();', 'S_GoLineTop();', 'S_GoLineTop();', 'S_Paste();'], { keys: ['Control+c', 'ArrowDown', 'Home', 'Home', 'Control+v'] }, { cur: [0, 3] });
add('LINEPASTE over sel', ['S_Copy();', 'S_Down();', 'S_GoLineEnd_Sel();', 'S_Paste();'], { keys: ['Control+c', 'ArrowDown', 'Shift+End', 'Control+v'] }, { cur: [0, 3] });
add('LINECUT then paste', ['S_Cut();', 'S_Down();', 'S_Paste();'], { keys: ['Control+x', 'ArrowDown', 'Control+v'] }, { cur: [0, 3] });
// 选区上回车 / 输入
add('ENTER over sel', ['S_Char(13);'], { keys: ['Enter'] }, { sel: [0, 2, 0, 8], tail: 'marker' });
add('ENTER over multiline sel', ['S_Char(13);'], { keys: ['Enter'] }, { sel: [0, 2, 1, 4], tail: 'marker' });
add('ENTER autoindent', ['S_Char(13);'], { keys: ['Enter'] }, { cur: [1, 8] });
add('TYPE over multiline sel', ['S_Char(81);'], { keys: ['Q'] }, { sel: [0, 2, 1, 4], tail: 'marker' });
export default CASES;
