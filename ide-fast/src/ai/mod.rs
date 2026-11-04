//! AI 对话的与界面无关的部分, 对标 Tauri 版 (editor.html 的 aiConfig / chatCallModel / chatInvokeTool), 但这里是流式的。
//! - `config`: 配置、端点校验、脱敏; `prompt`: 上下文与请求体; `sse`: 流解析; `tools`: 工具表与往返; `transport`: curl 传输。
//! 约束:
//! - 只在用户点「发送」时联网; 端点只许 https (本机回环地址允许 http, 给本地模型用);
//! - 密钥只写进 `ai.json` (和 settings.json 同目录, 不进导出的配置), 不进日志、不进命令行;
//! - 有连接 / 总时长 / 低速超时和响应字节上限; 取消 = 杀掉 curl 子进程。

mod config;
mod efforts;
mod prompt;
mod sse;
mod tools;
mod transport;

pub use config::*;
pub use efforts::*;
pub use prompt::*;
pub use sse::*;
pub use tools::*;
pub use transport::*;
