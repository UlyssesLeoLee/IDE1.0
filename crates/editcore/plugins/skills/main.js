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
    return {
      summary: `Skills 模块已启用 (${SKILLS.length} 个核心技能)`,
      html: `<pre>${SKILLS.map(s => `${s.trigger}  ${s.name} (${s.id})`).join("\n")}</pre>`
    };
  },
  "agent.tool:list_skills"() {
    return { result: { skills: SKILLS } };
  }
});
