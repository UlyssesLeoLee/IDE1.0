// 回归: 界面语言 (中/日/英) / 分层菜单与自适应滚动 / 左侧导航·底部终端可拖动调整 / 终端设置
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';
import { fill } from '../../lib/harness.mjs';

const cmd = (page, c) => page.evaluate((x) => dispatchCommand(x), c);
const menuLabels = (page) => page.evaluate(() => [...document.querySelectorAll('#menubar > .menu-item > .mb-btn')].map((b) => b.textContent.replace(/[A-Z]$/, '')));

test('界面语言: 中 → 日 → 英 → 中, 菜单 / 提示 / 对话框都跟着换, 重载后保持, 切回中文还原', () =>
  withMocks({ name: 'im', portOffset: 290 }, async ({ page, press }) => {
    assert.deepEqual(await menuLabels(page), ['文件', '编辑', '搜索', '视图', '工具', '设置', '窗口', '帮助']);
    await cmd(page, 'lang-ja'); await page.waitForTimeout(300);
    assert.deepEqual(await menuLabels(page), ['ファイル', '編集', '検索', '表示', 'ツール', '設定', 'ウィンドウ', 'ヘルプ']);
    await fill(page, 'x');
    await press('Control+a'); await cmd(page, 'copy'); await page.waitForTimeout(400);
    assert.match(await page.evaluate(() => document.getElementById('s-tip').textContent), /コピーしました/);
    await cmd(page, 'lang-en'); await page.waitForTimeout(300);
    assert.deepEqual(await menuLabels(page), ['File', 'Edit', 'Search', 'View', 'Tools', 'Settings', 'Window', 'Help']);
    await cmd(page, 'common-config'); await page.waitForTimeout(300);
    assert.match(await page.evaluate(() => document.getElementById('md-body').innerText), /Font size|Auto indent/);
    await page.locator('#md-foot [data-cmd="modal-close"]').click();
    assert.equal(await page.evaluate(() => localStorage.getItem('ide10.lang')), 'en');
    await page.reload(); await page.waitForTimeout(800);
    assert.deepEqual(await menuLabels(page), ['File', 'Edit', 'Search', 'View', 'Tools', 'Settings', 'Window', 'Help']);
    await cmd(page, 'lang-zh'); await page.waitForTimeout(300);
    assert.deepEqual(await menuLabels(page), ['文件', '编辑', '搜索', '视图', '工具', '设置', '窗口', '帮助']);
    await cmd(page, 'common-config'); await page.waitForTimeout(300);
    assert.match(await page.evaluate(() => document.getElementById('md-body').innerText), /字号/);
  }));

test('三种语言的菜单: 每个命令都有三语名字, 没有漏翻 (英文菜单里不含汉字/假名)', () =>
  withMocks({ name: 'im', portOffset: 291 }, async ({ page }) => {
    await cmd(page, 'lang-en'); await page.waitForTimeout(300);
    const bad = await page.evaluate(() => [...document.querySelectorAll('#menubar .menu-row > span:first-child')].map((e) => e.textContent).filter((t) => /[぀-ヿ一-鿿]/.test(t) && !/中文|日本語|IDE1.0/.test(t)));
    assert.deepEqual(bad, []);
    await cmd(page, 'lang-ja'); await page.waitForTimeout(300);
    const badJa = await page.evaluate(() => [...document.querySelectorAll('#menubar .menu-row > span:first-child')].map((e) => e.textContent).filter((t) => /[简体删设档]|无|个|选|应|将|关闭/.test(t) && !/中文|IDE1.0/.test(t)));
    assert.deepEqual(badJa, [], '日文菜单里没有残留的简体中文');
  }));

