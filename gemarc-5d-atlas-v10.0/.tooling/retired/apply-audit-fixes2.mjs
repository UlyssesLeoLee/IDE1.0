// 追加审核修复：状态机终态/初态 + 新增 evidence 条目（EV-079..EV-082）
import fs from "node:fs";
import path from "node:path";

const ROOT = path.resolve(process.argv[2] || ".");
const M = path.join(ROOT, "models");
const read = (n) => JSON.parse(fs.readFileSync(path.join(M, n), "utf8"));
const write = (n, o) => fs.writeFileSync(path.join(M, n), JSON.stringify(o, null, 2) + "\n", "utf8");
const log = (...a) => console.log(...a);

// ---- 1) 状态机初态 / 终态修正 ----
{
  const r = read("causal-rules.json");
  const sm = Object.fromEntries(r.stateMachines.map((s) => [s.nodeId, s]));

  // 同步节点激活即 Running（原为 Idle，导致 when=="Running" 永不满足）
  if (sm["N-REMOTE-SYNC"]) {
    sm["N-REMOTE-SYNC"].initial = "Running";
    log("SM-REMOTE-SYNC initial -> Running");
  }
  // 脱敏节点 Completed 后还要把请求送进 provider，因此 Completed 不是终态
  if (sm["N-AI-SANITIZE"]) {
    sm["N-AI-SANITIZE"].terminalStates = ["Failed"];
    log("SM-AI-SANITIZE terminalStates -> [Failed]（Completed 后仍需发出请求）");
  }
  // 案例侧显式声明入口状态
  const c = read("cases.json");
  const byId = Object.fromEntries(c.cases.map((x) => [x.id, x]));
  for (const id of ["CASE-SYNC-NOT-FAST-FORWARD", "CASE-SYNC-FAST-FORWARD-PUSH"]) {
    if (byId[id]) { byId[id].entryState = "Running"; log(id + " entryState -> Running"); }
  }
  write("cases.json", c);
  write("causal-rules.json", r);
}

// ---- 2) 追加 evidence（EV-079..EV-082），与子代理新增的 EV-078 不冲突 ----
{
  const e = read("evidence.json");
  const have = new Set(e.evidence.map((x) => x.id));
  const NEW = [
    {
      id: "EV-079", kind: "SourceCode", status: "Implemented", sourceRevisionId: "REV-MAIN",
      summary: "默认裸库目录常量 DEFAULT_REPOS_DIR = \"repos\"；默认凭据根目录常量 DEFAULT_VAULT_FILE_ROOT = \".gitgit/vault\"",
      path: "src/config.rs", startLine: 36, endLine: 50, revision: "4df73d9"
    },
    {
      id: "EV-080", kind: "SourceCode", status: "Implemented", sourceRevisionId: "REV-MAIN",
      summary: "Config 的 repos_dir: PathBuf 与 vault_file_root: PathBuf 两个字段决定运行时存储位置",
      path: "src/config.rs", startLine: 106, endLine: 117, revision: "4df73d9"
    },
    {
      id: "EV-081", kind: "SourceCode", status: "Implemented", sourceRevisionId: "REV-MAIN",
      summary: "pub trait VersionedVault —— 版本化凭据存储的抽象边界，FileVault 与 MinioVault 分别实现",
      path: "src/server/vault_versioned.rs", startLine: 185, endLine: 233, revision: "4df73d9"
    },
    {
      id: "EV-082", kind: "SourceCode", status: "Implemented", sourceRevisionId: "REV-MAIN",
      summary: "AI diff 模块：pub struct DiffRequest 及其 git 调用构造（整文件 269 行）",
      path: "src/ai/diff.rs", startLine: 1, endLine: 60, revision: "4df73d9"
    }
  ];
  for (const n of NEW) {
    if (have.has(n.id)) { log("skip (exists): " + n.id); continue; }
    e.evidence.push(n);
    log("+ evidence " + n.id);
  }
  write("evidence.json", e);
}

// ---- 3) 校验：所有 evidenceIds 引用都能解析 ----
{
  const arch = read("architecture.json");
  const ev = read("evidence.json");
  const ids = new Set(ev.evidence.map((x) => x.id));
  const dangling = [];
  const check = (owner, list) => (list || []).forEach((i) => { if (!ids.has(i)) dangling.push(owner + " -> " + i); });
  arch.nodes.forEach((n) => check("node " + n.id, n.evidenceIds));
  arch.edges.forEach((x) => check("edge " + x.id, x.evidenceIds));
  log("悬空 evidence 引用: " + (dangling.length ? dangling.join(", ") : "无"));
}
