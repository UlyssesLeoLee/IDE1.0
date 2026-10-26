//! 文档正文镜像 (ARCH-01: 文档状态上移 Rust 的第一步).
//!
//! 前端的行数组仍是编辑面 (按键 / 渲染都在那), 但**大文档的正文不再每次整篇发给 Rust**:
//! 第一次把整篇交上来登记 (`put`), 之后只交一个指纹 (`fingerprint`: 行数 + 两路 32 位哈希),
//! Rust 核对指纹一致才用自己手里那份 (`get`); 对不上 (前端改过 / 进程重启 / 被淘汰) 就返回"要整篇",
//! 前端补发。核对的是**内容指纹**而不是版本号, 所以前端有任何地方直接改了行数组也不会让 Rust 用到过期的正文
//! (置換全部会据此改文档, 用错正文 = 写坏文字)。
//!
//! 指纹算法与前端 `docFingerprint` 逐字相同: 按 UTF-16 码元逐个喂入, 每行末尾补一个码元 10。
//!   h1: FNV-1a 32 位 (h1 = (h1 ^ u) * 16777619, 初值 0x811C9DC5)
//!   h2: djb2 的 xor 变体 (h2 = (h2 * 33) ^ u, 初值 5381)

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// 同时保留的文档数上限 (多出的淘汰最久没用的)
const MAX_DOCS: usize = 64;

struct Entry {
    fp: [u32; 2],
    n: usize,
    lines: Arc<Vec<String>>,
    tick: u64,
}

#[derive(Default)]
struct Store {
    tick: u64,
    docs: HashMap<String, Entry>,
}

fn store() -> &'static Mutex<Store> {
    static S: OnceLock<Mutex<Store>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Store::default()))
}

/// 内容指纹 (与前端 docFingerprint 同一算法)
pub fn fingerprint(lines: &[String]) -> [u32; 2] {
    let (mut h1, mut h2) = (0x811C_9DC5u32, 5381u32);
    let mut feed = |u: u32| {
        h1 = (h1 ^ u).wrapping_mul(16_777_619);
        h2 = h2.wrapping_mul(33) ^ u;
    };
    for l in lines {
        for u in l.encode_utf16() {
            feed(u as u32);
        }
        feed(10);
    }
    [h1, h2]
}

/// 登记 (或覆盖) 一份文档的正文, 返回登记的那份
pub fn put(id: &str, lines: Vec<String>) -> Arc<Vec<String>> {
    let fp = fingerprint(&lines);
    let n = lines.len();
    let lines = Arc::new(lines);
    let mut s = store().lock().unwrap_or_else(|e| e.into_inner());
    s.tick += 1;
    let tick = s.tick;
    insert_capped(&mut s, id, Entry { fp, n, lines: lines.clone(), tick }, MAX_DOCS);
    lines
}

/// 放进去; 超过 max 份就淘汰最久没用的
fn insert_capped(s: &mut Store, id: &str, e: Entry, max: usize) {
    s.docs.insert(id.to_string(), e);
    while s.docs.len() > max {
        match s.docs.iter().min_by_key(|(_, e)| e.tick).map(|(k, _)| k.clone()) {
            Some(oldest) => {
                s.docs.remove(&oldest);
            }
            None => break,
        }
    }
}

/// 取一份文档的正文: 登记过且指纹、行数都对得上才给
pub fn get(id: &str, fp: [u32; 2], n: usize) -> Option<Arc<Vec<String>>> {
    let mut s = store().lock().unwrap_or_else(|e| e.into_inner());
    s.tick += 1;
    let tick = s.tick;
    let e = s.docs.get_mut(id)?;
    if e.fp != fp || e.n != n {
        return None;
    }
    e.tick = tick;
    Some(e.lines.clone())
}

/// 标签关闭: 丢掉这份文档
pub fn forget(id: &str) {
    store().lock().unwrap_or_else(|e| e.into_inner()).docs.remove(id);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn fingerprint_is_content_sensitive_and_utf16_based() {
        let a = fingerprint(&doc(&["abc", "def"]));
        assert_eq!(a, fingerprint(&doc(&["abc", "def"])));
        assert_ne!(a, fingerprint(&doc(&["abc", "deg"])));
        assert_ne!(a, fingerprint(&doc(&["abcdef"])), "行边界也算内容");
        assert_ne!(fingerprint(&doc(&["a", ""])), fingerprint(&doc(&["a"])), "末尾空行也算内容");
        // 非 BMP 字符按两个 UTF-16 码元喂入 (前端 charCodeAt 就是这样)
        let mut h1 = 0x811C_9DC5u32;
        for u in [0xD83Du32, 0xDE00, 10] {
            h1 = (h1 ^ u).wrapping_mul(16_777_619);
        }
        assert_eq!(fingerprint(&doc(&["😀"]))[0], h1);
        // 与前端 docFingerprint 的已知值核对 (用前端算法在 node 里算出来的; tests/uat/specs/doc-mirror.spec.ts 里是同一组输入)
        assert_eq!(a, [2_700_757_720, 821_783_810]);
        assert_eq!(fingerprint(&doc(&["日本語😀", "x"])), [2_129_295_297, 4_181_592_631]);
    }

    #[test]
    fn put_get_forget() {
        let d = doc(&["one", "two"]);
        let fp = fingerprint(&d);
        assert!(get("t-doc-a", fp, 2).is_none(), "没登记过");
        put("t-doc-a", d.clone());
        assert_eq!(get("t-doc-a", fp, 2).as_deref(), Some(&d));
        assert!(get("t-doc-a", [fp[0] ^ 1, fp[1]], 2).is_none(), "指纹对不上 = 前端改过");
        assert!(get("t-doc-a", fp, 3).is_none(), "行数对不上");
        forget("t-doc-a");
        assert!(get("t-doc-a", fp, 2).is_none());
    }

    #[test]
    fn oldest_doc_is_evicted() {
        // 用自己的 Store, 不碰进程里共用的那份 (别的测试并行在用)
        let mut st = Store::default();
        for i in 0..5u64 {
            insert_capped(&mut st, &format!("d{i}"), Entry { fp: [0, 0], n: 0, lines: Arc::new(vec![]), tick: i }, 3);
        }
        let mut ids: Vec<&String> = st.docs.keys().collect();
        ids.sort();
        assert_eq!(ids, ["d2", "d3", "d4"], "最早登记的两份被淘汰");
    }
}
