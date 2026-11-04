// PPA 子集解释器 (WBS-02 4.4): Pascal 风格的宏脚本, 在一份「虚拟文档」上跑, 结果以一次补丁交还宿主 (一步撤销)。
// 纯函数: 没有网络 / DOM; 靠 editor.read 拿文档文本, editor.write 才能把结果写回。
//
// 语言子集:
//   声明   var a, b: Integer; s: String;        procedure P(x: Integer); ... end;   function F(x): Integer; ... end;
//   语句   := | if/then/else | while/do | for/to|downto/do | repeat/until | begin/end | exit | break | 过程调用
//   表达式 + - * / div mod  = <> < > <= >=  and or not xor  字符串 'a''b' 与 #10 #13 字符码  (字符串用 + 连接)
//   内建   Length Copy Pos UpperCase LowerCase Trim IntToStr StrToInt StrToIntDef Chr Ord
//   编辑   S_GetSelectedString S_GetLineStr S_GetLineCount S_GetFileName S_InsText S_Left S_Right S_Up S_Down
//          S_GoLineTop S_GoLineEnd S_GoFileTop S_GoFileEnd S_SelectAll S_Delete S_DeleteLine S_ReplaceAll
//          S_InputBox (取默认值) S_MessageBox (记入输出) S_Sleep (忽略)
// 不支持: 类型检查 / 记录 / 数组 / 单元 / 模式对话框; 遇到不认识的函数明确报错, 不静默忽略。

const MAX_STEPS = 200000;

// ---------- 词法 ----------
function lex(src) {
  const out = []; let i = 0, line = 1;
  const err = (m) => { throw new Error("第 " + line + " 行: " + m); };
  while (i < src.length) {
    const c = src[i];
    if (c === "\n") { line++; i++; continue; }
    if (/\s/.test(c)) { i++; continue; }
    if (c === "/" && src[i + 1] === "/") { while (i < src.length && src[i] !== "\n") i++; continue; }
    if (c === "{") { while (i < src.length && src[i] !== "}") { if (src[i] === "\n") line++; i++; } i++; continue; }
    if (c === "(" && src[i + 1] === "*") { i += 2; while (i < src.length && !(src[i] === "*" && src[i + 1] === ")")) { if (src[i] === "\n") line++; i++; } i += 2; continue; }
    if (c === "'" || c === '"') {                          // 字符串: 同种引号连写两次 = 一个引号
      let s = ""; i++;
      for (;;) {
        if (i >= src.length) err("字符串没有结束");
        if (src[i] === c) { if (src[i + 1] === c) { s += c; i += 2; continue; } i++; break; }
        s += src[i++];
      }
      out.push({ t: "str", v: s, line }); continue;
    }
    if (c === "#") {                                      // #10 = 字符码
      let j = i + 1; while (/[0-9]/.test(src[j] || "")) j++;
      if (j === i + 1) err("# 后面要跟字符码");
      out.push({ t: "str", v: String.fromCharCode(+src.slice(i + 1, j)), line }); i = j; continue;
    }
    if (/[0-9]/.test(c)) { let j = i; while (/[0-9]/.test(src[j] || "")) j++; out.push({ t: "num", v: +src.slice(i, j), line }); i = j; continue; }
    if (/[A-Za-z_]/.test(c)) { let j = i; while (/[A-Za-z0-9_]/.test(src[j] || "")) j++; out.push({ t: "id", v: src.slice(i, j), k: src.slice(i, j).toLowerCase(), line }); i = j; continue; }
    const two = src.slice(i, i + 2);
    if ([":=", "<>", "<=", ">="].includes(two)) { out.push({ t: "op", v: two, line }); i += 2; continue; }
    if ("+-*/=<>(),;:.".includes(c)) { out.push({ t: "op", v: c, line }); i++; continue; }
    err("不认识的字符「" + c + "」");
  }
  out.push({ t: "eof", line });
  return out;
}

