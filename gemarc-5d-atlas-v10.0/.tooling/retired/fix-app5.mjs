// 补修 app.js：跨多行的 X.appendChild(el(...)) 调用
import fs from "node:fs";
import path from "node:path";

const file = path.resolve(process.argv[2]);
const lines = fs.readFileSync(file, "utf8").split(/\r?\n/);

let changed = 0;
for (let i = 0; i < lines.length; i++) {
  const l = lines[i];
  // 形如:  host.appendChild(el("div", { ... }, [
  const m = /^(\s*)([A-Za-z_$][\w$]*)\.appendChild\((el\(|wrap\b|win\b|diagBar\b|svg\b)/.exec(l);
  if (!m) continue;
  lines[i] = l.replace(".appendChild(", '.insertAdjacentHTML("beforeend", ');
  changed++;
}
fs.writeFileSync(file, lines.join("\n"), "utf8");
console.log("converted " + changed + " multi-line string appendChild call(s)");
