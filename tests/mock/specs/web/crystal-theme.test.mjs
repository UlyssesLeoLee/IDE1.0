// 回归: 水晶透明主题 (crystal) 与主题插件机 (更新: 新架构用 data-crystal-lum 属性替代 mix-blend-mode: difference)制
// 1. 水晶主题具备更高透光度、钻石切面棱纹 (app::before) 与字符级动态反色
//    新架构: JS 采样背景亮度 → 写 data-crystal-lum="light"|"dark" → CSS 切黑/白文字
//    (不再使用 mix-blend-mode: difference, 该方案无法穿透 OS 层合成背景)
// 2. 主题插件支持独立增删、停用/启用、恢复内置与 .zip 插件包安装
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { withMocks } from '../../lib/with-mocks.mjs';

const PLUG_DIR = path.resolve(import.meta.dirname, '../../../..', 'crates/editcore/plugins');

function zipOf(files) {
  const enc = new TextEncoder(); const parts = []; const cd = []; let off = 0;
  const crc = (b) => { let c, t = []; for (let n = 0; n < 256; n++) { c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; t[n] = c >>> 0; } let r = 0xffffffff; for (const x of b) r = t[(r ^ x) & 0xff] ^ (r >>> 8); return (r ^ 0xffffffff) >>> 0; };
  for (const [name, data] of files) {
    const nb = enc.encode(name), cs = crc(data), h = Buffer.alloc(30);
    h.writeUInt32LE(0x04034b50, 0); h.writeUInt16LE(20, 4); h.writeUInt32LE(cs, 14); h.writeUInt32LE(data.length, 18); h.writeUInt32LE(data.length, 22); h.writeUInt16LE(nb.length, 26);
    parts.push(h, Buffer.from(nb), Buffer.from(data));
    const c = Buffer.alloc(46); c.writeUInt32LE(0x02014b50, 0); c.writeUInt16LE(20, 4); c.writeUInt16LE(20, 6); c.writeUInt32LE(cs, 16); c.writeUInt32LE(data.length, 20); c.writeUInt32LE(data.length, 24); c.writeUInt16LE(nb.length, 28); c.writeUInt32LE(off, 42);
    cd.push(c, Buffer.from(nb)); off += 30 + nb.length + data.length;
  }
  const cdb = Buffer.concat(cd), e = Buffer.alloc(22);
  e.writeUInt32LE(0x06054b50, 0); e.writeUInt16LE(files.length, 8); e.writeUInt16LE(files.length, 10); e.writeUInt32LE(cdb.length, 12); e.writeUInt32LE(off, 16);
  return Buffer.concat([...parts, cdb, e]);
}

test('水晶主题: 通透度、钻石切面棱纹与字符级动态反色 (data-crystal-lum 属性架构)', () =>
  withMocks({ name: 'cryst', portOffset: 652 }, async ({ page }) => {
    // 切换到水晶透明主题
    await page.evaluate(() => setTheme('crystal'));
    await page.waitForTimeout(200);

    const check = await page.evaluate(() => {
      const root = document.documentElement;
      const theme = root.getAttribute('data-theme');
      const bodyBg = getComputedStyle(document.body).backgroundColor;
      const ed = document.getElementById('ed-content');
      const edStyle = getComputedStyle(ed);
      const edBg = edStyle.backgroundColor;

      // 新架构: 文字反色靠 data-crystal-lum 属性 + CSS 选择器, 不再靠 mix-blend-mode: difference
      const crystalLum = root.getAttribute('data-crystal-lum'); // 应为 "light" 或 "dark"

      // 检查钻石切面伪元素棱纹
      const appBefore = window.getComputedStyle(document.getElementById('app'), '::before');
      const hasFacetImg = appBefore.backgroundImage && appBefore.backgroundImage.includes('linear-gradient');

      // 检查 data-crystal-lum 是否存在 CSS 规则 (通过查找 stylesheet 里的选择器)
      let hasCrystalLumRule = false;
      try {
        for (const ss of document.styleSheets) {
          for (const rule of ss.cssRules || []) {
            if (rule.selectorText && rule.selectorText.includes('data-crystal-lum')) {
              hasCrystalLumRule = true; break;
            }
          }
          if (hasCrystalLumRule) break;
        }
      } catch (_) {}

      return {
        theme,
        bodyBg,
        edBg,
        hasFacetImg: !!hasFacetImg,
        crystalLumSet: crystalLum === 'light' || crystalLum === 'dark',
        hasCrystalLumRule,
        color: edStyle.color,
      };
    });

    assert.equal(check.theme, 'crystal', 'data-theme 已设为 crystal');
    assert.match(check.bodyBg, /rgba\(0,\s*0,\s*0,\s*0\)|transparent/, '背景完全透明');
    assert.match(check.edBg, /rgba\(0,\s*0,\s*0,\s*0\)|transparent/, '编辑区背景全透');
    assert.ok(check.crystalLumSet, 'data-crystal-lum 属性已由 JS 写入 (light 或 dark)');
    assert.ok(check.hasCrystalLumRule, 'CSS 中存在 [data-crystal-lum] 选择器规则');
    assert.ok(check.hasFacetImg, '钻石切面多角度渐变棱纹存在于伪元素层');
  }));

test('主题插件体系: 内置 theme-retro16 随程序打包 -> 可删除 -> 主题列表联动移除 -> 用插件包恢复', () =>
  withMocks({ name: 'thplug', portOffset: 653 }, async ({ page }) => {
    await page.locator('#ed-content').click();

    // 检查初始状态: theme-retro16 内置且启用
    const initial = await page.evaluate(async () => {
      await plugRefresh();
      return {
        plugins: PLUG.list.map((p) => p.manifest.id),
        hasLabel: !!THEME_LABELS.retro16,
      };
    });
    assert.ok(initial.plugins.includes('theme-retro16'), '内置 theme-retro16 存在');
    assert.ok(initial.hasLabel, 'THEME_LABELS 包含 retro16');

    // 删除 theme-retro16
    await page.evaluate(async () => {
      await plugApi({ op: 'remove', id: 'theme-retro16' });
      await plugRefresh();
    });

    const afterRemove = await page.evaluate(() => ({
      plugins: PLUG.list.map((p) => p.manifest.id),
      removed: PLUG.removed.map((x) => x.id),
    }));
    assert.ok(!afterRemove.plugins.includes('theme-retro16'), '已从启用列表移除');
    assert.ok(afterRemove.removed.includes('theme-retro16'), '记录在 removed 清单');

    // 打包 theme-retro16 为 zip 并重新安装
    const zip = zipOf([
      ['manifest.json', fs.readFileSync(path.join(PLUG_DIR, 'theme-retro16/manifest.json'))],
      ['main.js', fs.readFileSync(path.join(PLUG_DIR, 'theme-retro16/main.js'))],
    ]).toString('base64');

    await page.evaluate((d) => plugApi({ op: 'install_package', data: d, approved: [] }), zip);
    const restored = await page.evaluate(async () => {
      await plugRefresh();
      return {
        plugins: PLUG.list.map((p) => p.manifest.id),
        hasLabel: !!THEME_LABELS.retro16,
      };
    });
    assert.ok(restored.plugins.includes('theme-retro16'), '插件包安装恢复成功');
    assert.ok(restored.hasLabel, 'THEME_LABELS 恢复绑定');
  }));
