# IDE1.0 Mock Markdown Preview Fixture

## 1. 任务清单与就地编辑测试
- [x] 支持 **Markdown 星号加粗** 与 __双下划线加粗__ 渲染
- [x] 支持 Myers Diff 无损行级补丁预览
- [ ] 待测条目：双栏协同与桌面便笺同步

## 2. 功能矩阵示例表

| 模块 | 状态 | 验证方式 |
| :--- | :---: | :--- |
| 核心内核 | Ready | `cargo test -j 1 -p editcore` |
| editcore | Ready | `cargo test -j 1 -p editcore` |
| nema-codec | Ready | `cargo test -j 1 -p nema-codec` |
| ide-fast | Ready | `cargo test -j 1 -p ide-fast` |

## 3. 代码块示例
```rust
pub fn mock_health() -> &'static str {
    "IDE1.0_MOCK_OK"
}
```
