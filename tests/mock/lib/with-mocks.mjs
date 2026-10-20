// with-mocks.mjs — 起 server + 开页面 + 装测试替身 (必须在 goto 之前装) 的一站式辅助.
import { tmpProject, startServer, openApp, gotoEditor, closeApp, snap, press, PORT_BASE } from './harness.mjs';
import { installMocks } from './mock.mjs';

export async function withMocks(opts, fn) {
  const root = tmpProject(opts.name || 'm');
  const srv = await startServer({ port: PORT_BASE + (opts.portOffset || 0), root, extraEnv: opts.env || {} });
  const app = await openApp({ base: srv.base, layout: opts.layout, vim: opts.vim, viewport: opts.viewport });
  const m = await installMocks(app.page, { baseUrl: srv.base, ...(opts.mock || {}) });
  try {
    await gotoEditor(app);
    return await fn({ page: app.page, app, srv, root, m, snap: () => snap(app.page), press: (k, o) => press(app.page, k, o) });
  } finally {
    await closeApp(app);
    srv.stop();
  }
}
