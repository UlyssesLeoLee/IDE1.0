// 逐条行为比对用例: 同一份种子文本 + 同一个起点 (光标 / 选区), 分别交给真实 Sakura (键盘宏) 和我们的编辑器执行,
// 然后在光标处插入标记 "|" 并比较结果文本。 (种子文本为 LF; Sakura 侧写成 CRLF 再读回归一)
//
// 用例: { id, sk: Sakura 宏行 (S_Xxx();), ours: { key | cmd }, seed?, cur?: [row, col], sel?: [r0, c0, r1, c1], clip?: 'x', tail?: 'paste' }
//   tail: 'paste' = 结尾再把剪贴板粘贴到文件末尾 (用于比较复制类命令), 默认在光标处插入标记 |
export const SEED = 'hello world foo\n  indented line\n\nthird line here\nlast Line END\n';
const CASES = [];
const add = (id, sk, ours, o = {}) => CASES.push({ id, sk: Array.isArray(sk) ? sk : [sk], ours, ...o });

// ---- 单词 / 行 删除·剪切 ----
add('F_WordCut', 'S_WordCut();', { key: 'Control+d' }, { cur: [0, 2] });
add('F_WordDelete', 'S_WordDelete();', { key: 'Control+Shift+d' }, { cur: [0, 2] });
add('F_WordDeleteToStart', 'S_WordDeleteToStart();', { key: 'Control+Backspace' }, { cur: [0, 8] });
add('F_WordDeleteToEnd', 'S_WordDeleteToEnd();', { key: 'Control+Delete' }, { cur: [0, 8] });
add('F_LineCutToStart', 'S_LineCutToStart();', { key: 'Control+u' }, { cur: [0, 5] });
add('F_LineCutToEnd', 'S_LineCutToEnd();', { key: 'Control+k' }, { cur: [0, 5] });
add('F_LineDeleteToStart', 'S_LineDeleteToStart();', { key: 'Control+Shift+u' }, { cur: [0, 5] });
add('F_LineDeleteToEnd', 'S_LineDeleteToEnd();', { key: 'Control+Shift+k' }, { cur: [0, 5] });
add('F_CUT_LINE', 'S_CutLine();', { key: 'Control+e' }, { cur: [1, 4] });
add('F_DELETE_LINE', 'S_DeleteLine();', { key: 'Control+Shift+e' }, { cur: [1, 4] });
add('F_DUPLICATELINE', 'S_DuplicateLine();', { key: 'Control+i' }, { cur: [1, 4] });
add('F_DELETE', 'S_Delete();', { key: 'Delete' }, { cur: [0, 3] });
add('F_DELETE_BACK', 'S_DeleteBack();', { key: 'Backspace' }, { cur: [0, 3] });
add('F_DELETE(sel)', 'S_Delete();', { key: 'Delete' }, { sel: [0, 2, 0, 8] });
add('F_SELECTWORD', 'S_SelectWord();', { key: 'Control+w' }, { cur: [0, 8] });
// ---- インデント ----
add('F_INDENT_TAB', 'S_IndentTab();', { key: 'Tab' }, { sel: [0, 2, 1, 3] });
add('F_UNINDENT_TAB', 'S_UnindentTab();', { key: 'Shift+Tab' }, { seed: '\ta\n\tb\nc\n', sel: [0, 0, 1, 1] });
add('F_INDENT_SPACE', 'S_IndentSpace();', { cmd: 'indent-space' }, { sel: [0, 2, 1, 3] });
add('F_UNINDENT_SPACE', 'S_UnindentSpace();', { cmd: 'unindent-space' }, { seed: '    a\n    b\nc\n', sel: [0, 0, 1, 1] });
// ---- 変換 ----
add('F_LTRIM', 'S_LTrim();', { key: 'Alt+l' }, { seed: '  a\n   b\nc\n', sel: [0, 0, 2, 1] });
add('F_RTRIM', 'S_RTrim();', { key: 'Alt+r' }, { seed: 'a  \nb   \nc\n', sel: [0, 0, 2, 1] });
add('F_SORT_ASC', 'S_SortAsc();', { key: 'Alt+a' }, { seed: 'c\nb\na\nB\n', sel: [0, 0, 3, 1] });
add('F_SORT_DESC', 'S_SortDesc();', { key: 'Alt+d' }, { seed: 'a\nb\nc\nB\n', sel: [0, 0, 3, 1] });
add('F_MERGE', 'S_Merge();', { key: 'Alt+m' }, { seed: 'a\na\nb\nb\nb\nc\n', sel: [0, 0, 5, 1] });
add('F_TOLOWER', 'S_ToLower();', { key: 'Control+F6' }, { seed: 'ABC Def\n', sel: [0, 0, 0, 7] });
add('F_TOUPPER', 'S_ToUpper();', { key: 'Control+F7' }, { seed: 'abc Def\n', sel: [0, 0, 0, 7] });
add('F_TOHANKAKU', 'S_ToHankaku();', { key: 'Control+F8' }, { seed: 'ＡＢＣ１２３　あ\n', sel: [0, 0, 0, 8] });
add('F_TOZENKAKUKATA', 'S_ToZenKata();', { key: 'Control+F9' }, { seed: 'abc ｶﾀｶﾅ あいう\n', sel: [0, 0, 0, 12] });
add('F_TOZENKAKUHIRA', 'S_ToZenHira();', { key: 'Control+F10' }, { seed: 'abc ｶﾀｶﾅ カタカナ\n', sel: [0, 0, 0, 13] });
add('F_TOHANEI', 'S_ToHanEi();', { cmd: 'to-hanei' }, { seed: 'ＡＢＣ１２３あ\n', sel: [0, 0, 0, 7] });
add('F_TOZENEI', 'S_ToZenEi();', { cmd: 'to-zenei' }, { seed: 'ABC123あ\n', sel: [0, 0, 0, 7] });
add('F_TOHANKATA', 'S_ToHankata();', { cmd: 'to-hankata' }, { seed: 'カタカナガパ\n', sel: [0, 0, 0, 6] });
add('F_HANKATATOZENKATA', 'S_HanKataToZenKata();', { key: 'Control+F11' }, { seed: 'ｶﾀｶﾅｶﾞﾊﾟ\n', sel: [0, 0, 0, 8] });
add('F_HANKATATOZENHIRA', 'S_HanKataToZenHira();', { key: 'Control+F12' }, { seed: 'ｶﾀｶﾅｶﾞﾊﾟ\n', sel: [0, 0, 0, 8] });
add('F_TABTOSPACE', 'S_TABToSPACE();', { key: 'Control+Alt+F5' }, { seed: '\ta\tb\n', sel: [0, 0, 0, 4] });
add('F_SPACETOTAB', 'S_SPACEToTAB();', { key: 'Control+Shift+Alt+F5' }, { seed: '        a\n', sel: [0, 0, 0, 9] });
// ---- カーソル移動 (標記 | の位置で比較) ----
add('F_GOLINETOP', 'S_GoLineTop();', { key: 'Home' }, { cur: [1, 6] });
add('F_GOLINEEND', 'S_GoLineEnd();', { key: 'End' }, { cur: [1, 2] });
add('F_GOFILETOP', 'S_GoFileTop();', { key: 'Control+Home' }, { cur: [3, 4] });
add('F_GOFILEEND', 'S_GoFileEnd();', { key: 'Control+End' }, { cur: [1, 4] });
add('F_WORDRIGHT', 'S_WordRight();', { key: 'Control+ArrowRight' }, { cur: [0, 1] });
add('F_WORDLEFT', 'S_WordLeft();', { key: 'Control+ArrowLeft' }, { cur: [0, 8] });
add('F_GONEXTPARAGRAPH', 'S_GoNextParagraph();', { cmd: 'cursor-para-next' }, { cur: [0, 3] });
add('F_GOPREVPARAGRAPH', 'S_GoPrevParagraph();', { cmd: 'cursor-para-prev' }, { cur: [4, 3] });
add('F_UP2', 'S_Up2();', { cmd: 'cursor-up-2' }, { cur: [4, 3] });
add('F_DOWN2', 'S_Down2();', { cmd: 'cursor-down-2' }, { cur: [0, 3] });
add('F_LEFT@bol', 'S_Left();', { key: 'ArrowLeft' }, { cur: [1, 0] });
add('F_RIGHT@eol', 'S_Right();', { key: 'ArrowRight' }, { cur: [0, 15] });
add('F_UP(goal col)', ['S_Up();'], { key: 'ArrowUp' }, { cur: [3, 10] });
add('F_DOWN(goal col)', ['S_Down();'], { key: 'ArrowDown' }, { cur: [0, 12] });
// ---- 範囲選択 ----
add('F_RIGHT_SEL', 'S_Right_Sel();', { key: 'Shift+ArrowRight' }, { cur: [0, 3] });
add('F_DOWN_SEL', 'S_Down_Sel();', { key: 'Shift+ArrowDown' }, { cur: [0, 3] });
add('F_WORDRIGHT_SEL', 'S_WordRight_Sel();', { key: 'Control+Shift+ArrowRight' }, { cur: [0, 1] });
add('F_GOLINEEND_SEL', 'S_GoLineEnd_Sel();', { key: 'Shift+End' }, { cur: [0, 3] });
add('F_GOFILEEND_SEL', 'S_GoFileEnd_Sel();', { key: 'Control+Shift+End' }, { cur: [3, 3] });
add('F_SELECTALL', 'S_SelectAll();', { key: 'Control+a' }, { cur: [1, 3] });
// ---- コピー系 (クリップボードを文末に貼り付けて比較) ----
add('F_COPY', 'S_Copy();', { key: 'F8' }, { sel: [0, 2, 0, 8], tail: 'paste' });
add('F_CUT', 'S_Cut();', { key: 'F7' }, { sel: [0, 2, 0, 8], tail: 'paste' });
add('F_COPYLINES', 'S_CopyLines();', { cmd: 'copy-lines' }, { sel: [0, 2, 1, 3], tail: 'paste' });
add('F_COPYLINESASPASSAGE', 'S_CopyLinesAsPassage();', { key: 'Control+.' }, { sel: [0, 2, 1, 3], tail: 'paste' });
add('F_COPYLINESWITHLINENUMBER', 'S_CopyLinesWithLineNumber();', { cmd: 'copy-linenum' }, { sel: [0, 2, 1, 3], tail: 'paste' });
add('F_COPY_CRLF', 'S_CopyCRLF();', { key: 'Shift+F8' }, { sel: [0, 2, 1, 3], tail: 'paste' });
add('F_PASTE', 'S_Paste();', { key: 'F9' }, { cur: [0, 3], clip: 'XY' });
add('F_PASTE(sel)', 'S_Paste();', { key: 'F9' }, { sel: [0, 2, 0, 5], clip: 'XY' });
add('F_PASTEBOX', 'S_PasteBox();', { key: 'Shift+F9' }, { cur: [0, 3], clip: 'ab\ncd' });

export default CASES;