// ---------- 语法 ----------
function parse(tokens) {
  let p = 0;
  const peek = () => tokens[p], next = () => tokens[p++];
  const err = (m) => { throw new Error("第 " + peek().line + " 行: " + m); };
  const isKw = (k) => peek().t === "id" && peek().k === k;
  const isOp = (o) => peek().t === "op" && peek().v === o;
  const eatKw = (k) => { if (!isKw(k)) err("这里应该是 " + k); return next(); };
  const eatOp = (o) => { if (!isOp(o)) err("这里应该是 " + o); return next(); };
  const ident = () => { if (peek().t !== "id") err("这里应该是名字"); return next(); };

  const procs = new Map(), globals = [];
  const BIN = [["or", "xor"], ["and"], ["=", "<>", "<", ">", "<=", ">="], ["+", "-"], ["*", "/", "div", "mod"]];
  function expr(level = 0) {
    if (level >= BIN.length) return unary();
    let l = expr(level + 1);
    for (;;) {
      const t = peek(), op = t.t === "op" ? t.v : t.t === "id" ? t.k : null;
      if (!op || !BIN[level].includes(op)) return l;
      next(); l = { n: "bin", op, l, r: expr(level + 1) };
    }
  }
  function unary() {
    if (isKw("not")) { next(); return { n: "not", e: unary() }; }
    if (isOp("-")) { next(); return { n: "neg", e: unary() }; }
    return primary();
  }
  function primary() {
    const t = peek();
    if (t.t === "num" || t.t === "str") { next(); return { n: "lit", v: t.v }; }
    if (isOp("(")) { next(); const e = expr(); eatOp(")"); return e; }
    if (t.t === "id") {
      next();
      if (t.k === "true") return { n: "lit", v: true };
      if (t.k === "false") return { n: "lit", v: false };
      if (isOp("(")) { next(); const args = []; if (!isOp(")")) { do { args.push(expr()); } while (isOp(",") && next()); } eatOp(")"); return { n: "call", name: t.k, args, line: t.line }; }
      return { n: "var", name: t.k, line: t.line };
    }
    return err("表达式不完整");
  }
  function stmt() {
    const t = peek();
    if (isKw("begin")) { next(); const body = []; while (!isKw("end")) { if (peek().t === "eof") err("缺少 end"); body.push(stmt()); if (isOp(";")) next(); } next(); return { n: "block", body }; }
    if (isKw("if")) { next(); const c = expr(); eatKw("then"); const a = stmt(); let b = null; if (isKw("else")) { next(); b = stmt(); } return { n: "if", c, a, b }; }
    if (isKw("while")) { next(); const c = expr(); eatKw("do"); return { n: "while", c, body: stmt() }; }
    if (isKw("repeat")) { next(); const body = []; while (!isKw("until")) { if (peek().t === "eof") err("缺少 until"); body.push(stmt()); if (isOp(";")) next(); } next(); return { n: "repeat", body, c: expr() }; }
    if (isKw("for")) {
      next(); const v = ident().k; eatOp(":="); const from = expr();
      const down = isKw("downto"); if (!down) eatKw("to"); else next();
      const to = expr(); eatKw("do"); return { n: "for", v, from, to, down, body: stmt() };
    }
    if (isKw("exit")) { next(); return { n: "exit" }; }
    if (isKw("break")) { next(); return { n: "break" }; }
    if (t.t === "id") {
      next();
      if (isOp(":=")) { next(); return { n: "set", name: t.k, e: expr(), line: t.line }; }
      const args = []; if (isOp("(")) { next(); if (!isOp(")")) { do { args.push(expr()); } while (isOp(",") && next()); } eatOp(")"); }
      return { n: "callstmt", name: t.k, args, line: t.line };
    }
    return err("看不懂这条语句");
  }
  function varDecl(into) {
    eatKw("var");
    while (peek().t === "id" && !["procedure", "function", "begin", "var"].includes(peek().k)) {
      const names = [ident().k]; while (isOp(",")) { next(); names.push(ident().k); }
      eatOp(":"); ident(); if (isOp("(") || isOp("[")) err("不支持这种类型"); eatOp(";");
      into.push(...names);
    }
  }
  function proc() {
    const isFn = next().k === "function", name = ident().k, params = [];
    if (isOp("(")) { next(); if (!isOp(")")) { do { if (isKw("var")) next(); const ns = [ident().k]; while (isOp(",")) { next(); ns.push(ident().k); } eatOp(":"); ident(); params.push(...ns); } while (isOp(";") && next()); } eatOp(")"); }
    if (isFn) { eatOp(":"); ident(); }
    eatOp(";");
    const locals = []; if (isKw("var")) varDecl(locals);
    const body = stmt(); if (isOp(";")) next();
    procs.set(name, { params, locals, body, isFn });
  }
  const top = [];
  while (peek().t !== "eof") {
    if (isKw("var")) { varDecl(globals); continue; }
    if (isKw("procedure") || isKw("function")) { proc(); continue; }
    if (isOp(".") || isOp(";")) { next(); continue; }
    top.push(stmt()); if (isOp(";")) next();
  }
  return { procs, globals, top };
}

