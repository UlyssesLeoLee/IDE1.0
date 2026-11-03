//! 脚本插件宿主: 用 QuickJS (rquickjs) 运行插件入口脚本, 语义对齐 Tauri 版的沙箱 iframe 宿主:
//! 只注入 `ide.register({...})`, 没有网络 / 文件 / DOM; 每次调用 15 秒墙钟超时、结果 2 MB 上限; 超时的插件整个丢弃 (下次调用重新加载)。
//!
//! **惰性**: 启动时什么都不创建 —— 第一次真的调用某个插件命令时才起工作线程, 第一次调用某个插件时才为它建 Runtime。
//! 每个插件各自一个 Runtime + Context (互相看不见); 全部在一条工作线程里, UI 线程只发消息、轮询回复, 永不阻塞。

use rquickjs::{CatchResultExt, Context, Ctx, Function, Promise, Runtime};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 单次调用的墙钟上限 (和 Tauri 版 CALL_TIMEOUT 一致)
pub const CALL_TIMEOUT: Duration = Duration::from_secs(15);
/// 结果序列化后的字节上限
pub const MAX_RESULT: usize = 2 * 1024 * 1024;
/// 单个插件的内存上限
const MEM_LIMIT: usize = 256 * 1024 * 1024;
/// QuickJS 的 JS 栈上限 (工作线程的栈比它大)
const JS_STACK: usize = 4 * 1024 * 1024;

/// 注入到每个插件里的引导脚本: `ide.register` + 统一的调用入口 (async 处理函数的 Promise 由宿主抽干)
const BOOT: &str = r#"
globalThis.__h = {};
globalThis.ide = { register(o) { Object.assign(globalThis.__h, o); } };
globalThis.console = { log() {}, info() {}, warn() {}, error() {}, debug() {} };
globalThis.__call = async (m, a) => {
  const f = globalThis.__h[m];
  if (typeof f !== "function") throw new Error("插件没有登记 " + m);
  const r = await f(...JSON.parse(a));
  return JSON.stringify(r === undefined ? null : r);
};
"#;

/// 一个插件的 JS 运行环境
pub struct Engine {
    _rt: Runtime,
    ctx: Context,
    /// 截止时刻 (相对 `base` 的毫秒); 中断处理器每隔一阵检查, 过了就杀掉当前脚本
    deadline: Arc<AtomicU64>,
    base: Instant,
    timeout: Duration,
}

impl Engine {
    /// 按顺序执行 `sources` (manifest.scripts 然后入口)。任何一个抛错 = 加载失败
    pub fn load(sources: &[(String, String)], timeout: Duration) -> Result<Self, String> {
        let rt = Runtime::new().map_err(|e| e.to_string())?;
        rt.set_memory_limit(MEM_LIMIT);
        rt.set_max_stack_size(JS_STACK);
        let base = Instant::now();
        let deadline = Arc::new(AtomicU64::new(u64::MAX));
        let d2 = deadline.clone();
        rt.set_interrupt_handler(Some(Box::new(move || base.elapsed().as_millis() as u64 > d2.load(Ordering::Relaxed))));
        let ctx = Context::full(&rt).map_err(|e| e.to_string())?;
        let me = Engine { _rt: rt, ctx, deadline, base, timeout };
        me.arm();
        me.ctx.with(|ctx| -> Result<(), String> {
            ctx.eval::<(), _>(BOOT).catch(&ctx).map_err(|e| e.to_string())?;
            for (name, src) in sources {
                ctx.eval::<(), _>(src.as_bytes().to_vec()).catch(&ctx).map_err(|e| format!("{name}: {}", fmt_err(&e.to_string())))?;
            }
            Ok(())
        })?;
        Ok(me)
    }

    fn arm(&self) {
        self.deadline.store(self.base.elapsed().as_millis() as u64 + self.timeout.as_millis() as u64, Ordering::Relaxed);
    }

    /// 调用已登记的处理函数: `args` 是 JSON 数组 (展开成参数), 返回结果的 JSON 文本
    pub fn call(&self, method: &str, args_json: &str) -> Result<String, String> {
        self.arm();
        let out = self.ctx.with(|ctx: Ctx| -> Result<String, String> {
            let f: Function = ctx.globals().get("__call").map_err(|e| e.to_string())?;
            let p: Promise = f.call((method, args_json)).catch(&ctx).map_err(|e| fmt_err(&e.to_string()))?;
            p.finish::<String>().catch(&ctx).map_err(|e| fmt_err(&e.to_string()))
        });
        self.deadline.store(u64::MAX, Ordering::Relaxed);
        let s = out?;
        if s.len() > MAX_RESULT {
            return Err(format!("插件结果过大 ({} 字节, 上限 {})", s.len(), MAX_RESULT));
        }
        Ok(s)
    }

