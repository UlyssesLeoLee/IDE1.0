// 回归: 导入 sakura.ini (WBS-02 3.7 子集) —— [Common] 与 [Types(n)] 的认得的键进首选项 / 类型别设置, 其余忽略
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withMocks } from '../../lib/with-mocks.mjs';

const INI = '[Common]\r\nnMRUArrNum_MAX=7\r\nbExitConfirm=0\r\nbRegularExp=1\r\nunknown=1\r\n[Types(0)]\r\nnTabSpace=2\r\nbInsSpace=1\r\n[Types(3)]\r\nszTypeExts=rs\r\nnTabSpace=8\r\nbAutoIndent=0\r\n';

test('导入 sakura.ini: 全局与按类型的设置生效', () =>
  withMocks({ name: 'ini', portOffset: 626 }, async ({ page }) => {
    await page.locator('#ed-content').click();
    const r = await page.evaluate((t) => { const n = importSakuraIni(t); return { n, p: [PREFS.mruFiles, PREFS.confirmClose, PREFS.findRegex, PREFS.tabWidth, PREFS.insertSpaces], rs: typeSettingsAll().rust }; }, INI);
    assert.deepEqual(r.p, [7, false, true, 2, true]);
    assert.deepEqual(r.rs, { tabWidth: 8, autoIndent: false });
    assert.equal(r.n, 7, '认得的项数 (不含 unknown)');
    assert.equal(await page.evaluate(() => importSakuraIni('[X]\r\na=1')), 0, '没有认得的项 = 0');
  }));