// ---------- 虚拟文档 ----------
function makeDoc(input) {
  const lines = String(input.text == null ? "" : input.text).replace(/\r\n?/g, "\n").split("\n");
  const cur = input.cursor ? { row: Math.min(Math.max(0, input.cursor.row | 0), lines.length - 1), col: Math.max(0, input.cursor.col | 0) } : { row: 0, col: 0 };
  let anchor = null;
  const s = input.selection;
  if (s && (s.r0 !== s.r1 || s.c0 !== s.c1)) { anchor = { row: s.r0, col: s.c0 }; cur.row = s.r1; cur.col = s.c1; }
  return { lines, cur, anchor, name: input.name || "", log: [] };
}
const order = (a, b) => (a.row < b.row || (a.row === b.row && a.col <= b.col) ? [a, b] : [b, a]);
function selText(d) {
  if (!d.anchor) return "";
  const [a, b] = order(d.anchor, d.cur);
  if (a.row === b.row) return d.lines[a.row].slice(a.col, b.col);
  return [d.lines[a.row].slice(a.col), ...d.lines.slice(a.row + 1, b.row), d.lines[b.row].slice(0, b.col)].join("\n");
}
function delSel(d) {
  if (!d.anchor) return false;
  const [a, b] = order(d.anchor, d.cur);
  const head = d.lines[a.row].slice(0, a.col), tail = d.lines[b.row].slice(b.col);
  d.lines.splice(a.row, b.row - a.row + 1, head + tail);
  d.cur = { row: a.row, col: a.col }; d.anchor = null; return true;
}
function insText(d, text) {
  delSel(d);
  const parts = String(text).replace(/\r\n?/g, "\n").split("\n"), l = d.lines[d.cur.row];
  const head = l.slice(0, d.cur.col), tail = l.slice(d.cur.col);
  if (parts.length === 1) { d.lines[d.cur.row] = head + parts[0] + tail; d.cur.col += parts[0].length; return; }
  const mid = parts.slice(1, -1), last = parts[parts.length - 1];
  d.lines.splice(d.cur.row, 1, head + parts[0], ...mid, last + tail);
  d.cur = { row: d.cur.row + parts.length - 1, col: last.length };
}
const clampCol = (d) => { d.cur.col = Math.min(d.cur.col, d.lines[d.cur.row].length); };
function move(d, f) { d.anchor = null; f(); clampCol(d); }