    /// 上一次调用是不是被超时 / 内存限制打断的 (这样的环境不再复用)
    pub fn is_poisoned(msg: &str) -> bool {
        msg.contains("interrupted") || msg.contains("out of memory")
    }
}

fn fmt_err(s: &str) -> String {
    if s.contains("interrupted") {
        format!("插件执行超时 (超过 {} 秒), 已终止", CALL_TIMEOUT.as_secs())
    } else {
        s.trim().to_string()
    }
}

// ---------- 工作线程 ----------

enum Msg {
    Call { plugin: String, method: String, args: String, reply: Sender<Result<String, String>> },
    Unload(String),
}

/// 惰性宿主: `PluginHost::new` 只建通道 + 起线程, 在第一次 `call` 之前 App 根本不创建它
pub struct PluginHost {
    tx: Sender<Msg>,
}

impl PluginHost {
    pub fn new(root: PathBuf) -> Self {
        let (tx, rx) = channel::<Msg>();
        // 栈要比 JS_STACK 大, 不然 QuickJS 还没报栈溢出, 线程先爆了
        let _ = std::thread::Builder::new().name("plugin-host".into()).stack_size(JS_STACK * 2).spawn(move || worker(root, rx));
        PluginHost { tx }
    }

    /// 异步调用: 返回一个收件箱, UI 每帧 `try_recv`
    pub fn call(&self, plugin: &str, method: &str, args: &Value) -> Receiver<Result<String, String>> {
        let (reply, rx) = channel();
        let msg = Msg::Call { plugin: plugin.into(), method: method.into(), args: args.to_string(), reply: reply.clone() };
        if self.tx.send(msg).is_err() {
            let _ = reply.send(Err("插件宿主已停止".into()));
        }
        rx
    }

    /// 插件被禁用 / 删除: 丢掉它的运行环境
    pub fn unload(&self, plugin: &str) {
        let _ = self.tx.send(Msg::Unload(plugin.into()));
    }
}

fn worker(root: PathBuf, rx: Receiver<Msg>) {
    let mut engines: HashMap<String, Engine> = HashMap::new();
    while let Ok(msg) = rx.recv() {
        match msg {
            Msg::Unload(id) => {
                engines.remove(&id);
            }
            Msg::Call { plugin, method, args, reply } => {
                let res = (|| {
                    if !engines.contains_key(&plugin) {
                        let sources = sources_of(&root, &plugin)?;
                        engines.insert(plugin.clone(), Engine::load(&sources, CALL_TIMEOUT)?);
                    }
                    engines[&plugin].call(&method, &args)
                })();
                if let Err(e) = &res {
                    if Engine::is_poisoned(e) || e.contains("执行超时") {
                        engines.remove(&plugin);
                    }
                }
                let _ = reply.send(res);
            }
        }
    }
}

/// 读插件的脚本: manifest.scripts 然后入口; 插件必须已安装且启用
fn sources_of(root: &std::path::Path, id: &str) -> Result<Vec<(String, String)>, String> {
    use editcore::plugin_host::store;
    let p = store::list(root).into_iter().find(|p| p.manifest.id == id).ok_or_else(|| format!("没有安装插件 {id}"))?;
    if !p.enabled {
        return Err(format!("插件 {id} 已禁用"));
    }
    let mut out = vec![];
    for f in p.manifest.scripts.iter().chain(std::iter::once(&p.manifest.entry)) {
        let bytes = store::read_asset(root, id, f)?;
        out.push((f.clone(), String::from_utf8_lossy(&bytes).into_owned()));
    }
    Ok(out)
}

// ---------- 结果解析 ----------

/// 插件命令返回的行补丁 (行号 0 起, 和 Tauri 版 applyPluginEdit 同一契约)
#[derive(Debug, Clone, PartialEq)]
pub struct Patch {
    pub start: usize,
    pub remove: usize,
    pub insert: Vec<String>,
    pub cursor: Option<[usize; 2]>,
}

