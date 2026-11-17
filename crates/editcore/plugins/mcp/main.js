ide.register({
  "command:status"(doc) {
    const lines = doc && typeof doc.text === "string" ? doc.text.split("\n").length : 0;
    return {
      summary: `MCP 协议模块已启用 (v2024-11-05) · 当前文档 ${lines} 行 · 支持 .mcp.json / mcp_servers.json 自动发现`
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