// 编辑内建: 名字 (不含 S_ 前缀, 小写) -> (doc, args) => 值
const EDIT = {
  getselectedstring: (d) => selText(d),
  getlinestr: (d, [n]) => d.lines[(+n > 0 ? +n - 1 : d.cur.row)] ?? "",
  getlinecount: (d) => d.lines.length,
  getfilename: (d) => d.name,
  instext: (d, [s]) => { insText(d, s); return 0; },
  left: (d) => { move(d, () => { if (d.cur.col > 0) d.cur.col--; else if (d.cur.row > 0) { d.cur.row--; d.cur.col = d.lines[d.cur.row].length; } }); return 0; },
  right: (d) => { move(d, () => { if (d.cur.col < d.lines[d.cur.row].length) d.cur.col++; else if (d.cur.row < d.lines.length - 1) { d.cur.row++; d.cur.col = 0; } }); return 0; },
  up: (d) => { move(d, () => { if (d.cur.row > 0) d.cur.row--; }); return 0; },
  down: (d) => { move(d, () => { if (d.cur.row < d.lines.length - 1) d.cur.row++; }); return 0; },
  golinetop: (d) => { move(d, () => { d.cur.col = 0; }); return 0; },
  golineend: (d) => { move(d, () => { d.cur.col = d.lines[d.cur.row].length; }); return 0; },
  gofiletop: (d) => { move(d, () => { d.cur = { row: 0, col: 0 }; }); return 0; },
  gofileend: (d) => { move(d, () => { const r = d.lines.length - 1; d.cur = { row: r, col: d.lines[r].length }; }); return 0; },
  selectall: (d) => { d.anchor = { row: 0, col: 0 }; const r = d.lines.length - 1; d.cur = { row: r, col: d.lines[r].length }; return 0; },
  delete: (d) => { if (!delSel(d)) { const l = d.lines[d.cur.row]; if (d.cur.col < l.length) d.lines[d.cur.row] = l.slice(0, d.cur.col) + l.slice(d.cur.col + 1); else if (d.cur.row < d.lines.length - 1) d.lines.splice(d.cur.row, 2, l + d.lines[d.cur.row + 1]); } return 0; },
  deleteline: (d) => { d.anchor = null; if (d.lines.length === 1) d.lines[0] = ""; else d.lines.splice(d.cur.row, 1); d.cur.row = Math.min(d.cur.row, d.lines.length - 1); d.cur.col = 0; return 0; },
  replaceall: (d, [from, to]) => { from = String(from); if (!from) return 0; let n = 0; d.lines = d.lines.map((l) => l.split(from).reduce((acc, part, i) => { if (i) n++; return i ? acc + String(to) + part : part; }, "")); clampCol(d); return n; },
  inputbox: (d, [, def]) => String(def ?? ""),
  messagebox: (d, [m]) => { d.log.push(String(m)); return 0; },
  sleep: () => 0,
};
const PURE = {
  length: ([s]) => String(s).length,
  copy: ([s, i, n]) => String(s).substr(Math.max(0, i - 1), Math.max(0, n)),
  pos: ([sub, s]) => String(s).indexOf(String(sub)) + 1,
  uppercase: ([s]) => String(s).toUpperCase(), lowercase: ([s]) => String(s).toLowerCase(), trim: ([s]) => String(s).trim(),
  inttostr: ([n]) => String(Math.trunc(+n)), strtoint: ([s]) => { const n = parseInt(s, 10); if (Number.isNaN(n)) throw new Error("StrToInt: 「" + s + "」不是整数"); return n; },
  strtointdef: ([s, d]) => { const n = parseInt(s, 10); return Number.isNaN(n) ? d : n; },
  chr: ([n]) => String.fromCharCode(n), ord: ([s]) => String(s).charCodeAt(0) || 0,
};