/// 命令结果的四种形状: 错误 / 补丁 / 仅摘要 / SVG 或 HTML (外加没有已知字段时的原始 JSON)
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Outcome {
    pub error: Option<String>,
    pub summary: String,
    pub svg: Option<String>,
    pub html: Option<String>,
    pub edit: Option<Patch>,
    /// 没有任何已知字段时, 把结果美化成文本显示
    pub raw: Option<String>,
}

pub fn parse_outcome(json: &str) -> Outcome {
    let v: Value = serde_json::from_str(json).unwrap_or(Value::Null);
    let Some(o) = v.as_object() else {
        return Outcome { raw: if v.is_null() { None } else { Some(pretty(&v)) }, ..Default::default() };
    };
    let s = |k: &str| o.get(k).and_then(Value::as_str).map(str::to_string);
    let mut out = Outcome { error: s("error"), summary: s("summary").unwrap_or_default(), svg: s("svg"), html: s("html"), ..Default::default() };
    out.edit = o.get("edit").and_then(|e| {
        let us = |k: &str| e.get(k).and_then(Value::as_u64).map(|n| n as usize);
        let insert: Option<Vec<String>> = e.get("insert")?.as_array()?.iter().map(|l| l.as_str().map(str::to_string)).collect();
        let cursor = e.get("cursor").and_then(|c| Some([c.get("row")?.as_u64()? as usize, c.get("col")?.as_u64()? as usize]));
        Some(Patch { start: us("start")?, remove: us("remove")?, insert: insert?, cursor })
    });
    let known = out.error.is_some() || !out.summary.is_empty() || out.svg.is_some() || out.html.is_some() || o.contains_key("edit");
    if !known {
        out.raw = Some(pretty(&v));
    }
    out
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_default()
}

