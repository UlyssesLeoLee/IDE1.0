import fs from "node:fs";
import path from "node:path";
const ROOT = "E:/IDE1.0/gemarc-5d-atlas-v10.0";

// 1) 模型 schema 标识统一为 ide1-5d-atlas（去掉跨项目命名残留）
const MODEL_FILES = fs.readdirSync(path.join(ROOT, "models")).filter((f) => f.endsWith(".json"));
let renamed = 0;
for (const f of MODEL_FILES) {
  const p = path.join(ROOT, "models", f);
  const t = fs.readFileSync(p, "utf8");
  const n = t.replace(/"gemarc-5d-atlas\//g, '"ide1-5d-atlas/');
  if (n !== t) { fs.writeFileSync(p, n, "utf8"); renamed++; }
}
for (const f of fs.readdirSync(path.join(ROOT, "design-system"))) {
  const p = path.join(ROOT, "design-system", f);
  const t = fs.readFileSync(p, "utf8");
  const n = t.replace(/"gemarc-5d-atlas\//g, '"ide1-5d-atlas/');
  if (n !== t) { fs.writeFileSync(p, n, "utf8"); renamed++; }
}
console.log("schema ids renamed:", renamed);

// 2) 設計辭典の外部プロジェクト固有名を除去
const vd = path.join(ROOT, "design-system/visual-dictionary.json");
let v = fs.readFileSync(vd, "utf8");
v = v.replace(
  "只有进入 3D 空间投影（spatial.json 的 nodeOffsets）的节点才使用宝石；前端 UI 层节点（gm-console / gm-desktop / prototype）不进入 3D，不宝石化。",
  "只有进入 3D 空间投影（spatial.json 的 nodeOffsets）的节点才使用宝石；前端 UI 层节点（editor.html / 第二客户端 / 原型示范屏）不进入 3D，不宝石化。"
);
fs.writeFileSync(vd, v, "utf8");
console.log("visual-dictionary cleaned:", !v.includes("gm-console"));

// 3) 旧 E2E 仕様は他プロジェクト前提のままで失敗するため、実装済みの旅程ランナーに委譲する
const e2e = path.join(ROOT, "tests/e2e/atlas.spec.mjs");
fs.writeFileSync(e2e, `/**
 * tests/e2e/atlas.spec.mjs — file:// 実ブラウザ E2E（12 旅程 / 55 実測）
 *
 * 実体は .tooling/e2e-journeys.mjs に置かれ、ここは門禁の入口として同じ検査を実行する。
 * 実行: node tests/e2e/atlas.spec.mjs   （または node .tooling/e2e-journeys.mjs）
 *
 * 注意: 旧版は別プロジェクト前提の検証（タイトル文字列・別フォーマット名など）を含み
 * そのままでは失敗する。対象プロジェクトの前提に依存しない検査へ更新済み。
 */
import "../../.tooling/e2e-journeys.mjs";
`, "utf8");
console.log("e2e spec rewritten");

// 4) 使い捨ての修补/診断スクリプトを retired/ へ退避（誤実行防止）
const RETIRED = [
  "apply-audit-fixes.mjs", "apply-audit-fixes2.mjs", "fix-app.mjs", "fix-app2.mjs",
  "fix-app3.mjs", "fix-app4.mjs", "fix-app5.mjs", "fix-app6.mjs", "fix-diagrams.mjs",
  "fix-edge-id.mjs", "fix-models.mjs", "fix-rule-priority.mjs", "bisect-blocks.mjs",
  "bisect-syntax.mjs", "diagnose-2d-proto.mjs", "diagnose-chain.mjs", "diagnose-j03-j09.mjs",
  "diagnose-page.mjs", "diagnose-views.mjs", "find-bad-block.mjs", "find-bad-func.mjs",
  "locate-syntax.mjs", "probe-edge.mjs", "mutation-test.mjs", "repro-undo.mjs"
];
const retiredDir = path.join(ROOT, ".tooling/retired");
fs.mkdirSync(retiredDir, { recursive: true });
let moved = 0;
for (const f of RETIRED) {
  const src = path.join(ROOT, ".tooling", f);
  if (fs.existsSync(src)) { fs.renameSync(src, path.join(retiredDir, f)); moved++; }
}
fs.writeFileSync(path.join(retiredDir, "README.md"), `# retired/ — 退避した一次性スクリプト

これらは **5D 図集エンジンの初期反復**で使った使い捨ての修补/診断スクリプトであり、
IDE1.0 のモデルに対しては**実行してはならない**（別プロジェクトの前提・ID を前提に-written されている）。

- 使い捨て: \`fix-*.mjs\` / \`apply-audit-fixes*.mjs\` / \`diagnose-*.mjs\` / \`bisect-*.mjs\` /
  \`find-bad-*.mjs\` / \`locate-syntax.mjs\` / \`probe-edge.mjs\` / \`mutation-test.mjs\` / \`repro-undo.mjs\`
- 保持する本体: \`.tooling/build.mjs\`（构建）/ \`.tooling/verify-refs.mjs\`（参照整合）/
  \`.tooling/verify-cases.mjs\`（4D/5D 実走と被覆率）/ \`.tooling/e2e-journeys.mjs\`（12 旅程）/
  \`.tooling/screenshots.mjs\`（視覚証拠）/ \`.tooling/verify*.mjs\`（vendor 検証）/
  \`.tooling/check-src.mjs\`（構文）/ \`.tooling/smoke.mjs\`（vendor smoke）

移動数: ${moved}
`, "utf8");
console.log("retired scripts moved:", moved);
