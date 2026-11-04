const fs=require("fs");
const b="D:/GitGit/docs/gemarc-5d-atlas-v10.0/models/";
function shape(v,d=0){
  if(Array.isArray(v)) return "["+(v.length?shape(v[0],d+1):"empty")+"]";
  if(v&&typeof v==="object"){
    if(d>3) return "{...}";
    return "{"+Object.keys(v).map(k=>k+":"+shape(v[k],d+1)).join(", ")+"}";
  }
  return typeof v;
}
for(const f of ["information-placement.yaml","prompts.json","prototype.json","requirements.json","scenario-deltas.json","spatial.json","traceability.json","tours.json"]){
  if(f.endsWith(".yaml")){ console.log("=== "+f+" (yaml, skipped) ==="); continue; }
  const j=JSON.parse(fs.readFileSync(b+f,"utf8"));
  console.log("=== "+f+" ===");
  console.log(shape(j).slice(0,3000));
  console.log();
}
