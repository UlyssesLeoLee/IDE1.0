//! 推理强度 (effort) 的档位: 默认档位 / 手填的自定义档位 / 从供应商的模型接口获取的档位。
//! 对标 Tauri 版 editor.html 的 `AI_EFFORTS_DEFAULT` / `extractEfforts` / `aiFetchEfforts`:
//! 档位不写死 —— 优先用获取到的 (或手填的), 都没有才用通用默认; 请求里只有选了档位才带 `reasoning_effort`。

use super::config::{checked_base, AiConfig};
use super::transport::get_json;
use serde_json::{json, Value};

/// 没有获取到 / 没有手填时用的通用档位
pub const DEFAULT: [&str; 4] = ["minimal", "low", "medium", "high"];

/// 已知档位的强弱顺序 (排序用; 不认识的排在后面)
const ORDER: [&str; 7] = ["none", "minimal", "low", "medium", "high", "xhigh", "max"];

/// 一个档位名是否合法: 1 到 24 个字符, 只含字母数字和 `_ . -` (它会原样写进请求体, 所以不信任何来源)
pub fn valid(x: &str) -> bool {
    (1..=24).contains(&x.len()) && x.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

/// 清洗一组档位: 不合法的丢掉, 去重 (保持顺序), 最多 12 个
pub fn clean_list(list: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for x in list {
        if valid(&x) && !out.contains(&x) {
            out.push(x);
        }
        if out.len() >= 12 {
            break;
        }
    }
    out
}

/// 手填框里的文字 (逗号分隔) -> 档位列表
pub fn parse_custom(text: &str) -> Vec<String> {
    clean_list(text.split([',', '，', ';', '、']).map(|s| s.trim().to_string()).collect())
}

/// 界面里能选的档位: 获取到 / 手填的, 没有就用默认
pub fn options(cfg: &AiConfig) -> Vec<String> {
    if cfg.efforts.is_empty() { DEFAULT.iter().map(|s| s.to_string()).collect() } else { cfg.efforts.clone() }
}

fn rank(x: &str) -> usize {
    ORDER.iter().position(|o| *o == x).unwrap_or(99)
}

/// 从模型接口的返回里找 effort 档位。供应商写法各不相同, 所以按形状找而不是认某一家:
/// - 键名含 effort 的数组: `["low","medium","high"]`
/// - 键名含 effort 的对象: `{ low: {supported:true}, high: {supported:false} }` -> 取 supported 的; 或 `{ levels: [...] }` / `enum` / `values` / `supported_values` / `options`
///
/// 找不到 = 空 (界面再提示手填)。结果去重, 按强弱排序 (同强弱按名字)。
pub fn extract(node: &Value) -> Vec<String> {
    fn walk(node: &Value, depth: usize, out: &mut Vec<String>) {
        if depth > 8 {
            return;
        }
        let Some(obj) = node.as_object() else {
            if let Some(a) = node.as_array() {
                for v in a { walk(v, depth + 1, out); }
            }
            return;
        };
        let strs = |a: &Vec<Value>| a.iter().filter_map(|x| x.as_str()).filter(|s| valid(s)).map(str::to_string).collect::<Vec<_>>();
        for (k, v) in obj {
            if k.to_ascii_lowercase().contains("effort") {
                if let Some(a) = v.as_array() {
                    out.extend(strs(a));
                } else if let Some(o) = v.as_object() {
                    for key in ["levels", "enum", "values", "supported_values", "options"] {
                        if let Some(a) = o.get(key).and_then(Value::as_array) { out.extend(strs(a)); }
                    }
                    for (lk, lv) in o {
                        if lv.get("supported").and_then(Value::as_bool) == Some(true) && valid(lk) { out.push(lk.clone()); }
                    }
                }
            }
            if v.is_object() || v.is_array() {
                walk(v, depth + 1, out);
            }
        }
    }
    let mut out = vec![];
    walk(node, 0, &mut out);
    let mut uniq: Vec<String> = vec![];
    for x in out {
        if !uniq.contains(&x) { uniq.push(x); }
    }
    uniq.sort_by(|a, b| rank(a).cmp(&rank(b)).then_with(|| a.cmp(b)));
    uniq
}

/// 在一次接口响应里取这个模型的档位: 是列表接口 (`data` / `models`) 时先找这个模型自己那一项; 列表里没有这个模型就报错
/// (不能把别的模型的档位并起来当成它的); 没有指定模型 / 不是列表时看整份响应
pub fn pick(j: &Value, model: &str) -> Result<Vec<String>, String> {
    let list = j.get("data").and_then(Value::as_array).or_else(|| j.get("models").and_then(Value::as_array));
    let mine = match (list, model.is_empty()) {
        (Some(l), false) => l.iter().find(|m| m.get("id").and_then(Value::as_str) == Some(model) || m.get("name").and_then(Value::as_str) == Some(model)),
        _ => None,
    };
    if list.is_some() && !model.is_empty() && mine.is_none() {
        return Err(format!("接口列表里没有模型 {model}"));
    }
    Ok(match (mine, list) {
        (Some(m), _) => extract(m),
        (None, Some(l)) => extract(&json!({ "data": l })),
        (None, None) => extract(j),
    })
}

/// encodeURIComponent: 字母数字和 `- _ . ! ~ * ' ( )` 原样, 其余按 UTF-8 字节写成 %XX
fn pct(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// 要查的模型接口地址, 按顺序试: 先查这个模型自己的 (`/models/<模型>`), 再查整个列表 (`/models`)
pub fn models_urls(base: &str, model: &str) -> Vec<String> {
    let base = base.trim_end_matches('/');
    if model.is_empty() { vec![format!("{base}/models")] } else { vec![format!("{base}/models/{}", pct(model)), format!("{base}/models")] }
}

/// 向供应商的模型接口要这个模型的 effort 档位 (用户点「从供应商获取」才联网; 密钥只发给用户自己配置的接口)
pub fn fetch_efforts(cfg: &AiConfig) -> Result<Vec<String>, String> {
    if cfg.base_url.trim().is_empty() || cfg.key.is_empty() {
        return Err("先填好接口地址和 API Key".into());
    }
    let base = checked_base(&cfg.base_url)?;
    let mut last = String::new();
    for u in models_urls(&base, cfg.model.trim()) {
        match get_json(cfg, &u) {
            Ok((st, j)) if (200..300).contains(&st) => match pick(&j, cfg.model.trim()) {
                Ok(f) if !f.is_empty() => return Ok(f),
                Ok(_) => {}
                Err(e) => last = e,
            },
            Ok((st, _)) => last = format!("HTTP {st}"),
            Err(e) => last = e,
        }
    }
    Err(format!("供应商的模型接口没有公布 effort 档位{}, 可以在下面手动填写", if last.is_empty() { String::new() } else { format!(" ({last})") }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn extract_finds_levels_in_every_known_shape_and_orders_them_by_strength() {
        assert_eq!(extract(&json!({ "model": { "reasoning_effort": ["high", "low", "minimal"] } })), ["minimal", "low", "high"], "键名含 effort 的数组, 按强弱排序");
        assert_eq!(extract(&json!({ "x": { "effort_levels": { "low": { "supported": true }, "high": { "supported": false }, "medium": { "supported": true } } } })), ["low", "medium"], "只取 supported 的");
        assert_eq!(extract(&json!({ "reasoning_effort": { "levels": ["medium", "xhigh"] } })), ["medium", "xhigh"]);
        assert_eq!(extract(&json!({ "a": [ { "Reasoning_Effort": { "enum": ["low"], "values": ["max", "low"] } } ] })), ["low", "max"], "嵌在数组里 / 键名不分大小写 / 去重");
        assert_eq!(extract(&json!({ "effort": ["zeta", "alpha", "low"] })), ["low", "alpha", "zeta"], "不认识的排在已知档位后面, 彼此按名字");
        assert!(extract(&json!({ "context_length": 8192, "name": "m" })).is_empty());
    }

    #[test]
    fn extract_drops_unsafe_names_and_stops_at_depth() {
        let bad = json!({ "effort": ["ok", "has space", "semi;colon", "", "a".repeat(25), 5, null, "fine_2.1-x"] });
        assert_eq!(extract(&bad), ["fine_2.1-x", "ok"], "非法名字 / 非字符串都丢掉");
        let mut deep = json!({ "effort": ["low"] });
        for _ in 0..10 { deep = json!({ "n": deep }); }
        assert!(extract(&deep).is_empty(), "嵌套超过 8 层不再往下找");
        let mut ok = json!({ "effort": ["low"] });
        for _ in 0..8 { ok = json!({ "n": ok }); }
        assert_eq!(extract(&ok), ["low"], "8 层以内能找到");
    }

    #[test]
    fn pick_prefers_the_models_own_entry_and_refuses_to_merge_others() {
        let list = json!({ "data": [
            { "id": "a", "reasoning_effort": ["low"] },
            { "id": "b", "name": "bee", "reasoning_effort": ["minimal", "high"] },
        ] });
        assert_eq!(pick(&list, "b").unwrap(), ["minimal", "high"], "按 id 找");
        assert_eq!(pick(&list, "bee").unwrap(), ["minimal", "high"], "按 name 找");
        assert_eq!(pick(&list, "").unwrap(), ["minimal", "low", "high"], "没指定模型: 看整张列表");
        assert_eq!(pick(&list, "zzz").unwrap_err(), "接口列表里没有模型 zzz", "列表里没有这个模型: 报错, 不把别的模型的档位并进来");
        assert_eq!(pick(&json!({ "models": [ { "name": "m", "effort": ["low"] } ] }), "m").unwrap(), ["low"], "`models` 键也认");
        assert_eq!(pick(&json!({ "id": "m", "reasoning_effort": ["high"] }), "m").unwrap(), ["high"], "单个模型的响应 (不是列表)");
        assert!(pick(&json!({ "data": [ { "id": "m" } ] }), "m").unwrap().is_empty(), "这个模型没公布档位: 空");
    }

    #[test]
    fn urls_try_the_model_first_and_encode_it() {
        assert_eq!(models_urls("https://h/v1/", "gpt 5/x"), ["https://h/v1/models/gpt%205%2Fx", "https://h/v1/models"]);
        assert_eq!(models_urls("https://h/v1", ""), ["https://h/v1/models"]);
        assert_eq!(models_urls("https://h/v1", "日"), ["https://h/v1/models/%E6%97%A5", "https://h/v1/models"], "按 UTF-8 字节编码");
        assert_eq!(pct("a-b_c.d~e*f'g(h)!"), "a-b_c.d~e*f'g(h)!");
    }

    #[test]
    fn custom_levels_are_parsed_cleaned_and_capped() {
        assert_eq!(parse_custom(" low, High ,,low，xhigh; bad val , a'b"), ["low", "High", "xhigh"], "逗号 / 全角逗号 / 分号分隔, 去空白去重去非法");
        let many = (0..30).map(|i| format!("l{i}")).collect::<Vec<_>>().join(",");
        assert_eq!(parse_custom(&many).len(), 12, "最多 12 个");
        assert!(parse_custom("").is_empty());
        let mut c = AiConfig::default();
        assert_eq!(options(&c), DEFAULT, "没有获取到 / 手填的: 用通用默认");
        c.efforts = v(&["low", "max"]);
        assert_eq!(options(&c), ["low", "max"]);
    }

    #[test]
    fn config_roundtrip_keeps_clean_levels_only() {
        let mut c = AiConfig::default_cfg();
        c.efforts = v(&["low", "high"]);
        let back = AiConfig::from_json(&c.to_json());
        assert_eq!(back.efforts, ["low", "high"]);
        let hostile = AiConfig::from_json(&json!({ "baseUrl": "https://x/v1", "efforts": ["ok", "no way!", 7, "ok"] }));
        assert_eq!(hostile.efforts, ["ok"], "ai.json 里的档位按不可信数据清洗");
        assert!(AiConfig::from_json(&json!({})).efforts.is_empty());
    }

    #[test]
    fn fetch_needs_a_base_and_a_key_before_touching_the_network() {
        assert_eq!(fetch_efforts(&AiConfig::default()).unwrap_err(), "先填好接口地址和 API Key");
        let no_key = AiConfig { base_url: "https://x/v1".into(), ..Default::default() };
        assert_eq!(fetch_efforts(&no_key).unwrap_err(), "先填好接口地址和 API Key");
        let bad = AiConfig { base_url: "http://example.com/v1".into(), key: "k".into(), ..Default::default() };
        assert!(fetch_efforts(&bad).unwrap_err().contains("https"), "非 https 的远程地址在联网前就被拒绝");
    }
}
