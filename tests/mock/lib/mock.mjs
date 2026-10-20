// mock.mjs — 把「原生 / 不可自动化」的边界替换成可脚本化的替身, 让每个功能点都测得到.
// installMocks(page, opts) 必须在 page.goto 之前调用 (内部用 addInitScript).
//   m.dialog.answer('prompt'|'confirm'|'alert', v) / m.dialog.log()   脚本化 prompt/confirm/alert 并记录 (文案 / 默认值 / 回答)
//   m.folder.next('D:/proj' | null)       下一次 pick_folder 返回指定目录 / null=取消 (模拟原生文件夹选择器)
//   m.clipboard.get()/set(t)              内存剪贴板 (navigator.clipboard 替身; clipboardDenied:true 模拟权限被拒)
//   opts.tauri=true                       仿真 __TAURI_INTERNALS__ (invoke 转发到 web 后端并记录), 走 inTauri 分支;
//                                         与真 Tauri 一致: search_cmd 收 {options} 返回裸数组, 失败 reject 字符串
//   m.win.log()                           window.open / 全屏 / 打印 调用记录
//   m.theme.set('dark'|'light')           系统明暗
//   dropFiles(page, selector, files)      派发真实拖放事件序列
//   ime(page).compose(steps, final)       CDP 模拟输入法组合输入 (imeSetComposition → insertText)
//   accel(page, key, mods)                keydown 是否被页面 preventDefault (桌面版浏览器加速键风险)
//   touchFile(path, text) / makeGitRepo(dir)
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';

export async function installMocks(page, opts = {}) {
  const cfg = { tauri: false, fakeClipboard: true, clipboardDenied: false, baseUrl: '', ...opts };
  await page.addInitScript((cfg) => {
    const M = (window.__mock = { dialogs: [], answers: { prompt: [], confirm: [], alert: [] }, folders: [], win: [], ipc: [], events: {}, clip: '', lum: 'dark' });
    const mk = (kind, dflt) => (msg, def) => {
      const q = M.answers[kind];
      const has = q.length > 0;
      const ans = has ? q.shift() : dflt(def);
      M.dialogs.push({ kind, msg: String(msg), def: def === undefined ? null : def, answer: ans, scripted: has });
      return ans;
    };
    window.prompt = mk('prompt', () => null);
    window.confirm = mk('confirm', () => false);
    window.alert = mk('alert', () => undefined);
    const rec = (n, r) => (...a) => { M.win.push({ call: n, args: a.map(String) }); return r; };
    window.open = rec('open', { closed: false, focus() {}, close() {} });
    window.print = rec('print');
    try {
      Element.prototype.requestFullscreen = function () {
        M.win.push({ call: 'requestFullscreen' });
        Object.defineProperty(document, 'fullscreenElement', { configurable: true, get: () => this });
        document.dispatchEvent(new Event('fullscreenchange'));
        return Promise.resolve();
      };
      document.exitFullscreen = function () {
        M.win.push({ call: 'exitFullscreen' });
        Object.defineProperty(document, 'fullscreenElement', { configurable: true, get: () => null });
        document.dispatchEvent(new Event('fullscreenchange'));
        return Promise.resolve();
      };
    } catch (_) {}
    if (cfg.fakeClipboard) {
      const denied = () => Promise.reject(new DOMException('denied', 'NotAllowedError'));
      const clip = {
        readText: () => (cfg.clipboardDenied ? denied() : Promise.resolve(M.clip)),
        writeText: (t) => (cfg.clipboardDenied ? denied() : ((M.clip = String(t)), Promise.resolve())),
        read: () => Promise.resolve([]),
        write: () => Promise.resolve(),
      };
      Object.defineProperty(navigator, 'clipboard', { configurable: true, get: () => clip });
      // 真实 Ctrl+V 产生的 paste 事件: 让 clipboardData 读到替身剪贴板 (系统剪贴板是空的)
      document.addEventListener('paste', (e) => {
        if (cfg.clipboardDenied) return;
        Object.defineProperty(e, 'clipboardData', { configurable: true, value: { getData: () => M.clip } });
      }, true);
    }
    const realMM = window.matchMedia.bind(window);
    window.matchMedia = (q) => {
      const r = realMM(q);
      if (/prefers-color-scheme/.test(q)) {
        const want = /dark/.test(q) ? 'dark' : 'light';
        return Object.assign(Object.create(r), { matches: M.lum === want, media: q, addEventListener() {}, removeEventListener() {}, addListener() {}, removeListener() {} });
      }
      return r;
    };
    if (cfg.tauri) {
      window.__TAURI_INTERNALS__ = {
        invoke: async (name, args) => {
          M.ipc.push({ name, args });
          if (name === 'pick_folder' && M.folders.length) return M.folders.shift();
          if (name === 'desktop_luminance') return M.lum;
          if (name.startsWith('plugin:window|')) {            // 窗口插件替身: 固定 1920x1080 显示器, 窗口 800x600 @ (100,50)
            const k = name.slice(14);
            if (k === 'current_monitor') return { position: { x: 0, y: 0 }, size: { width: 1920, height: 1080 } };
            if (k === 'outer_position') return { x: 100, y: 50 };
            if (k === 'outer_size') return { width: 800, height: 600 };
            return null;
          }
          let body = args || {};
          let url = name;
          if (name === 'search_cmd') { url = 'search'; body = (args && args.options) || {}; }
          const r = await fetch((cfg.baseUrl || '') + '/api/' + url, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) });
          const ct = r.headers.get('content-type') || '';
          let res = ct.includes('json') ? await r.json() : await r.text();
          if (!r.ok) throw (res && res.error) || 'HTTP ' + r.status;          // 真 Tauri 失败时 reject 的是字符串
          if (name === 'search_cmd' && res && res.results) res = res.results;
          return res;
        },
        transformCallback: (cb) => cb,
        plugins: {},
      };
      window.__TAURI__ = { event: { listen: async (ev, cb) => { (M.events[ev] = M.events[ev] || []).push(cb); return () => {}; } } };
    } else {
      const rf = window.fetch.bind(window);
      window.fetch = async (u, init) => {
        if (String(u).includes('/api/pick_folder') && M.folders.length) {
          const f = M.folders.shift();
          if (f === null) return new Response('null', { status: 200, headers: { 'content-type': 'application/json' } });
          return rf('/api/open_project', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ path: f }) });
        }
        return rf(u, init);
      };
    }
  }, cfg);
  const ev = (fn, ...a) => page.evaluate(fn, ...a);
  return {
    dialog: {
      answer: (kind, v) => ev(([k, x]) => window.__mock.answers[k].push(x), [kind, v]),
      log: () => ev(() => window.__mock.dialogs.slice()),
      clear: () => ev(() => { window.__mock.dialogs.length = 0; }),
    },
    folder: { next: (p) => ev((x) => window.__mock.folders.push(x), p) },
    clipboard: { get: () => ev(() => window.__mock.clip), set: (t) => ev((x) => { window.__mock.clip = x; }, t) },
    win: { log: () => ev(() => window.__mock.win.slice()) },
    theme: { set: (s) => ev((x) => { window.__mock.lum = x; }, s) },
    tauri: {
      ipc: () => ev(() => window.__mock.ipc.slice()),
      emit: (name, payload) => ev(([n, p]) => (window.__mock.events[n] || []).forEach((cb) => cb({ payload: p })), [name, payload]),
    },
  };
}

