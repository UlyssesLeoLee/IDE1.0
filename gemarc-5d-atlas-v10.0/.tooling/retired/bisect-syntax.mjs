// 二分定位 JS 语法问题：用 node 自身的 parser
// 策略：逐步延长前缀 → new Function(prefix)；错误信息里的行号收敛处即为问题行
import fs from "node:fs";
import vm from "node:vm";

const file = process.argv[2];
const lines = fs.readFileSync(file, "utf8").split(/\r?\n/);
const total = lines.length;

function check(n) {
  const src = lines.slice(0, n).join("\n");
  try {
    new vm.Script(src);
    return { ok: true };
  } catch (e) {
    const m = /:(\d+)\n/.exec(e.stack || "");
    return { ok: false, msg: e.message, line: m ? Number(m[1]) : null, stack: (e.stack || "").split("\n").slice(0, 4) };
  }
}

// 阶段 1：找出"Unexpected token"类错误首次出现的行号
let firstUnexpected = null;
for (let n = 1; n <= total; n++) {
  const r = check(n);
  if (!r.ok && r.msg && /Unexpected token|Unexpected identifier|Invalid or unexpected/i.test(r.msg)) {
    // 只有当报错行 <= n 时才算"文件自身"的错误，而非"前缀不完整"
    if (r.line && r.line <= n) { firstUnexpected = { n, ...r }; break; }
  }
}

if (!firstUnexpected) {
  console.log("no Unexpected-token error while growing prefix");
  const r = check(total);
  console.log("full-file result:", JSON.stringify({ ok: r.ok, msg: r.msg, line: r.line }));
} else {
  console.log("first Unexpected-token at prefix length", firstUnexpected.n);
  console.log("  message:", firstUnexpected.msg);
  console.log("  error line:", firstUnexpected.line);
  console.log("  ---");
  firstUnexpected.stack.forEach((l) => console.log("  " + l));

  // 阶段 2：在该前缀里二分，报错行号收敛的位置
  let lo = Math.max(1, firstUnexpected.n - 60);
  let hi = firstUnexpected.n;
  while (lo < hi) {
    const mid = Math.floor((lo + hi) / 2);
    const r = check(mid);
    if (!r.ok && r.line && r.line <= mid) hi = mid; else lo = mid + 1;
  }
  console.log("=== problem narrowed to line", lo, "===");
  for (let i = Math.max(0, lo - 4); i < Math.min(total, lo + 2); i++) {
    console.log((i + 1) + ": " + JSON.stringify(lines[i]));
  }
}
