// コード変換 (選択範囲の文字列を別エンコードとして解釈し直す)
const CASES = [];
const add = (id, sk, cmd, seed, sel, o = {}) => CASES.push({ id, sk: [sk], ours: { cmd }, seed, sel, ...o });
add('CodeCnv UNICODE→SJIS ascii', 'S_CodeCnvUNICODEtoSJIS();', 'codecnv-unicode2sjis', 'abcd\n', [0, 0, 0, 4]);
add('CodeCnv UNICODE→SJIS ja', 'S_CodeCnvUNICODEtoSJIS();', 'codecnv-unicode2sjis', 'あいう\n', [0, 0, 0, 3]);
add('CodeCnv UNICODEBE→SJIS ascii', 'S_CodeCnvUNICODEBEtoSJIS();', 'codecnv-unicodebe2sjis', 'abcd\n', [0, 0, 0, 4]);
add('CodeCnv UNICODE→SJIS odd', 'S_CodeCnvUNICODEtoSJIS();', 'codecnv-unicode2sjis', 'abc\n', [0, 0, 0, 3]);
export default CASES;
