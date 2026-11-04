// .tooling/vitest.config.mjs — 单元测试配置（root = 图集根）
import { defineConfig } from "vitest/config";
import path from "node:path";
import { fileURLToPath } from "node:url";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, "..");

export default defineConfig({
  root: ROOT,
  test: {
    include: ["tests/unit/**/*.test.mjs"],
    environment: "node",
    reporters: ["default"],
    globals: false
  }
});