export async function dropFiles(page, selector, files, { dirPath } = {}) {
  await page.evaluate(([sel, files, dirPath]) => {
    const el = document.querySelector(sel);
    if (!el) throw new Error('no element ' + sel);
    const dt = new DataTransfer();
    for (const f of files) dt.items.add(new File([f.text || ''], f.name, { type: 'text/plain' }));
    if (dirPath) dt.setData('text/plain', dirPath);
    const r = el.getBoundingClientRect();
    const o = { bubbles: true, cancelable: true, dataTransfer: dt, clientX: r.x + 20, clientY: r.y + 20 };
    for (const t of ['dragenter', 'dragover', 'drop']) el.dispatchEvent(new DragEvent(t, o));
  }, [selector, files, dirPath || null]);
}

export function ime(page) {
  const cdp = page.context().newCDPSession(page);
  return {
    async compose(steps, final) {
      const c = await cdp;
      for (const s of steps) await c.send('Input.imeSetComposition', { text: s, selectionStart: s.length, selectionEnd: s.length });
      await c.send('Input.insertText', { text: final });
    },
    async preedit(text) { const c = await cdp; await c.send('Input.imeSetComposition', { text, selectionStart: text.length, selectionEnd: text.length }); },
    async commit(text) { const c = await cdp; await c.send('Input.insertText', { text }); },
  };
}

export async function accel(page, key, mods = {}) {
  return page.evaluate(([key, mods]) => {
    const ev = new KeyboardEvent('keydown', Object.assign({ key, bubbles: true, cancelable: true }, mods));
    (document.getElementById('ed-content') || document.body).dispatchEvent(ev);
    return ev.defaultPrevented;
  }, [key, mods]);
}

export function touchFile(p, text) {
  fs.writeFileSync(p, text);
  const t = new Date(Date.now() + 5000);
  fs.utimesSync(p, t, t);
}

export function makeGitRepo(dir) {
  const g = (...a) => execFileSync('git', a, { cwd: dir, stdio: 'pipe' });
  fs.mkdirSync(dir, { recursive: true });
  g('init', '-q'); g('config', 'user.email', 'qa@example.com'); g('config', 'user.name', 'qa');
  fs.writeFileSync(path.join(dir, 'tracked.txt'), 'one\n');
  fs.writeFileSync(path.join(dir, 'staged.txt'), 'a\n');
  g('add', '.'); g('commit', '-q', '-m', 'init');
  fs.writeFileSync(path.join(dir, 'tracked.txt'), 'one\ntwo\n');
  fs.writeFileSync(path.join(dir, 'staged.txt'), 'a\nb\n'); g('add', 'staged.txt');
  fs.writeFileSync(path.join(dir, 'new.txt'), 'n\n');
  return dir;
}
