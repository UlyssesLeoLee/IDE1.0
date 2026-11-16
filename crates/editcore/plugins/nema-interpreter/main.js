ide.register({
  parse(source) {
    const parser = globalThis.NemaStrict;
    if (!parser || typeof parser.parse !== "function") {
      throw new Error("NemaStrict.parse unavailable (strict-parser.js not loaded)");
    }
    return parser.parse(String(source ?? ""));
  },
  render(source) {
    const parser = globalThis.NemaStrict;
    if (!parser || typeof parser.parse !== "function") {
      return { error: "NemaStrict.parse unavailable (strict-parser.js not loaded)" };
    }
    try {
      const ast = parser.parse(String(source ?? ""));
      return { ast, version: "strict-js-1.6" };
    } catch (e) {
      return { error: e && e.message ? e.message : String(e) };
    }
  },
  "command:validate"({ text }) {
    const parser = globalThis.NemaStrict;
    if (!parser || typeof parser.parse !== "function") {
      return "NemaStrict unavailable";
    }
    try {
      const domains = parser.parse(String(text ?? ""));
      const recCount = domains.reduce((acc, d) => acc + (d.records ? d.records.length : 0), 0);
      return `.NEMA Strict 校验通过: ${domains.length} 个域, ${recCount} 条记录`;
    } catch (e) {
      return `.NEMA Strict 语法错误: ${e && e.message ? e.message : String(e)}`;
    }
  },
});
