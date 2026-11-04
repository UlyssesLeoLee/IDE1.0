// 验证 build.mjs 的两道门禁真的会失败（在 TEMP 副本上做，不碰仓库）
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { execFileSync } from "node:child_process";

const SRC_ROOT = path.resolve(process.argv[2]);
const TMP = fs.mkdtempSync(path.join(os.tmpdir(), "atlas-gate-"));

function copyDir(from, to) {
  fs.mkdirSync(to, { recursive: true });
  for (const e of fs.readdirSync(from, { withFileTypes: true })) {
    if (e.name === "node_modules") continue;
    const s = path.join(from, e.name);
    const d = path.join(to, e.name);
    if (e.isDirectory()) copyDir(s, d);
    else fs.copyFileSync(s, d);
  }
}
copyDir(path.join(SRC_ROOT, "src"), path.join(TMP, "src"));
copyDir(path.join(SRC_ROOT, "models"), path.join(TMP, "models"));
copyDir(path.join(SRC_ROOT, "vendor"), path.join(TMP, "vendor"));
fs.copyFileSync(path.join(SRC_ROOT, ".tooling", "build.mjs"), path.join(TMP, "build.mjs"));

function runBuild(label) {
  try {
    const out = execFileSync(process.execPath, [path.join(TMP, "build.mjs"), TMP], {
      encoding: "utf8", stdio: ["ignore", "pipe", "pipe"]
    });
    console.log(`[${label}] BUILD SUCCEEDED`);
    console.log(out.split("\n").slice(0, 3).join("\n"));
    return { ok: true };
  } catch (e) {
    const err = (e.stderr || "").trim();
    console.log(`[${label}] BUILD FAILED as expected:`);
    console.log(err.split("\n").slice(0, 4).map((l) => "    " + l).join("\n"));
    return { ok: false, err };
  }
}

// 基线：干净副本应构建成功
const r0 = runBuild("baseline clean");
if (!r0.ok) {
  console.log("!! 基线构建失败，后续门禁测试无意义");
  process.exit(1);
}

// 用例 1：注入语法错误
const appFile = path.join(TMP, "src", "app.js");
const appOrig = fs.readFileSync(appFile, "utf8");
fs.writeFileSync(appFile, appOrig + "\nfunction broken( {\n", "utf8");
const r1 = runBuild("syntax error");
fs.writeFileSync(appFile, appOrig, "utf8");

// 用例 2：注入不存在的模型键
fs.writeFileSync(appFile, appOrig + "\n// M.bogusModelKey\n", "utf8");
const r2 = runBuild("bogus model key");
fs.writeFileSync(appFile, appOrig, "utf8");

// 恢复后应能构建
const r3 = runBuild("restored");

console.log("\n=== GATE SUMMARY ===");
console.log("syntax gate blocks bad code :", r1.ok === false && /BUILD ABORTED/.test(r1.err || ""));
console.log("model-key gate blocks typo  :", r2.ok === false && /BUILD ABORTED/.test(r2.err || ""));
console.log("clean build still succeeds  :", r3.ok === true);

fs.rmSync(TMP, { recursive: true, force: true });
