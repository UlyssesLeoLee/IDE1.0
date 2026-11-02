#!/usr/bin/env node
/**
 * 打包插件为独立 Release 资产 (.zip)
 *
 * 遍历 crates/editcore/plugins/ 目录下的所有插件 (包括主题插件 theme-crystal, theme-retro16 及功能插件 pele, doc-graph, ppa)，
 * 打包成标准 .zip 文件输出到 dist/plugins/<id>-<version>.zip，并生成 SHA-256 校验和清单。
 */
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..');
const PLUGINS_DIR = path.resolve(ROOT, 'crates/editcore/plugins');
const OUT_DIR = path.resolve(ROOT, 'dist/plugins');

// 最小标准 ZIP 写入器 (stored, 兼容所有标准解压工具与 Rust zip crate)
function createZip(files) {
  const enc = new TextEncoder();
  const parts = [];
  const cd = [];
  let off = 0;

  const crc32 = (b) => {
    let c;
    const t = [];
    for (let n = 0; n < 256; n++) {
      c = n;
      for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
      t[n] = c >>> 0;
    }
    let r = 0xffffffff;
    for (const x of b) r = t[(r ^ x) & 0xff] ^ (r >>> 8);
    return (r ^ 0xffffffff) >>> 0;
  };

  for (const [name, data] of files) {
    const nb = enc.encode(name);
    const cs = crc32(data);
    const h = Buffer.alloc(30);
    h.writeUInt32LE(0x04034b50, 0); // Local file header signature
    h.writeUInt16LE(20, 4);         // Version needed
    h.writeUInt32LE(cs, 14);        // CRC-32
    h.writeUInt32LE(data.length, 18); // Compressed size
    h.writeUInt32LE(data.length, 22); // Uncompressed size
    h.writeUInt16LE(nb.length, 26); // File name length
    parts.push(h, Buffer.from(nb), Buffer.from(data));

    const c = Buffer.alloc(46);
    c.writeUInt32LE(0x02014b50, 0); // Central directory header
    c.writeUInt16LE(20, 4);
    c.writeUInt16LE(20, 6);
    c.writeUInt32LE(cs, 16);
    c.writeUInt32LE(data.length, 20);
    c.writeUInt32LE(data.length, 24);
    c.writeUInt16LE(nb.length, 28);
    c.writeUInt32LE(off, 42);       // Relative offset of local header
    cd.push(c, Buffer.from(nb));
    off += 30 + nb.length + data.length;
  }

  const cdb = Buffer.concat(cd);
  const e = Buffer.alloc(22);
  e.writeUInt32LE(0x06054b50, 0);   // End of central directory record
  e.writeUInt16LE(files.length, 8);
  e.writeUInt16LE(files.length, 10);
  e.writeUInt32LE(cdb.length, 12);
  e.writeUInt32LE(off, 16);

  return Buffer.concat([...parts, cdb, e]);
}

function packagePlugins() {
  if (!fs.existsSync(OUT_DIR)) {
    fs.mkdirSync(OUT_DIR, { recursive: true });
  }

  const entries = fs.readdirSync(PLUGINS_DIR, { withFileTypes: true });
  const manifestList = [];
  const checksums = [];

  for (const entry of entries) {
    if (!entry.isDirectory()) continue;
    const pluginPath = path.join(PLUGINS_DIR, entry.name);
    const manifestFile = path.join(pluginPath, 'manifest.json');
    if (!fs.existsSync(manifestFile)) continue;

    const manifestRaw = fs.readFileSync(manifestFile, 'utf8');
    let manifest;
    try {
      manifest = JSON.parse(manifestRaw);
    } catch (e) {
      console.error(`[Error] 无法解析 ${manifestFile}:`, e);
      continue;
    }

    const filesInDir = fs.readdirSync(pluginPath);
    const zipFiles = [];
    for (const f of filesInDir) {
      const fullPath = path.join(pluginPath, f);
      if (fs.statSync(fullPath).isFile()) {
        zipFiles.push([f, fs.readFileSync(fullPath)]);
      }
    }

    const zipBuffer = createZip(zipFiles);
    const zipName = `${manifest.id}-${manifest.version}.zip`;
    const zipDest = path.join(OUT_DIR, zipName);
    fs.writeFileSync(zipDest, zipBuffer);

    const sha256 = crypto.createHash('sha256').update(zipBuffer).digest('hex');
    checksums.push(`${sha256}  ${zipName}`);
    manifestList.push({
      id: manifest.id,
      name: manifest.name,
      version: manifest.version,
      type: manifest.contributes && manifest.contributes.theme ? 'theme' : 'extension',
      description: manifest.description || '',
      file: zipName,
      size: zipBuffer.length,
      sha256,
    });

    console.log(`[Packaged] ${manifest.id} (v${manifest.version}) -> ${zipName} (${zipBuffer.length} bytes, sha256: ${sha256.slice(0, 12)}...)`);
  }

  const manifestPath = path.join(OUT_DIR, 'plugins.json');
  fs.writeFileSync(manifestPath, JSON.stringify(manifestList, null, 2), 'utf8');

  const checksumPath = path.join(OUT_DIR, 'checksums.txt');
  fs.writeFileSync(checksumPath, checksums.join('\n') + '\n', 'utf8');

  console.log(`\n完成! 共打包 ${manifestList.length} 个插件 (含主题插件) 至 ${OUT_DIR}`);
}

packagePlugins();
