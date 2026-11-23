function scanText(text) {
  const src = String(text ?? "");
  const lines = src.split("\n");
  const findings = [];
  for (let i = 0; i < lines.length; i++) {
    const l = lines[i];
    if (
      l.includes("sk-") ||
      l.includes("Bearer ") ||
      l.includes("ghp_") ||
      l.includes("AKIA") ||
      l.includes("-----BEGIN ")
    ) {
      findings.push(`第 ${i + 1} 行: 检测到疑似明文密钥/Token (sk-/Bearer/ghp_/AKIA/PEM)`);
    }
    if (l.includes("../") || l.includes("..\\")) {
      findings.push(`第 ${i + 1} 行: 检测到相对父目录跳转 (..)`);
    }
  }
  return findings;
}

const esc = (s) => String(s).replace(/[&<>]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" }[c]));

ide.register({
  "command:audit"(doc) {
    const text = doc && typeof doc.text === "string" ? doc.text : "";
    const lineCount = text ? text.split("\n").length : 0;
    const findings = scanText(text);
    const activeHooks = [
      "• Pre-Input: 注入工程安全与编码规范提醒",
      "• Pre-Tool (path-sanitizer): 拦截系统目录与 .. 路径越界写操作",
      "• Pre-Tool (loop-breaker): 拦截连续 3 次重复死循环工具调用",
      "• Post-Tool (payload-limit): 单次工具返回超 20,000 字符自动截断",
      "• Post-Response (secret-mask): 自动脱敏响应中的 sk-/ghp_/AKIA/PEM 凭证"
    ].join("\n");
    if (findings.length === 0) {
      return {
        summary: "Hooks 安全巡检通过: 未发现明文密钥或危险路径引用",
        html: `<pre>${esc(
          `[Hooks 生命周期安全引擎状态: 已启用]\n已扫描当前文档: ${lineCount} 行\n巡检结论: ✅ 安全巡检通过，未发现明文密钥或危险路径引用。\n\n当前挂载生命周期钩子:\n${activeHooks}`
        )}</pre>`
      };
    }
    return {
      summary: `Hooks 巡检发现 ${findings.length} 项风险`,
      html: `<pre>${esc(
        `[Hooks 生命周期安全引擎状态: 发现 ${findings.length} 项风险]\n已扫描当前文档: ${lineCount} 行\n\n风险明细:\n${findings.join("\n")}\n\n当前挂载生命周期钩子:\n${activeHooks}`
      )}</pre>`
    };
  },
  "agent.tool:security_scan"(args) {
    const findings = scanText(args && args.text);
    return { result: { passed: findings.length === 0, count: findings.length, findings } };
  }
});
