const esc = (s) => String(s).replace(/[&<>]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" }[c]));

ide.register({
  "command:status"(doc) {
    const lines = doc && typeof doc.text === "string" ? doc.text.split("\n").length : 0;
    const mcpList = doc && Array.isArray(doc.active_mcp) ? doc.active_mcp : [];
    const srvLines = mcpList.length > 0
      ? mcpList.map(s => `  - [${s.disabled ? "已禁用" : "已启用"}] ${s.id}: ${s.command} ${(s.args || []).join(" ")}`).join("\n")
      : "  (当前暂未配置外部 MCP 服务，可在扩展栏点击 MCP 展开表单或点击上方按钮一键添加)";
    const summary = `MCP 协议模块已启用 (v2024-11-05) · 已登记 ${mcpList.length} 个外部服务 · 当前文档 ${lines} 行`;
    const detail = [
      "[MCP 模型上下文协议模块状态]",
      "• 协议版本: 2024-11-05 (JSON-RPC 2.0 / Stdio & HTTP Transport)",
      `• 当前活动文档行数: ${lines} 行`,
      `• 当前已登记自定义/第三方 MCP 服务 (${mcpList.length} 个):`,
      srvLines,
      "• 自动发现与合并配置: 工作区 .mcp.json / mcp_servers.json / .cursor/mcp.json + 全局 mcp_servers.json / custom_mcp/*.json",
      "• 自定义与第三方支持: 兼容标准 mcpServers 与第三方 servers 格式（支持 node/python/npx/uvx/可执行文件），可在扩展栏点击 MCP 直接添加或导入",
      "• 已注册桥接工具: mcp.status / tools/list / tools/call / resources/list"
    ].join("\n");
    return {
      summary,
      html: `<pre>${esc(detail)}</pre>`
    };
  },
  "agent.tool:status"(args) {
    const text = args && typeof args.text === "string" ? args.text : "";
    return {
      result: {
        protocolVersion: "2024-11-05",
        status: "active",
        lines: text ? text.split("\n").length : 0
      }
    };
  }
});
