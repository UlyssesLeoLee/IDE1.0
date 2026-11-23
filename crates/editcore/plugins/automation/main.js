function collectTodos(text) {
  const lines = String(text ?? "").split("\n");
  const items = [];
  for (let i = 0; i < lines.length; i++) {
    if (/(TODO|FIXME|HACK|XXX)/.test(lines[i])) {
      items.push(`L${i + 1}: ${lines[i].trim()}`);
    }
  }
  return items;
}

const esc = (s) => String(s).replace(/[&<>]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" }[c]));

ide.register({
  "command:clean"(doc) {
    if (!doc || typeof doc.text !== "string") {
      return { summary: "没有活动文档内容" };
    }
    const lines = doc.text.split("\n");
    let changed = 0;
    const cleaned = lines.map(l => {
      const trimmed = l.replace(/[ \t]+$/, "");
      if (trimmed !== l) changed++;
      return trimmed;
    });
    if (changed === 0) {
      return { summary: "Automation: 文档已非常整洁，无需清理行尾空白" };
    }
    return {
      summary: `Automation: 已自动清理 ${changed} 行的行尾多余空白`,
      edit: {
        start: 0,
        remove: lines.length,
        insert: cleaned
      }
    };
  },
  "command:todos"(doc) {
    const items = collectTodos(doc && doc.text);
    if (items.length === 0) {
      return {
        summary: "Automation: 当前文档没有未完成的 TODO / FIXME 项",
        html: "<pre>[Automation 待办扫描]\n✅ 当前活动文档未发现 TODO / FIXME / HACK / XXX 待办标记。</pre>"
      };
    }
    return {
      summary: `Automation: 扫描到 ${items.length} 个待办事项`,
      html: `<pre>${esc(items.join("\n"))}</pre>`
    };
  },
  "agent.tool:scan_todos"(args) {
    const items = collectTodos(args && args.text);
    return { result: { count: items.length, items } };
  }
});
