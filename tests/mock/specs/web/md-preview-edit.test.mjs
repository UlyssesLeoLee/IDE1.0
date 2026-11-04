// 回归: 预览模式 —— 和原文二选一 (含透明玻璃主题)、点块就地编辑、点目录滚动; 标题栏自适应 + logo 不随主题变
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';

const LINES = ['# 标题甲', '', '第一段 **粗**', '', ...Array.from({ length: 60 }, (_, i) => '填充 ' + i + String.fromCharCode(10)), '## 小节乙', '', '尾段'];
const setup = (page) => page.evaluate(async (L) => {
  const t = getActiveTab(); t.name = 'p.md'; t.path = '/x/p.md'; t.untitled = false; t.lines = L.join(String.fromCharCode(10)).split(String.fromCharCode(10)); t.cursor = { row: 0, col: 0 }; t.sel = null; t.dirty = false; renderVim(t);
  toggleMdPreview(); await renderMdPreview();
}, LINES);

test('预览打开时原文层被藏起来 (透明玻璃主题也一样), 关了又回来', () =>
  withMocks({ name: 'mp', portOffset: 645 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    await page.evaluate(() => document.documentElement.setAttribute('data-theme', 'clear'));
    await setup(page);
    await page.waitForSelector('#md-preview [data-l]');
    const vis = () => page.evaluate(() => [getComputedStyle(document.getElementById('ed-content')).visibility, getComputedStyle(document.getElementById('md-preview')).display]);
    assert.deepEqual(await vis(), ['hidden', 'block']);
    await page.evaluate(() => toggleMdPreview());
    assert.deepEqual(await vis(), ['visible', 'none']);
  }));

test('点预览里的块 -> 就地编辑原文行, Ctrl+Enter 提交 (一步撤销); Esc 放弃; 目录点击滚动预览', () =>
  withMocks({ name: 'mp', portOffset: 646 }, async ({ page, snap }) => {
    await page.locator('#ed-content').click();
    await setup(page);
    await page.waitForSelector('#md-preview p[data-l="2-2"]');
    await page.locator('#md-preview p[data-l="2-2"]').click();
    const ta = page.locator('#md-preview textarea.md-edit-ta');
    await ta.waitFor();
    assert.equal(await ta.inputValue(), '第一段 **粗**');
    await ta.fill('改过的段落'); await ta.press('Control+Enter');
    await page.waitForFunction(() => getActiveTab().lines[2] === '改过的段落');
    await page.waitForSelector('#md-preview p:has-text("改过的段落")');
    await page.evaluate(() => dispatchCommand('undo')); await page.evaluate(() => window.__editIdle());
    assert.equal((await snap()).lines[2], '第一段 **粗**', '一步撤销');
    // Esc 放弃
    await page.waitForSelector('#md-preview p[data-l="2-2"]');
    await page.locator('#md-preview p[data-l="2-2"]').click();
    await page.locator('#md-preview textarea.md-edit-ta').fill('不要保存'); await page.locator('#md-preview textarea.md-edit-ta').press('Escape');
    await page.waitForFunction(() => !document.querySelector('#md-preview textarea'));
    assert.equal((await page.evaluate(() => getActiveTab().lines[2])), '第一段 **粗**');
    // 目录点击 -> 预览滚到「小节乙」
    const idx = await page.evaluate(() => getActiveTab().lines.indexOf('## 小节乙'));
    const top0 = await page.evaluate(() => document.getElementById('md-preview').scrollTop);
    const ok = await page.evaluate((i) => mdPreviewScrollTo(i), idx);
    assert.equal(ok, true);
    assert.ok(await page.evaluate(() => document.getElementById('md-preview').scrollTop) > top0, '预览滚动了');
  }));

test('标题栏: 标题一行不折, 窄窗口不溢出; logo 是固定图片, 换主题不变', () =>
  withMocks({ name: 'mp', portOffset: 647 }, async ({ page }) => {
    await page.setViewportSize({ width: 640, height: 500 });
    await page.locator('#ed-content').click();
    const info = () => page.evaluate(() => {
      const t = document.getElementById('tb-name'), m = document.getElementById('menubar'), l = document.querySelector('#tb-title img.tb-logo');
      return { oneLine: t.getBoundingClientRect().height < 24, fits: m.scrollWidth <= m.clientWidth + 1, src: l && l.getAttribute('src'), w: l && l.getBoundingClientRect().width };
    });
    const a = await info();
    assert.ok(a.oneLine, '标题只占一行'); assert.ok(a.fits, '菜单栏不横向溢出'); assert.equal(a.src, 'logo.png');
    await page.evaluate(() => document.documentElement.setAttribute('data-theme', 'light'));
    const b = await info();
    assert.deepEqual([b.src, b.w], [a.src, a.w], '主题变了 logo 不变');
    await page.evaluate(() => { const t = getActiveTab(); t.name = '一个非常非常非常长的文件名用来测试标题省略号.md'; syncDocChrome(); });
    assert.ok((await info()).oneLine, '长文件名也只占一行 (省略号)');
  }));

test('评审修复: 提交后立刻点另一个块不会改错行; 提交清掉源码层的旧选区', () =>
  withMocks({ name: 'mp', portOffset: 650 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    await page.evaluate(async () => {
      const t = getActiveTab(); t.name = 'q.md'; t.path = '/x/q.md'; t.untitled = false;
      t.lines = ['# 标题', '', '第一段', '', '第二段', '', '第三段']; t.cursor = { row: 0, col: 0 }; t.sel = { r0: 0, c0: 0, r1: 0, c1: 2 }; renderVim(t);
      toggleMdPreview(); await renderMdPreview();
    });
    await page.waitForSelector('#md-preview p[data-l="2-2"]');
    await page.locator('#md-preview p[data-l="2-2"]').click();
    // 把第一段扩成 3 行 (后面的块行号要整体后移 2)
    const ta = page.locator('#md-preview textarea.md-edit-ta');
    await ta.fill('甲' + String.fromCharCode(10) + '乙' + String.fromCharCode(10) + '丙');
    // 紧接着点「第二段」: 失焦提交和点击在同一轮里发生 (重画还没回来)。关键是**不会改错行**: 可能当场打开第二段, 也可能这一下被忽略 (布局在 mousedown 与 mouseup 之间变了), 再点一次就行
    await page.locator('#md-preview p[data-l="4-4"]').click({ timeout: 5000 }).catch(() => {});
    await page.waitForFunction(() => getActiveTab().lines.length === 9, null, { timeout: 5000 });
    await page.waitForFunction(() => !document.querySelector('#md-preview .md-editing') || document.querySelector('#md-preview textarea.md-edit-ta'));
    if (!(await page.locator('#md-preview textarea.md-edit-ta').count())) {
      await page.waitForSelector('#md-preview p:has-text("第二段")');
      await page.locator('#md-preview p:has-text("第二段")').click();
    }
    await page.waitForSelector('#md-preview textarea.md-edit-ta');
    const v = await page.locator('#md-preview textarea.md-edit-ta').inputValue();
    assert.equal(v, '第二段', '打开的必须是第二段 (行号已按新增的两行平移, 不会落到别的行)');
    const st = await page.evaluate(() => ({ lines: getActiveTab().lines.slice(), sel: getActiveTab().sel }));
    assert.deepEqual(st.lines, ['# 标题', '', '甲', '乙', '丙', '', '第二段', '', '第三段']);
    assert.equal(st.sel, null, '源码层的旧选区被清掉');
  }));
