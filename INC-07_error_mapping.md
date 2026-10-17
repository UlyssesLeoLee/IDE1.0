# INC-07 Error Code Mapping Table - 完了 ✅

## 状态: 100% 完成

### 替换结果

| 文档 | 旧 Code | 替换后 |
|------|---------|--------|
| DD-01 | ~60 | ✅ 0 残留 |
| DD-02 | 25 | ✅ 0 残留 |
| DD-04 | 0 | ✅ 已统一 |

### 映射规则

| 旧 Namespace | 新 Namespace | 语义 |
|--------------|-------------|------|
| ERR-CB-XXX | ERR-BIZ / ERR-AUTHZ / ERR-EXT / ERR-VAL / ERR-SYS | Command Bus |
| ERR-EB-XXX | ERR-DB / ERR-VAL / ERR-BIZ / ERR-SYS | Event Bus |
| ERR-CR-XXX | ERR-SYS / ERR-AUTHZ / ERR-VAL | Capability Registry |
| ERR-BE-XXX | ERR-BIZ / ERR-VAL / ERR-AUTHZ / ERR-DB | Buffer Engine |

### 自审结果
- `search_files ERR-CB-|ERR-EB-|ERR-CR-|ERR-BE-` 在工作区返回 5 个结果
- 这 5 个结果全部来自 `INC-07_error_mapping.md`（映射表文档本身，记录旧代码）

### 修改文件列表
1. `DD-01_Microkernel_Core詳細設計書.md` - 60+ 处更新
2. `DD-02_Session_Transaction_Buffer_Engine詳細設計書.md` - 25+ 处更新
3. `INC-07_error_mapping.md` - 新建映射表
