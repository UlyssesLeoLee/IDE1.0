const MODES = [
  { id: "code", label: "⚡ Code (编码直改)" },
  { id: "plan", label: "📐 Plan (架构规划)" },
  { id: "review", label: "🔍 Review (对抗审查)" },
  { id: "ask", label: "💬 Ask (只读问答)" }
];

ide.register({
  "command:cycle"() {
    return {
      summary: "Claude Mod 模式已就绪: 支持 Code / Plan / Review / Ask 四种智能体行为模式切换"
    };
  },
  "agent.tool:modes"() {
    return { result: { modes: MODES } };
  }
});
