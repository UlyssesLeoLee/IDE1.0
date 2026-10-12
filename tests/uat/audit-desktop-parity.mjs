// tests/uat/audit-desktop-parity.mjs
// 桌面版 (Tauri) 与网页版前端一致性审计.
//
// 背景: desktop/dist/index.html 曾长期落后于 web 版 editor.html, 缺 15 项能力
// (语法高亮/多重光标/列块/补全/行操作/剪贴板历史/编码转换/时间戳/查找替换/
// 书签/宏/分屏/缩略图/折叠/Extensions). 现已改为 build.rs 构建时同步, 本脚本
// 防止再次分叉 — 在 CI 与本地都能跑.
//
// 用法: node tests/uat/audit-desktop-parity.mjs
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const repo = join(here, '..', '..');
const webPath = join(repo, 'crates', 'ide-shell-web', 'src', 'editor.html');
const deskPath = join(repo, 'crates', 'ide-shell-desktop', 'dist', 'index.html');

let failures = 0;
const fail = (msg) => { failures++; console.error('  ✗ ' + msg); };
const ok = (msg) => console.log('  ✓ ' + msg);

console.log('桌面版 / 网页版 前端一致性审计\n');

let web, desk;
try {
  web = readFileSync(webPath, 'utf8');
  desk = readFileSync(deskPath, 'utf8');
} catch (e) {
  console.error('读文件失败: ' + e.message);
  process.exit(1);
}

// 1) 字节级一致
if (web === desk) ok(`内容一致 (${web.length} bytes)`);
else fail(`内容不一致: web ${web.length}B vs desktop ${desk.length}B — 跑 cargo build -p ide-shell-desktop 让 build.rs 同步`);

// 2) 关键能力都在 (即使字节不一致也能独立报缺失)
const CAPS = {
  '语法高亮': 'Syntax.highlightLine',
  '高亮渲染': 'function renderVim',
  '多重光标': 'addCursor',
  '列块选择': 'startColumnSelect',
  '自动补全': 'completionCandidates',
  '行移动': 'function moveLines',
  '剪贴板历史': 'CLIP_HISTORY',
  '编码转换': 'ENCODINGS',
  '时间戳': 'stampNow',
  '查找替换': 'findInTab',
  '书签': 'BOOKMARKS',
  '宏录制': 'MACRO.recording',
  '分屏': 'SPLIT.mode',
  '缩略图': 'buildMinimap',
  '折叠': 'foldAtCursor',
  'Extensions 面板': 'data-pane="extensions"',
  'Tauri 命令名映射': 'TAURI_CMD_ALIAS',
  'Tauri 终端事件通道': 'setupTerminalChannel',
};
console.log('\n能力覆盖 (desktop):');
for (const [name, pat] of Object.entries(CAPS)) {
  if (desk.includes(pat)) ok(name);
  else fail(`${name} 缺失 (${pat})`);
}

// 3) dispatchCommand 的 case 数 — 防止又出现「菜单声明了但没 case」
const cases = (desk.match(/case "/g) || []).length;
console.log('\n命令路由:');
if (cases >= 110) ok(`dispatchCommand ${cases} 个 case`);
else fail(`dispatchCommand 只有 ${cases} 个 case (期望 >= 110)`);

// 4) 占位提示语
const stubs = [...desk.matchAll(/setTip\("([^"]*(?:未实装|待实装|未接入)[^"]*)"\)/g)].map((m) => m[1]);
if (stubs.length === 0) ok('无占位提示');
else { fail(`${stubs.length} 处占位:`); stubs.forEach((s) => console.error('      ' + s)); }

// 5) Tauri 侧必须注册的 command 与前端 invoke 对齐
const libPath = join(repo, 'crates', 'ide-shell-desktop', 'src', 'lib.rs');
const lib = readFileSync(libPath, 'utf8');
const regBlock = lib.match(/generate_handler!\[([^\]]*)\]/s);
console.log('\nTauri command 注册:');
if (!regBlock) fail('找不到 generate_handler!');
else {
  const registered = new Set(regBlock[1].split(',')
    .map((s) => s.trim()).filter(Boolean)
    .map((s) => s.split('::').pop()));
  const invoked = [...web.matchAll(/invoke\("([^"]+)"/g)].map((m) => m[1]);
  const aliasBlock = web.match(/const TAURI_CMD_ALIAS = \{([^}]*)\}/s);
  const aliases = new Set(aliasBlock
    ? [...aliasBlock[1].matchAll(/(\w+):/g)].map((m) => m[1]) : []);
  // Tauri 侧"可选"命令: 前端有 try/catch 降级, 缺了也不致命.
  // terminal_output 只在事件通道不可用时才调 (TERM_CHANNEL !== 'event'),
  // 正常路径走 terminal://output 事件, 所以未注册可以接受.
  const OPTIONAL = new Set(['terminal_output']);
  for (const name of invoked) {
    if (aliases.has(name) || registered.has(name)) { ok(name); continue; }
    if (OPTIONAL.has(name)) {
      console.log(`  ~ ${name} (可选, 事件通道可用时不调用)`);
      continue;
    }
    fail(`${name} — web 前端 invoke 了, 但 Tauri 未注册`);
  }
}

console.log('\n' + (failures === 0
  ? '全部通过 — 桌面版与网页版功能一致'
  : `${failures} 项不一致`));
process.exit(failures === 0 ? 0 : 1);