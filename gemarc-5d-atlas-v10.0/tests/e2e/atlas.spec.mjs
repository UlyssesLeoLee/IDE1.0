/**
 * tests/e2e/atlas.spec.mjs — file:// 実ブラウザ E2E（12 旅程 / 55 実測）
 *
 * 実体は .tooling/e2e-journeys.mjs に置かれ、ここは門禁の入口として同じ検査を実行する。
 * 実行: node tests/e2e/atlas.spec.mjs   （または node .tooling/e2e-journeys.mjs）
 *
 * 注意: 旧版は別プロジェクト前提の検証（タイトル文字列・別フォーマット名など）を含み
 * そのままでは失敗する。対象プロジェクトの前提に依存しない検査へ更新済み。
 */
import "../../.tooling/e2e-journeys.mjs";
