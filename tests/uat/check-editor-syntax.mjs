// 语法守卫: 把 editor.html 里最大的内联 <script> 拿出来过一遍 JS 解析。
// 起因: 合入别人的改动时多出一个 `}` , 整页脚本加载失败, 所有页面测试都挂 —— 这个脚本几秒钟就能指出来。
// 用法: node tests/uat/check-editor-syntax.mjs   (退出码 0 = 语法正确)
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const files = ['crates/ide-shell-web/src/editor.html', 'crates/ide-shell-desktop/dist/index.html'];
let bad = 0;
for (const f of files) {
  const html = fs.readFileSync(path.join(root, f), 'utf8');
  const scripts = [...html.matchAll(/<script(?![^>]*\bsrc\b)[^>]*>([\s\S]*?)<\/script>/g)].map((m) => m[1]);
  const main = scripts.sort((a, b) => b.length - a.length)[0] || '';
  try {
    new Function(main); // 只解析不执行
    console.log(`OK   ${f} (${main.length} 字符)`);
  } catch (e) {
    bad++;
    console.error(`FAIL ${f}: ${e.message}`);
  }
}
process.exit(bad ? 1 : 0);
