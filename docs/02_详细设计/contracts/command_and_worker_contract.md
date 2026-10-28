# IF-04 ～ IF-08 命令/插件/预览消息契约（设计草案）

## A. CommandContext / CommandResult

```json
{
  "requestId":"uuid-v4", "commandId":"md.text.bold", "origin":"palette",
  "document":{"sessionId":"ws1:buf8:file:///project/a.md","expectedVersion":12},
  "selection":{"start":102,"end":110,"unit":"host-native"},
  "parameters":{},"permissions":["document.write.active"]
}
```

结果：`{ok, reason?, nextVersion?, undoId?, diagnostics?}`，禁止返回 `ok:true` 但实际宿主 EditTransaction 未提交的状态。

## B. PatchProposal（Plugin/AI 只读请求返回）

```json
{
  "proposalId":"uuid-v4", "expectedVersion":12,
  "scope":{"documentSessionId":"ws1:buf8:file:///project/a.md", "allowedRanges":[[100,200]]},
  "edits":[{"start":110,"end":110,"insertText":"**"}],
  "justification":"对已授权选区执行格式操作",
  "requiresUserReview":true
}
```

校验：会话与版本相等；所有范围合法且在授权区域内；不重叠；补丁数量与总文本长度符合配额；预期新文本不会触发未经确认的编码变更。AI/P1默认先审核再提交。

## C. Plugin Worker IPC

- 协议候选：本地命名管道 + length-prefixed JSON，消息自带 `requestId`、`apiVersion`、超时、取消标识。
- IPC 最大单条消息设计默认1MiB，超额需走受控分页/句柄；最终参数待POC。
- Worker 不接受宿主 Buffer 指针、不允许直接写文档文件。所有编辑以 `PatchProposal` 回流到 CommandBus。
- 插件权限是**系统边界**：第三方无信任插件采用 AppContainer/限制令牌等可验证 OS 隔离；只有逻辑检查但仍运行完全可信用户权限的 DLL 不可视为真正安全隔离。
- 无可验证 OS 约束的主机须拒绝执行需网络/进程/任意文件权限的不可信插件，或明确采用仅可信插件模式并向用户说明。
- 超时/崩溃只影响当前请求，CommandBus 取消事务，且结果不得应用部分补丁。

## D. PreviewPort

输入：`{sessionId,version,sanitizedHtml,sourceMap,theme,remotePolicy}`；输出：`{ready,renderedVersion,errors,scrollAnchor}`。

仅允许受控本地资源源，禁止导航到任意 `file://`，CSP 默认 `default-src 'none'` 并对字体/图片/样式显式收敛授权。Mermaid 的可信捆绑脚本在**独立隔离iframe/文档环境**执行，采用消息白名单，并持续禁止未经授权网络外发与宿主对象暴露。

## E. Reference/Export

链接解析返回 `{targetUri,targetHeading,exists,isWithinWorkspace,needsPermission}`。HTML/PDF/DOCX 导出应返回 `{ok,outputUri,warnings:[...],sourceVersion}`；导出永远不能改变正文。
