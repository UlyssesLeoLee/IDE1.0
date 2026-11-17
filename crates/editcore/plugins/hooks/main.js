function scanText(text) {
  const src = String(text ?? "");
  const lines = src.split("\n");
  const findings = [];
  for (let i = 0; i < lines.length; i++) {
    const l = lines[i];
    if (l.includes("sk-") || l.includes("Bearer ")) {
      findings.push(`第 ${i + 1} 行: 检测到疑似明文密钥/Token (sk-/Bearer)`);
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
    const findings = scanText(doc && doc.text);
    if (findings.length === 0) {
      return { summary: "Hooks 安全巡检通过: 未发现明文密钥或危险路径引用" };
    }
    return {
      summary: `Hooks 巡检发现 ${findings.length} 项风险`,
      html: `<pre>${esc(findings.join("\n"))}</pre>`
    };
  },
  "agent.tool:security_scan"(args) {
    const findings = scanText(args && args.text);
    return { result: { passed: findings.length === 0, count: findings.length, findings } };
  }
});
