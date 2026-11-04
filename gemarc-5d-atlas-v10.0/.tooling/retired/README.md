# retired/ — 退避した一次性スクリプト

これらは **5D 図集エンジンの初期反復**で使った使い捨ての修补/診断スクリプトであり、
IDE1.0 のモデルに対しては**実行してはならない**（別プロジェクトの前提・ID を前提に-written されている）。

- 使い捨て: `fix-*.mjs` / `apply-audit-fixes*.mjs` / `diagnose-*.mjs` / `bisect-*.mjs` /
  `find-bad-*.mjs` / `locate-syntax.mjs` / `probe-edge.mjs` / `mutation-test.mjs` / `repro-undo.mjs`
- 保持する本体: `.tooling/build.mjs`（构建）/ `.tooling/verify-refs.mjs`（参照整合）/
  `.tooling/verify-cases.mjs`（4D/5D 実走と被覆率）/ `.tooling/e2e-journeys.mjs`（12 旅程）/
  `.tooling/screenshots.mjs`（視覚証拠）/ `.tooling/verify*.mjs`（vendor 検証）/
  `.tooling/check-src.mjs`（構文）/ `.tooling/smoke.mjs`（vendor smoke）

移動数: 25
