// make-fixtures.mjs — 生成按字节精确的编码夹具 (内嵌十六进制, 不依赖 iconv / Python).
// 文本: "日本語のテキスト\r\nこんにちは\r\n" (CRLF); cr.txt / lf.txt 是换行不同的 UTF-8 版本。
// 用法: node fixtures/make-fixtures.mjs  (默认写到 fixtures/encodings/, 已提交, 一般不用重新生成)
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const TEXT_LF = '日本語のテキスト\nこんにちは\n';
export const HEX = {
  'sjis.txt': '93fa967b8cea82cc8365834c835883670d0a82b182f182c982bf82cd0d0a',
  'eucjp.txt': 'c6fccbdcb8eca4cea5c6a5ada5b9a5c80d0aa4b3a4f3a4cba4c1a4cf0d0a',
  'jis.txt': '1b2442467c4b5c386c244e2546252d253925481b28420d0a1b244224332473244b2441244f1b28420d0a',
  'u16le.txt': 'fffee5652c679e8a6e30c630ad30b930c8300d000a00533093306b3061306f300d000a00',
  'u16be.txt': 'feff65e5672c8a9e306e30c630ad30b930c8000d000a30533093306b3061306f000d000a',
  'u8bom.txt': 'efbbbfe697a5e69cace8aa9ee381aee38386e382ade382b9e383880d0ae38193e38293e381abe381a1e381af0d0a',
  'u8.txt': 'e697a5e69cace8aa9ee381aee38386e382ade382b9e383880d0ae38193e38293e381abe381a1e381af0d0a',
  'cr.txt': 'e697a5e69cace8aa9ee381aee38386e382ade382b9e383880de38193e38293e381abe381a1e381af0d',
  'lf.txt': 'e697a5e69cace8aa9ee381aee38386e382ade382b9e383880ae38193e38293e381abe381a1e381af0a',
};

const out = path.join(path.dirname(fileURLToPath(import.meta.url)), 'encodings');
if (import.meta.url === 'file:///' + process.argv[1].replace(/\\/g, '/')) {
  fs.mkdirSync(out, { recursive: true });
  for (const [name, hex] of Object.entries(HEX)) fs.writeFileSync(path.join(out, name), Buffer.from(hex, 'hex'));
  console.log('wrote', Object.keys(HEX).length, 'fixtures to', out);
}