/// 把 HTML 粗暴剥成纯文本 (没有 HTML 渲染器: 标签丢掉, 常见实体还原)
pub fn strip_html(h: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in h.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => {
                in_tag = false;
                if !out.ends_with('\n') {
                    out.push('\n');
                }
            }
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&amp;", "&").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine(src: &str) -> Engine {
        Engine::load(&[("t.js".into(), src.into())], Duration::from_millis(400)).expect("加载")
    }

    #[test]
    fn register_and_call_sync_and_async_handlers() {
        let e = engine(r#"ide.register({ "a": (x, y) => x + y, "b": async (o) => ({ n: o.k * 2 }) });"#);
        assert_eq!(e.call("a", "[2,3]").unwrap(), "5");
        assert_eq!(e.call("b", r#"[{"k":21}]"#).unwrap(), r#"{"n":42}"#);
        assert!(e.call("nope", "[]").unwrap_err().contains("nope"));
    }

    #[test]
    fn script_errors_are_reported_not_fatal() {
        let e = engine(r#"ide.register({ "boom": () => { throw new Error("坏了"); } });"#);
        assert!(e.call("boom", "[]").unwrap_err().contains("坏了"));
        assert_eq!(engine("ide.register({x:()=>1})").call("x", "[]").unwrap(), "1");
        assert!(Engine::load(&[("bad.js".into(), "this is not js".into())], CALL_TIMEOUT).is_err());
    }

    #[test]
    fn sandbox_has_no_io_globals() {
        let e = engine(r#"ide.register({ t: () => [typeof fetch, typeof XMLHttpRequest, typeof document, typeof window, typeof require, typeof process].join() });"#);
        assert_eq!(e.call("t", "[]").unwrap(), r#""undefined,undefined,undefined,undefined,undefined,undefined""#);
    }

    #[test]
    fn infinite_loop_is_interrupted_by_the_deadline() {
        let e = engine(r#"ide.register({ spin: () => { for (;;) {} } });"#);
        let t = Instant::now();
        let err = e.call("spin", "[]").unwrap_err();
        assert!(err.contains("超时"), "{err}");
        assert!(t.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn oversized_result_is_rejected() {
        let e = engine(r#"ide.register({ big: () => "x".repeat(3 * 1024 * 1024) });"#);
        assert!(e.call("big", "[]").unwrap_err().contains("过大"));
    }

    #[test]
    fn outcome_shapes_parse() {
        let o = parse_outcome(r#"{"summary":"ok","edit":{"start":1,"remove":2,"insert":["a","b"],"cursor":{"row":3,"col":4}}}"#);
        assert_eq!(o.edit, Some(Patch { start: 1, remove: 2, insert: vec!["a".into(), "b".into()], cursor: Some([3, 4]) }));
        assert_eq!(parse_outcome(r#"{"error":"炸"}"#).error.as_deref(), Some("炸"));
        assert_eq!(parse_outcome(r#"{"svg":"<svg/>","summary":"s"}"#).svg.as_deref(), Some("<svg/>"));
        assert!(parse_outcome(r#"{"headings":3}"#).raw.unwrap().contains("headings"));
        assert!(parse_outcome("null").raw.is_none());
        assert_eq!(parse_outcome(r#"{"summary":"x","edit":null}"#).edit, None);
        assert_eq!(strip_html("<p>a &amp; b</p><p>c</p>"), "a & b\n\nc".replace("\n\n", "\n"));
    }

    #[test]
    fn bundled_ppa_edits_a_virtual_doc() {
        let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../crates/editcore/plugins/ppa/main.js")).unwrap();
        let e = Engine::load(&[("main.js".into(), src)], CALL_TIMEOUT).unwrap();
        let input = serde_json::json!([{ "script": "var i: Integer; begin for i := 1 to 3 do S_InsText('x'); end;", "text": "abc", "cursor": {"row":0,"col":0} }]);
        let out = parse_outcome(&e.call("command:run", &input.to_string()).unwrap());
        assert!(out.error.is_none(), "{:?}", out.error);
        let p = out.edit.expect("有补丁");
        assert_eq!(p.insert, vec!["xxxabc".to_string()]);
        let bad = serde_json::json!([{ "script": "foo(", "text": "" }]);
        assert!(parse_outcome(&e.call("command:run", &bad.to_string()).unwrap()).error.is_some());
    }

    #[test]
    fn bundled_doc_graph_renders_svg_with_data_lines() {
        let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../crates/editcore/plugins/doc-graph/main.js")).unwrap();
        let e = Engine::load(&[("main.js".into(), src)], CALL_TIMEOUT).unwrap();
        let input = serde_json::json!([{ "text": "# A\n## B\ntext [[A]]\n" }]);
        let out = parse_outcome(&e.call("command:show", &input.to_string()).unwrap());
        let svg = out.svg.expect("svg");
        assert!(svg.contains("data-line=\"2\""));
        assert!(out.summary.contains("2 个标题"));
        let empty = serde_json::json!([{ "text": "plain" }]);
        assert!(parse_outcome(&e.call("command:show", &empty.to_string()).unwrap()).error.is_some());
    }

    #[test]
    fn pele_bundle_loads_and_renders_mermaid_to_svg() {
        // pele 是纯 JS 渲染器 (730 KB), QuickJS 里无需 DOM 就能出 SVG; 预览里的 fence 接入留待后续
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../crates/editcore/plugins/pele/");
        let a = std::fs::read_to_string(format!("{dir}pele.global.js")).unwrap();
        let b = std::fs::read_to_string(format!("{dir}main.js")).unwrap();
        let e = Engine::load(&[("pele.global.js".into(), a), ("main.js".into(), b)], Duration::from_secs(20)).unwrap();
        let out = e.call("render", &serde_json::json!(["graph TD
 A-->B"]).to_string()).unwrap();
        assert!(parse_outcome(&out).svg.is_some_and(|s| s.starts_with("<svg")));
    }

    #[test]
    fn host_thread_loads_bundled_plugin_lazily_and_honors_disable() {
        let _g = crate::i18n::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("ide-fast-host-{}", std::process::id()));
        std::env::set_var("IDE_PLUGIN_DIR", &dir);
        let root = crate::plugins::root();
        let _ = editcore::plugin_host::store::list(&root); // 触发内置插件落盘
        let host = PluginHost::new(root.clone());
        let rx = host.call("doc-graph", "command:show", &serde_json::json!([{ "text": "# T\n" }]));
        let out = rx.recv_timeout(Duration::from_secs(20)).unwrap().unwrap();
        assert!(out.contains("svg"));
        let none = host.call("nonexistent", "command:x", &serde_json::json!([])).recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(none.is_err());
        editcore::plugin_host::store::set_enabled(&root, "doc-graph", false).unwrap();
        host.unload("doc-graph");
        let off = host.call("doc-graph", "command:show", &serde_json::json!([{ "text": "# T\n" }])).recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(off.unwrap_err().contains("禁用"));
        std::env::remove_var("IDE_PLUGIN_DIR");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
