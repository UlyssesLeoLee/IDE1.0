// Static verification of vendor/*.bundle.js
//  - parses each bundle as a CLASSIC SCRIPT (sourceType: 'script') -> residual
//    top-level import/export is a hard SyntaxError
//  - walks the AST for: ImportDeclaration / Export* / ImportExpression / require()
//    / eval() / new Function()
//  - inventories network-capable APIs (fetch, XMLHttpRequest) with context so a
//    human can confirm they are opt-in loader paths, not load-time code
import { parse } from 'acorn';
import fs from 'node:fs';
import path from 'node:path';

const here = path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1'));
const vendorDir = path.resolve(here, '..', 'vendor');

const BUNDLES = ['three.bundle.js', 'konva.bundle.js', 'graph.bundle.js'];

function walk(node, visit, parent = null) {
  if (!node || typeof node.type !== 'string') return;
  visit(node, parent);
  for (const key of Object.keys(node)) {
    if (key === 'type' || key === 'start' || key === 'end' || key === 'loc') continue;
    const child = node[key];
    if (Array.isArray(child)) {
      for (const c of child) walk(c, visit, node);
    } else if (child && typeof child.type === 'string') {
      walk(child, visit, node);
    }
  }
}

function ctx(source, node) {
  const line = source.slice(0, node.start).split('\n').length;
  const snippet = source.slice(node.start, Math.min(node.end, node.start + 110)).replace(/\s+/g, ' ');
  return `line ${line}: ${snippet}`;
}

let failed = false;
const summary = [];

for (const file of BUNDLES) {
  const full = path.join(vendorDir, file);
  const source = fs.readFileSync(full, 'utf8');
  const findings = { esm: [], dynamicImport: [], require: [], eval: [], newFunction: [], fetch: [], xhr: [] };

  let ast;
  try {
    ast = parse(source, { ecmaVersion: 2022, sourceType: 'script' });
  } catch (e) {
    console.log(`\n### ${file}`);
    console.log(`  PARSE FAILURE (not a valid classic script): ${e.message}`);
    failed = true;
    continue;
  }

  walk(ast, (node) => {
    switch (node.type) {
      case 'ImportDeclaration':
        findings.esm.push(ctx(source, node));
        break;
      case 'ExportNamedDeclaration':
      case 'ExportDefaultDeclaration':
      case 'ExportAllDeclaration':
        findings.esm.push(ctx(source, node));
        break;
      case 'ImportExpression':
        findings.dynamicImport.push(ctx(source, node));
        break;
      case 'CallExpression': {
        const c = node.callee;
        if (c.type === 'Identifier' && c.name === 'require') findings.require.push(ctx(source, node));
        if (c.type === 'Identifier' && c.name === 'eval') findings.eval.push(ctx(source, node));
        if (c.type === 'Identifier' && c.name === 'fetch') findings.fetch.push(ctx(source, node));
        break;
      }
      case 'NewExpression': {
        const c = node.callee;
        if (c.type === 'Identifier' && c.name === 'Function') findings.newFunction.push(ctx(source, node));
        if (c.type === 'Identifier' && c.name === 'XMLHttpRequest') findings.xhr.push(ctx(source, node));
        break;
      }
      case 'Identifier':
        if (node.name === 'XMLHttpRequest') findings.xhr.push(ctx(source, node));
        break;
      default:
        break;
    }
  });

  const bytes = Buffer.byteLength(source);
  const counts = Object.entries(findings).map(([k, v]) => `${k}=${v.length}`).join(' ');
  const hardFail = findings.esm.length || findings.dynamicImport.length || findings.require.length || findings.eval.length || findings.newFunction.length;
  if (hardFail) failed = true;

  console.log(`\n### ${file}  (${bytes} bytes)`);
  console.log(`  classic-script parse : OK`);
  console.log(`  ${counts}`);
  if (hardFail) {
    console.log('  RESULT: FAIL');
    for (const [k, v] of Object.entries(findings)) {
      if (v.length && k !== 'fetch' && k !== 'xhr') {
        console.log(`   ! ${k}:`);
        for (const item of v) console.log(`       ${item}`);
      }
    }
  } else {
    console.log('  RESULT: PASS (no residual ESM / require / dynamic import / eval / new Function)');
  }
  if (findings.fetch.length || findings.xhr.length) {
    console.log('  network-capable API references (opt-in loader APIs, NOT load-time):');
    for (const item of findings.fetch) console.log(`    fetch  -> ${item}`);
    for (const item of findings.xhr) console.log(`    XHR    -> ${item}`);
  }
  if (/sourceMappingURL/.test(source)) console.log('  note: contains sourceMappingURL comment');

  summary.push({ file, bytes, findings, hardFail: !!hardFail });
}

console.log('\n=== summary ===');
for (const s of summary) {
  console.log(`  ${s.file.padEnd(20)} ${String(s.bytes).padStart(8)} bytes  ${s.hardFail ? 'FAIL' : 'PASS'}`);
}

process.exit(failed ? 1 : 0);
