# IDE1.0 Web — Agent Router & Worker 测试设计与追溯矩阵书

**文档编号**: TEST-ROUTER-2026-V2.0  
**制定组织**: IDE1.0 内核与云架构委员会 / 日本 IPA 软件工程基准推进组  
**关联工件**: `docs/03_测试资料/UT-IT-ST-UAT-Web-Agent-Router.xlsx`  
**基准要件**: [Web-Agent-Router-Requirements.md](../00_要件基线/Web-Agent-Router-Requirements.md)

---

## 1. 测试体系设计与 IPA 追溯

本测试集全面覆盖日本 IPA 推荐的 V 字形验证模型，分为四个递进层级：
1. **UT (Unit Test)**: 覆盖语义解析器（`parser`）、指纹计算（`identity`）、AST 锚点重定位（`patch`）与 CAS 协调器基础逻辑。
2. **IT (Integration Test)**: 模拟多 Worker、分布式租约过期抢占、Fencing Token 拦截与 PostgreSQL 事务边界。
3. **ST (System & Load Test)**: 验证 100/1000 级高并发 Patch 合入吞吐与热点文件退避。
4. **UAT (User Acceptance Test)**: Vercel 生产环境访问、无头 Agent 纯 API 协作与 Tauri 桌面端双端一致性验证。

---

## 2. 详细测试用例清单

### 2.1 UT (单体测试)
- `UT-SEM-01`: Rust 多函数与 impl 块抽取，验证 qualified_path。
- `UT-SEM-02`: Python 函数与类方法提取，验证代码跨度计算。
- `UT-SEM-03`: TypeScript 接口、函数与类提取。
- `UT-SEM-04`: 稳定身份哈希在相同内容下的幂等性。
- `UT-SEM-05`: 上方增删注释/空行时，目标函数的指纹保持抗漂移。
- `UT-PATCH-01`: 前序代码行数变化后，后续函数 AST 锚点重定位算法。
- `UT-COORD-01`: CAS 单调递增版本号检验。
- `UT-COORD-02`: 过期 Fencing Token 拦截（返回 412）。

### 2.2 IT (结合测试)
- `IT-CONC-01`: 同文件不同函数双 Agent 并发提交，自动重定位并顺序合入。
- `IT-CONC-02`: 同函数双 Agent 并发写冲突拦截，返回 409 SEMANTIC_WRITE_CONFLICT。
- `IT-LEASE-01`: Worker 崩溃超时后租约自动回收，任务被重入队。
- `IT-OUTBOX-01`: 事务边界验证：文件推进与 outbox_events 在同一 DB 事务。
- `IT-IDEMP-01`: 携带相同 Idempotency-Key 重复提交时幂等防护。

### 2.3 ST (系统与负载测试)
- `ST-LOAD-01`: 100 个非相交 Patch 连续高频提交，数据无损。
- `ST-LOAD-02`: 1000 个任务批量入队与队列排队调度稳定性。
- `ST-FAIL-01`: 20 个 Worker 争抢同一热点文件时的 CAS 退避与一致性保障。

### 2.4 UAT (受入测试)
- `UAT-VERCEL-01`: Vercel 部署域名公网可访问，Web Workbench 正常渲染。
- `UAT-API-01`: 无头 Agent 通过 HTTP API 完成全流程作业。
- `UAT-DESK-01`: Tauri 桌面端与 Web 端共用编辑语义与测试全绿。

---
**确认人**: 
- 测试总监: Mavis (Test Lead)
- IPA 评审官: Approved 2026-11-05