test('菜单分层: 编辑/搜索/视图 的顶层行数 ≤ 22, 其余收进子菜单; 窗口很矮时下拉出现滚动条, 子菜单不被裁掉', () =>
  withMocks({ name: 'im', portOffset: 292, viewport: { width: 1100, height: 420 } }, async ({ page }) => {
    const rows = await page.evaluate(() => Object.fromEntries([...document.querySelectorAll('#menubar > .menu-item')].map((m) => [m.dataset.menu, m.querySelectorAll(':scope > .menu-dropdown > .menu-row').length])));
    for (const k of ['file', 'edit', 'search', 'view', 'tool', 'setting', 'window', 'help']) assert.ok(rows[k] <= 22, `${k} 顶层 ${rows[k]} 行`);
    await page.locator('.menu-item[data-menu="edit"] > .mb-btn').click();
    const dd = await page.evaluate(() => { const d = document.querySelector('.menu-dropdown[data-dropdown="edit"]'); const b = d.getBoundingClientRect(); return { bottom: b.bottom, vh: innerHeight, scroll: d.scrollHeight > d.clientHeight, oy: getComputedStyle(d).overflowY }; });
    assert.ok(dd.bottom <= dd.vh + 1, '下拉不超出窗口底边: ' + JSON.stringify(dd));
    assert.equal(dd.oy, 'auto');
    await page.locator('.menu-dropdown[data-dropdown="edit"] [data-submenu="edit-lines"]').hover(); await page.waitForTimeout(250);
    const sub = await page.evaluate(() => { const s = document.querySelector('[data-submenu="edit-lines"] > .submenu-dropdown'); const b = s.getBoundingClientRect(); return { top: b.top, bottom: b.bottom, left: b.left, right: b.right, vh: innerHeight, vw: innerWidth, disp: getComputedStyle(s).display, pos: getComputedStyle(s).position }; });
    assert.equal(sub.disp, 'block'); assert.equal(sub.pos, 'fixed');
    assert.ok(sub.top >= 0 && sub.bottom <= sub.vh + 1 && sub.right <= sub.vw + 1, '子菜单整个在窗口内: ' + JSON.stringify(sub));
    // 像真人一样: 先横向移进子菜单, 再在子菜单里上下移到目标行 (斜着穿过别的行会切换子菜单)
    const geo = await page.evaluate(() => { const row = document.querySelector('[data-submenu="edit-lines"]').getBoundingClientRect(); const sub = document.querySelector('[data-submenu="edit-lines"] > .submenu-dropdown'); const it = sub.querySelector('[data-cmd="duplicate-line"]'); it.scrollIntoView({ block: 'nearest' }); const ib = it.getBoundingClientRect(); const sb = sub.getBoundingClientRect(); return { ry: row.top + row.height / 2, sx: sb.left + 12, sTop: sb.top, iy: ib.top + ib.height / 2, left: sb.left, right: row.right }; });
    await page.mouse.move(geo.sx, Math.min(Math.max(geo.ry, geo.sTop + 6), geo.iy), { steps: 4 });
    await page.mouse.move(geo.sx, geo.iy, { steps: 4 });
    await page.mouse.click(geo.sx, geo.iy);                       // 点得到 (子菜单没被裁掉)
    await page.waitForTimeout(150);
    assert.equal(await page.evaluate(() => document.querySelectorAll('.menu-item.open').length), 0, '执行命令后菜单已收起');
    await page.keyboard.press('Escape');
  }));

