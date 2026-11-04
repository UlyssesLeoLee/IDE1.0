// Offline smoke test for vendor/*.bundle.js
// Opens the vendor folder's _smoke.html straight from file:// in the SYSTEM Chrome
// (no bundled Chromium download) and asserts:
//   - the four globals are present
//   - ZERO http(s) requests are attempted (offline guarantee, empirically)
import { chromium } from 'playwright';
import { pathToFileURL } from 'node:url';
import path from 'node:path';
import fs from 'node:fs';

const here = path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1'));
const vendorDir = path.resolve(here, '..', 'vendor');
const smoke = path.join(vendorDir, '_smoke.html');

if (!fs.existsSync(smoke)) {
  console.error(`FATAL: smoke page not found: ${smoke}`);
  process.exit(2);
}

const url = pathToFileURL(smoke).href;
console.log(`smoke page : ${smoke}`);
console.log(`file url   : ${url}`);

const browser = await chromium.launch({ channel: 'chrome' });
const context = await browser.newContext();
const page = await context.newPage();

const requests = [];
const consoleErrors = [];
const pageErrors = [];

page.on('request', (r) => requests.push(r.url()));
page.on('console', (m) => {
  if (m.type() === 'error') consoleErrors.push(m.text());
});
page.on('pageerror', (e) => pageErrors.push(String(e)));

await page.goto(url, { waitUntil: 'load' });
await page.waitForFunction(() => document.title && document.title !== 'SMOKE_PENDING', null, { timeout: 30000 });

const title = await page.title();
const details = await page.evaluate(() => ({
  THREE_REVISION: typeof THREE !== 'undefined' ? THREE.REVISION : 'UNDEFINED',
  Konva_Stage: typeof Konva !== 'undefined' ? typeof Konva.Stage : 'UNDEFINED',
  Graphology_Graph: typeof Graphology !== 'undefined' ? typeof Graphology.Graph : 'UNDEFINED',
  Dagre_graphlib: typeof Dagre !== 'undefined' ? typeof Dagre.graphlib : 'UNDEFINED',
  Graphology_keys: typeof Graphology !== 'undefined' ? Object.keys(Graphology).join(',') : 'UNDEFINED',
  Dagre_keys: typeof Dagre !== 'undefined' ? Object.keys(Dagre).join(',') : 'UNDEFINED',
  // deeper API probes — a present-but-empty global would still pass the four checks above
  THREE_deep: [
    'Scene', 'WebGLRenderer', 'PerspectiveCamera', 'BoxGeometry', 'MeshStandardMaterial',
    'Vector3', 'Color', 'Group', 'AmbientLight', 'DirectionalLight', 'PlaneGeometry', 'Mesh'
  ].map((k) => `${k}:${typeof THREE[k]}`).join(' '),
  Konva_deep: ['Stage', 'Layer', 'Rect', 'Circle', 'Line', 'Text', 'Group', 'Transform', 'Animation', 'Tween']
    .map((k) => `${k}:${typeof Konva[k]}`).join(' '),
  Graphology_deep: ['Graph', 'DirectedGraph', 'UndirectedGraph', 'MultiGraph', 'MultiDirectedGraph']
    .map((k) => `${k}:${typeof Graphology[k]}`).join(' '),
  Dagre_deep: ['graphlib', 'layout', 'version']
    .map((k) => `${k}:${typeof Dagre[k]}`).join(' '),
  // functional probes: actually build a graph and lay it out
  dagre_layout_probe: (() => {
    try {
      const g = new Dagre.graphlib.Graph();
      g.setGraph({ rankdir: 'LR' });
      g.setDefaultEdgeLabel(() => ({}));
      g.setNode('a', { width: 50, height: 30 });
      g.setNode('b', { width: 50, height: 30 });
      g.setEdge('a', 'b');
      Dagre.layout(g);
      return `ok nodes=${g.nodes().length} edge=${JSON.stringify(g.edge('a', 'b'))}`;
    } catch (e) {
      return `THREW ${e && e.message}`;
    }
  })(),
  graphology_probe: (() => {
    try {
      const g = new Graphology.Graph();
      g.addNode('x');
      g.addNode('y');
      g.addDirectedEdge('x', 'y');
      return `ok order=${g.order} size=${g.size} hasEdge=${g.hasDirectedEdge('x', 'y')}`;
    } catch (e) {
      return `THREW ${e && e.message}`;
    }
  })(),
  three_probe: (() => {
    try {
      const s = new THREE.Scene();
      const c = new THREE.PerspectiveCamera(50, 1, 0.1, 100);
      c.position.set(0, 0, 5);
      s.add(c);
      return `ok children=${s.children.length} version=${THREE.REVISION}`;
    } catch (e) {
      return `THREW ${e && e.message}`;
    }
  })()
}));

await browser.close();

console.log('\n=== document.title ===');
console.log(title);
console.log('\n=== evaluated details ===');
console.log(JSON.stringify(details, null, 2));

console.log('\n=== network requests observed ===');
for (const r of requests) console.log(`  ${r}`);
const remote = requests.filter((u) => /^https?:/i.test(u));

console.log('\n=== console errors ===');
console.log(consoleErrors.length ? consoleErrors.join('\n') : '  (none)');
console.log('=== page errors ===');
console.log(pageErrors.length ? pageErrors.join('\n') : '  (none)');

const required = ['THREE', 'Konva', 'Graphology', 'Dagre'];
const missing = required.filter((n) => !title.includes(n));
const failures = [];
if (missing.length) failures.push(`title missing global name(s): ${missing.join(', ')}`);
if (details.THREE_REVISION === 'UNDEFINED') failures.push('THREE.REVISION undefined');
if (details.Konva_Stage !== 'function') failures.push(`Konva.Stage is ${details.Konva_Stage}, expected function`);
if (details.Graphology_Graph !== 'function') failures.push(`Graphology.Graph is ${details.Graphology_Graph}, expected function`);
if (details.Dagre_graphlib !== 'object') failures.push(`Dagre.graphlib is ${details.Dagre_graphlib}, expected object`);
if (remote.length) failures.push(`remote requests attempted: ${remote.join(', ')}`);
if (pageErrors.length) failures.push(`page errors: ${pageErrors.join(' / ')}`);
if (/THREW/.test(details.dagre_layout_probe)) failures.push(`dagre layout probe: ${details.dagre_layout_probe}`);
if (/THREW/.test(details.graphology_probe)) failures.push(`graphology probe: ${details.graphology_probe}`);
if (/THREW/.test(details.three_probe)) failures.push(`three probe: ${details.three_probe}`);
for (const [name, blob] of [['THREE', details.THREE_deep], ['Konva', details.Konva_deep], ['Graphology', details.Graphology_deep], ['Dagre', details.Dagre_deep]]) {
  const bad = blob.split(' ').filter((p) => !/:(function|object|string|number|boolean)$/.test(p));
  if (bad.length) failures.push(`${name} deep probe missing/non-typed members: ${bad.join(' ')}`);
}

console.log('\n=== result ===');
if (failures.length) {
  console.log('FAIL');
  for (const f of failures) console.log(`  - ${f}`);
  process.exit(1);
}
console.log('PASS — all four globals present, zero remote requests.');
console.log(`\nTITLE_STRING=${title}`);
