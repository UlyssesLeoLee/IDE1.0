const SKILLS = [
  { id: "code-review", trigger: "/review", name: "代码审查 (Code Review)" },
  { id: "unit-test", trigger: "/test", name: "单测生成 (Unit Test)" },
  { id: "refactor", trigger: "/refactor", name: "架构重构 (Refactor)" },
  { id: "git-commit", trigger: "/commit", name: "Git 提交提炼 (Git Commit)" },
  { id: "nema-optimize", trigger: "/nema", name: "NEMA 拓扑优化 (NEMA Optimizer)" },
  { id: "explain", trigger: "/explain", name: "架构解说 (Explain)" }
];

const esc = (s) => String(s).replace(/[&<>]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" }[c]));

ide.register({
  "command:list"(doc) {
    const active = doc && Array.isArray(doc.active_skills) && doc.active_skills.length > 0
      ? doc.active_skills
      : SKILLS.map(s => ({ id: s.id, trigger: s.trigger, name: s.name, is_builtin: true }));
    const customCount = active.filter(s => !s.is_builtin).length;
    const listText = active
      .map(s => `${s.is_builtin ? "[内建]" : "[自定义/第三方]"} ${s.trigger}  ${s.name} (${s.id})`)
      .join("\n");
    const extInfo = [
      "",
      "[自定义与第三方 Skills 扩展支持]",
      "• 工作区目录: .skills/<id>/SKILL.md、.skills/*.md、.skills/*.json",
      "• 第三方兼容目录: .claude/skills/、.claude/commands/*.md、.agents/skills/",
      "• 全局自定义目录: <plugins_root>/custom_skills/",
      "• 可在扩展栏点击「Skills」展开添加面板，或在本窗口上方点击「➕ 打开交互式添加与管理中心」直接添加并立即生效"
    ].join("\n");
    return {
      summary: `Skills 模块已启用 (共 ${active.length} 个技能: 内建 ${active.length - customCount} + 自定义/第三方 ${customCount})`,
      html: `<pre>${esc(listText)}\n${esc(extInfo)}</pre>`
    };
  },
  "agent.tool:list_skills"(args) {
    const active = args && Array.isArray(args.active_skills) && args.active_skills.length > 0
      ? args.active_skills
      : SKILLS;
    return { result: { skills: active } };
  }
});
