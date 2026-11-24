const SKILLS = [
  { id: "code-review", trigger: "/review", name: "代码审查 (Code Review)" },
  { id: "unit-test", trigger: "/test", name: "单测生成 (Unit Test)" },
  { id: "refactor", trigger: "/refactor", name: "架构重构 (Refactor)" },
  { id: "git-commit", trigger: "/commit", name: "Git 提交提炼 (Git Commit)" },
  { id: "nema-optimize", trigger: "/nema", name: "NEMA 拓扑优化 (NEMA Optimizer)" },
  { id: "explain", trigger: "/explain", name: "架构解说 (Explain)" }
];

ide.register({
  "command:list"() {
    const builtinList = SKILLS.map(s => `${s.trigger}  ${s.name} (${s.id})`).join("\n");
    const extInfo = [
      "",
      "[自定义与第三方 Skills 扩展支持]",
      "• 工作区目录: .skills/<id>/SKILL.md、.skills/*.md、.skills/*.json",
      "• 第三方兼容目录: .claude/skills/、.claude/commands/*.md、.agents/skills/",
      "• 全局自定义目录: <plugins_root>/custom_skills/",
      "• 可在「插件」面板点击「➕ 新建自定义 Skill」或「📥 导入第三方 Skill」一键添加并热重载"
    ].join("\n");
    return {
      summary: `Skills 模块已启用 (${SKILLS.length} 个内建技能 + 支持自定义与第三方 SKILL.md / JSON 扩展)`,
      html: `<pre>${builtinList}\n${extInfo}</pre>`
    };
  },
  "agent.tool:list_skills"() {
    return { result: { skills: SKILLS } };
  }
});
