// 全局前置: webServer 起来后, 核对它内嵌的页面和仓库里的 editor.html 一致。
// 内嵌页面是编译进二进制的 (include_str!), 改了 editor.html 没重新编译, 整轮测的就是旧页面 —— 这里直接报错, 不让它悄悄过。
// 确认要测旧二进制: QA_ALLOW_STALE=1。
import * as fs from "fs";
import * as path from "path";

export default async function globalSetup() {
  if (process.env.QA_ALLOW_STALE === "1") return;
  const addr = process.env.IDE_SHELL_WEB_ADDR || "127.0.0.1:8123";
  const served = await (await fetch(`http://${addr}/editor`)).text();
  const src = fs.readFileSync(path.resolve(__dirname, "../../crates/ide-shell-web/src/editor.html"), "utf8");
  const norm = (t: string) => t.replace(/\r\n/g, "\n");
  if (norm(served) !== norm(src)) {
    throw new Error(`${process.env.IDE_SHELL_WEB_BIN} 内嵌的页面和 crates/ide-shell-web/src/editor.html 不一致: 先 cargo build -p ide-shell-web (或设 QA_ALLOW_STALE=1)`);
  }
}
