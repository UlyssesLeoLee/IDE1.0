const fs=require("fs");
const b="D:/GitGit/docs/gemarc-5d-atlas-v10.0/models/";
function shape(v,d=0){
  if(Array.isArray(v)) return "["+(v.length?shape(v[0],d+1):"empty")+"]";
  if(v&&typeof v==="object"){
    if(d>2) return "{...}";
    return "{"+Object.keys(v).map(k=>k+":"+shape(v[k],d+1)).join(", ")+"}";
  }
  return typeof v;
}
for(const f of fs.readdirSync(b)){
  const j=JSON.parse(fs.readFileSync(b+f,"utf8"));
  console.log("=== "+f+" ===");
  console.log(shape(j).slice(0,2600));
  console.log();
}
