# IDE1.0 Dedicated E2E Mock Project

> 本工程作为 IDE1.0 专属的独立标准测试空间，用于端到端验证多 Agent 并发作业、AST 补丁提交、Git/NEMA 双模式协作及数学不变量。

## 目录索引
- `src/`：Rust 源码，覆盖线性可验证函数与服务状态
- `knowledge/`：NEMA v1.5 格式的结构化数据，提供 5W1H 与 Table 域，供红绿语法校验与 AST 语义合并
- `config/`：环境与模型配置，用于隔离沙箱测试
- `docs/`：Markdown 技术规格，用于 Myers diff 增减行与阅读测试
- `scripts/`：Blender bpy 风格 `ide::*` 控制脚本