test('左侧导航 / 底部终端: 拖动手柄调整宽高, 落盘并在重载后还原, 双击恢复; 内容超出出现滚动条', () =>
  withMocks({ name: 'im', portOffset: 293, layout: 'full', viewport: { width: 1280, height: 800 } }, async ({ page }) => {
    const size = () => page.evaluate(() => ({ sw: document.getElementById('side-bar').getBoundingClientRect().width, bh: document.getElementById('bottom-body').getBoundingClientRect().height }));
    const s0 = await size();
    const r = await page.evaluate(() => { const b = document.getElementById('side-resizer').getBoundingClientRect(); return { x: b.left + b.width / 2, y: b.top + 80 }; });
    await page.mouse.move(r.x, r.y); await page.mouse.down(); await page.mouse.move(r.x + 120, r.y, { steps: 6 }); await page.mouse.up();
    const s1 = await size();
    assert.ok(s1.sw > s0.sw + 90, `侧栏变宽: ${s0.sw} → ${s1.sw}`);
    const r2 = await page.evaluate(() => { const b = document.getElementById('side-resizer').getBoundingClientRect(); return { x: b.left + b.width / 2, y: b.top + 80 }; });
    await page.mouse.move(r2.x, r2.y); await page.mouse.down(); await page.mouse.move(r2.x - 200, r2.y, { steps: 6 }); await page.mouse.up();
    const s2 = await size(); assert.ok(s2.sw < s1.sw - 150, `侧栏变窄: ${s1.sw} → ${s2.sw}`);
    await page.reload(); await page.waitForTimeout(700);
    assert.ok(Math.abs((await size()).sw - s2.sw) < 3, '重载后宽度还原');
    // 底部终端
    await page.keyboard.press('Control+`'); await page.waitForTimeout(400);
    const b0 = (await size()).bh;
    const h = await page.evaluate(() => { const b = document.getElementById('bottom-resizer').getBoundingClientRect(); return { x: b.left + 200, y: b.top + b.height / 2 }; });
    await page.mouse.move(h.x, h.y); await page.mouse.down(); await page.mouse.move(h.x, h.y - 120, { steps: 6 }); await page.mouse.up();
    const b1 = (await size()).bh; assert.ok(b1 > b0 + 90, `终端变高: ${b0} → ${b1}`);
    await page.locator('#bottom-resizer').dblclick(); await page.waitForTimeout(150);
    assert.ok(Math.abs((await size()).bh - b0) < 3, '双击恢复默认高度');
    // 滚动条: 侧栏内容超宽 → 横向滚动; 终端内容超高 → 纵向滚动
    const sc = await page.evaluate(() => { const sb = document.getElementById('side-body'); const x = document.createElement('div'); x.className = 'tree-node'; x.textContent = 'a_very_long_file_name_'.repeat(12); document.getElementById('explorer-pane') ? document.getElementById('explorer-pane').appendChild(x) : sb.appendChild(x); const tb = document.getElementById('terminal-body'); tb.innerHTML = Array.from({ length: 200 }, (_, i) => '<div class="term-line">line ' + i + '</div>').join(''); return { hs: sb.scrollWidth > sb.clientWidth, ox: getComputedStyle(sb).overflowX, vs: tb.scrollHeight > tb.clientHeight, oy: getComputedStyle(tb).overflowY }; });
    assert.equal(sc.hs, true, '侧栏出现横向滚动'); assert.ok(['auto', 'scroll'].includes(sc.ox));
    assert.equal(sc.vs, true, '终端出现纵向滚动'); assert.ok(['auto', 'scroll'].includes(sc.oy));
  }));

test('终端设置: 字号/行高/配色/折行/回滚行数/显示行数 立即生效并落盘; 回滚行数限制保留的行', () =>
  withMocks({ name: 'im', portOffset: 294, layout: 'full' }, async ({ page }) => {
    await page.keyboard.press('Control+`'); await page.waitForTimeout(300);
    await cmd(page, 'term-config'); await page.waitForTimeout(200);
    await page.fill('#tc-fs', '18'); await page.fill('#tc-lh', '1.8'); await page.fill('#tc-sb', '100'); await page.fill('#tc-rows', '12');
    await page.selectOption('#tc-scheme', 'ps'); await page.uncheck('#tc-wrap');
    await page.locator('#md-foot [data-cmd="term-config-apply"]').click(); await page.waitForTimeout(200);
    const st = await page.evaluate(() => { const tb = document.getElementById('terminal-body'); const cs = getComputedStyle(tb); return { fs: cs.fontSize, lh: cs.lineHeight, scheme: tb.dataset.scheme, wrap: tb.dataset.wrap, ws: cs.whiteSpace, bh: document.getElementById('bottom-body').getBoundingClientRect().height }; });
    assert.equal(st.fs, '18px'); assert.equal(st.scheme, 'ps'); assert.equal(st.wrap, 'off'); assert.equal(st.ws, 'pre');
    assert.ok(Math.abs(parseFloat(st.lh) - 32.4) < 1, '行高 18×1.8');
    assert.ok(st.bh >= 12 * 32 - 10, '面板高度 ≈ 12 行: ' + st.bh);
    await page.evaluate(() => { for (let i = 0; i < 250; i++) appendTerminal('row ' + i + '\n'); });
    assert.ok((await page.evaluate(() => document.querySelectorAll('#terminal-body .term-line').length)) <= 100, '只保留最近 100 行');
    assert.equal(await page.evaluate(() => JSON.parse(localStorage.getItem('ide10.term')).scheme), 'ps');
    await page.reload(); await page.waitForTimeout(700);
    assert.equal(await page.evaluate(() => document.getElementById('terminal-body').dataset.scheme), 'ps');
  }));
