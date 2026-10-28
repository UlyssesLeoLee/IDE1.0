// 默认插件入口: 在沙箱里把 pele.global.js 暴露的 render 注册为 markdown.fence 的 mermaid 渲染器
ide.register({
  async render(text) {
    const out = await pele.render(text);
    return { svg: typeof out === "string" ? out : out.svg };
  },
});
