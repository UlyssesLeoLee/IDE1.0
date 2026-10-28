# IF-01 ～ IF-03 宿主接口契约（设计草案）

本文件是宿主适配**规范**，不是 Sakura Editor 已存在的真实 API。Stage 0 必须核查原生接口能否实现。

```typescript
interface DocumentHandle {
  sessionId: string;      // workspaceId + hostBufferId + normalizedURI
  workspaceId: string;
  bufferId: string;
  uri: string;
  version: number;        // 每次宿主Buffer成功改变后递增
  dirty: boolean;
}
interface TextSnapshot {
  handle: DocumentHandle;
  text: string;           // 仅运行内存文本，不改写文件
  positionEncoding: 'utf8-byte' | 'utf16-unit' | 'unicode-scalar' | 'host-native';
  fileEncoding: string;   // utf-8/sjis... 原始编码声明
  hasBom: boolean;
  eol: 'LF' | 'CRLF' | 'CR' | 'MIXED';
  diskFingerprint: string; // 原磁盘版本标识，保存冲突检测
}
interface RangeEdit {
  start: number;          // Host Adapter 公布的单位，不猜测
  end: number;            // 半开区间 [start,end)
  insertText: string;
}
interface EditTransaction {
  commandId: string;
  expectedVersion: number;
  edits: RangeEdit[];     // 同一快照，互不重叠、降序提交
  selectionAfter?: {start: number; end: number};
  intent: 'user'|'plugin'|'ai';
}
type ApplyResult =
  | {ok:true; nextVersion:number; undoId:string}
  | {ok:false; reason:'CONFLICT'|'READ_ONLY'|'INVALID_RANGE'|'IME_ACTIVE'|'PERMISSION'|'IO_ERROR'};
interface IEditorHost {
  capabilities(): Set<string>;
  currentDocument(): DocumentHandle | null;
  snapshot(handle:DocumentHandle): TextSnapshot;
  applyAtomic(handle:DocumentHandle, tx:EditTransaction): ApplyResult;
  getSelection(handle:DocumentHandle): {start:number;end:number};
  isIMEComposing(handle:DocumentHandle): boolean;
  onDocumentEvent(callback:(event:unknown)=>void): Unsubscribe;
  registerCommand(id:string, callback:(args:unknown)=>void): Unsubscribe;
  registerPanel(id:string, factory:()=>unknown): Unsubscribe;
}
```

## 强制契约

1. 宿主 EditTransaction 的语义必须**全成功或全失败**；若原生 API 不支持，适配层必须在不破坏 Undo 的条件下实现回滚；无法实现时不支持会产生多笔修改的命令。
2. `expectedVersion` 必须在事务提交时重新核验；解析线程不得持有可跨版本写回的旧位置。
3. 外部文件保存必须由宿主原有编码器和冲突判定负责；插件不可直接写同一源文件。
4. 对多字节字符不允许仅靠 `column*2` 等假设推断位置。适配层提供单调、双向可验证的坐标转换及无效边界拒绝。
5. 在 IME composition 中禁用改变候选状态的工具激活；不得拦截 IME 键事件。
6. 事务结果必须包括 `undoId` 或等效确认，以便 UT 验证一次逻辑操作的撤销粒度。

## 预备兼容性清单

- 工作区/Worktree身份能否获得？
- 底层 Edit API 的offset单位和行/列计数语义是什么？
- 一次API是否支持多个非相邻RangeEdit？
- 是否能监听 `documentChanged`, `saved`, `closed`, `externallyChanged`？
- 是否支持无焦点抢夺的浮动面板及 DPI？
- 文件编码/BOM/EOL是否可无损访问，尤其 Shift-JIS？
- 插件能否在独立进程并通过IPC调用命令？

以上问题未验证前，绝不能声明实际 Sakura Editor 可直接接入该 API。