// ---------- 执行 ----------
class Exit { }
class Break { }
function run(prog, doc) {
  let steps = 0;
  const tick = () => { if (++steps > MAX_STEPS) throw new Error("脚本执行步数超过 " + MAX_STEPS + " (死循环?) —— 已中止, 文档没有被修改"); };
  const globals = new Map(prog.globals.map((n) => [n, 0]));
  const lookup = (env, name, line) => {
    if (env.has(name)) return env;
    if (globals.has(name)) return globals;
    throw new Error("第 " + line + " 行: 变量「" + name + "」没有声明 (var)");
  };
  function callNamed(name, args, env, line) {
    tick();
    const pr = prog.procs.get(name);
    if (pr) {
      if (args.length !== pr.params.length) throw new Error("第 " + line + " 行: " + name + " 需要 " + pr.params.length + " 个参数");
      const e = new Map([...pr.params.map((p, i) => [p, args[i]]), ...pr.locals.map((l) => [l, 0]), ["result", 0]]);
      try { exec(pr.body, e); } catch (x) { if (!(x instanceof Exit)) throw x; }
      return pr.isFn ? e.get("result") : 0;
    }
    if (PURE[name]) return PURE[name](args);
    if (name.startsWith("s_") && EDIT[name.slice(2)]) return EDIT[name.slice(2)](doc, args);
    throw new Error("第 " + line + " 行: 不支持的函数「" + name + "」(PPA 子集)");
  }
  function ev(e, env) {
    switch (e.n) {
      case "lit": return e.v;
      case "var": return lookup(env, e.name, e.line).get(e.name);
      case "neg": return -ev(e.e, env);
      case "not": { const v = ev(e.e, env); return typeof v === "boolean" ? !v : ~v; }
      case "call": return callNamed(e.name, e.args.map((a) => ev(a, env)), env, e.line);
      case "bin": {
        const a = ev(e.l, env);
        if (e.op === "and" && typeof a === "boolean" && !a) return false;       // 布尔短路
        if (e.op === "or" && typeof a === "boolean" && a) return true;
        const b = ev(e.r, env);
        switch (e.op) {
          case "+": return typeof a === "string" || typeof b === "string" ? String(a) + String(b) : a + b;
          case "-": return a - b; case "*": return a * b;
          case "/": return a / b; case "div": return Math.trunc(a / b); case "mod": return a % b;
          case "=": return a === b; case "<>": return a !== b; case "<": return a < b; case ">": return a > b; case "<=": return a <= b; case ">=": return a >= b;
          case "and": return typeof a === "boolean" ? a && b : a & b;
          case "or": return typeof a === "boolean" ? a || b : a | b;
          case "xor": return typeof a === "boolean" ? a !== b : a ^ b;
        }
      }
    }
    throw new Error("内部错误: 表达式 " + e.n);
  }
  function exec(s, env) {
    tick();
    switch (s.n) {
      case "block": for (const x of s.body) exec(x, env); return;
      case "set": { const t = s.name === "result" && env.has("result") ? env : lookup(env, s.name, s.line); t.set(s.name, ev(s.e, env)); return; }
      case "if": if (ev(s.c, env)) exec(s.a, env); else if (s.b) exec(s.b, env); return;
      case "while": try { while (ev(s.c, env)) { tick(); exec(s.body, env); } } catch (x) { if (!(x instanceof Break)) throw x; } return;
      case "repeat": try { do { tick(); for (const x of s.body) exec(x, env); } while (!ev(s.c, env)); } catch (x) { if (!(x instanceof Break)) throw x; } return;
      case "for": {
        const t = lookup(env, s.v, 0), to = ev(s.to, env);
        try { for (let i = ev(s.from, env); s.down ? i >= to : i <= to; i += s.down ? -1 : 1) { tick(); t.set(s.v, i); exec(s.body, env); } } catch (x) { if (!(x instanceof Break)) throw x; }
        return;
      }
      case "exit": throw new Exit();
      case "break": throw new Break();
      case "callstmt": callNamed(s.name, s.args.map((a) => ev(a, env)), env, s.line); return;
    }
    throw new Error("内部错误: 语句 " + s.n);
  }
  const env = new Map();
  try {
    for (const s of prog.top) exec(s, env);
    if (!prog.top.length || prog.procs.has("main")) { if (prog.procs.has("main")) callNamed("main", [], env, 0); }
  } catch (x) { if (!(x instanceof Exit)) throw x; }
}

// 新旧两份行之间的最小补丁 (头尾相同的行不动): 宿主按一步撤销套用
function diff(oldLines, newLines) {
  let a = 0; while (a < oldLines.length && a < newLines.length && oldLines[a] === newLines[a]) a++;
  let b = 0; while (b < oldLines.length - a && b < newLines.length - a && oldLines[oldLines.length - 1 - b] === newLines[newLines.length - 1 - b]) b++;
  return { start: a, remove: oldLines.length - a - b, insert: newLines.slice(a, newLines.length - b) };
}

function runScript(input) {
  const script = String(input.script || "");
  if (!script.trim()) return { error: "没有脚本" };
  const doc = makeDoc(input), before = doc.lines.slice();
  try { run(parse(lex(script)), doc); } catch (e) { return { error: "PPA: " + e.message }; }
  const patch = diff(before, doc.lines);
  const changed = patch.remove > 0 || patch.insert.length > 0;
  return { summary: (changed ? "已修改 " + (patch.remove || patch.insert.length) + " 行" : "文档没有变化") + (doc.log.length ? "；输出: " + doc.log.join(" / ") : ""), edit: changed ? Object.assign(patch, { cursor: { row: doc.cur.row, col: doc.cur.col } }) : null, log: doc.log };
}

if (typeof ide !== "undefined") ide.register({ "command:run": (input) => runScript(input || {}) });
if (typeof module !== "undefined") module.exports = { runScript, lex, parse };
