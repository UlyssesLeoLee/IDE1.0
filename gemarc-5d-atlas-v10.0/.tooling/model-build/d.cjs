const fs=require("fs");
const j=JSON.parse(fs.readFileSync("D:/GitGit/docs/gemarc-5d-atlas-v10.0/models/diagram-definitions.json","utf8"));
console.log("rendererPolicy:",JSON.stringify(j.rendererPolicy));
for(const d of j.diagrams){
  console.log("--- "+d.id+" ["+d.diagramType+"] "+d.title);
  console.log("   keys:",Object.keys(d).join(","));
  const first=(d.classes&&d.classes[0])||(d.relations&&d.relations[0])||(d.elements&&d.elements[0]);
  if(first)console.log("   sample:",JSON.stringify(first).slice(0,400));
}
