// 驱动真实的 Sakura Editor (SAKURA_EXE, 默认 C:/Program Files (x86)/sakura/sakura.exe) 运行键盘宏, 取回保存后的文件内容。
import { spawn, execSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

export const SAKURA_EXE = process.env.SAKURA_EXE || 'C:/Program Files (x86)/sakura/sakura.exe';
export const hasSakura = fs.existsSync(SAKURA_EXE);
const WORK = fs.mkdtempSync(path.join(os.tmpdir(), 'skdiff-')).replace(/\\/g, '/');

function procAlive() {
  try { return execSync('tasklist /FI "IMAGENAME eq sakura.exe" /NH', { encoding: 'latin1' }).toLowerCase().includes('sakura.exe'); } catch (_) { return false; }
}

// macroLines: Sakura 键盘宏行 (S_Xxx(0);)。固定在末尾保存并退出; 宏出错 (函数名不对等) 时 Sakura 会弹框不退出 → finished=false。
export async function runSakura(seed, macroLines, { timeoutMs = 12000 } = {}) {
  const file = WORK + '/work.txt', mac = WORK + '/m.mac';
  fs.writeFileSync(file, seed.replace(/\r?\n/g, '\r\n'));
  fs.writeFileSync(mac, [...macroLines, 'S_FileSave(0);', 'S_ExitAll(0);'].join('\r\n') + '\r\n');
  const p = spawn(SAKURA_EXE, ['-M=' + mac, file], { stdio: 'ignore' });
  let exited = false;
  p.on('exit', () => { exited = true; });
  const t0 = Date.now();
  let gone = false;
  while (Date.now() - t0 < timeoutMs) {
    await new Promise((r) => setTimeout(r, 200));
    if (Date.now() - t0 > 1500 && !procAlive()) { gone = true; break; }
  }
  try { execSync('taskkill /F /IM sakura.exe', { stdio: 'ignore' }); } catch (_) {}
  void exited;
  const out = fs.readFileSync(file, 'utf8').replace(/\r\n/g, '\n');
  return { text: out, finished: gone };
}
