/* .NEMA strict candidate parser. Kept in sync with interpreter/nema_strict.py.
   Zero lookahead ambiguity: native tokens split before escape decoding. */
(function(root){
'use strict';
const MISSING=()=>({type:'missing'});
const IMPLICIT_MISSING=Object.freeze({type:'missing'});
const esc={' ':' ','\\':'\\','?':'?','>':'>','<':'<','=':'=','~':'~','^':'^',n:'\n',r:'\r',t:'\t',_:'\u0000MISSING\u0000',e:'\u0000EMPTY\u0000'};
const HEADER=/^ \\([?>]?)([1-9][0-9]*) ([1-9][0-9]*)$/;
const MALFORMED_HEADER=/^ +\\(?:[?>]?[0-9]|[?>] +[0-9])/;
const CONT_INDEX=/^(?:0|[1-9][0-9]*)$/;
const NODE=/^[A-Za-z][A-Za-z0-9_.:-]*$/;
const EDGE=/^([A-Za-z][A-Za-z0-9_.:-]*)([><=])([A-Za-z][A-Za-z0-9_.:-]*)([><=])([A-Za-z][A-Za-z0-9_.:-]*)$/;
function fail(n,msg){throw new Error('line '+n+': '+msg)}
function tokenize(s,n){
 if(!s)fail(n,'empty field expression');
 let arr=[],cur=[];
 for(let i=0;i<s.length;){
  let c=s[i];
  if(c==='\\'){
   if(i+1>=s.length)fail(n,'trailing escape');
   let e=s[i+1];
   if(!(e in esc))fail(n,'unknown escape \\'+e);
   cur.push(['literal',esc[e]]);
   i+=2;
   continue;
  }
  if(c===' '){
   if(!cur.length)fail(n,'consecutive spaces');
   arr.push(cur);
   cur=[];
   i++;
   continue;
  }
  cur.push(['raw',c]);
  i++;
 }
 if(!cur.length)fail(n,'trailing separator space');
 arr.push(cur);
 return arr;
}
function value(p,n){
 if(p.length===1){
  const [t,v]=p[0];
  if(t==='raw'&&v==='~')return null;
  if(t==='literal'&&v==='\u0000MISSING\u0000')return MISSING();
  if(t==='literal'&&v==='\u0000EMPTY\u0000')return '';
 }
 if(p.some(([,v])=>v==='\u0000MISSING\u0000'||v==='\u0000EMPTY\u0000'))fail(n,'standalone token \\e or \\_ cannot be combined');
 return p.map(x=>x[1]).join('');
}
function fields(s,n){return tokenize(s,n).map(p=>value(p,n))}
function parse(text){
 if(text.includes('\t'))fail(0,'raw Tab prohibited');
 if(text.includes('\u3000'))fail(0,'fullwidth space prohibited');
 if(text.includes('\r'))fail(0,'raw CR prohibited');
 let arr=[],active=null,ids=new Set(),lines=text.split('\n');
 if(lines.at(-1)==='')lines.pop();
 for(let i=0;i<lines.length;i++){
  const l=lines[i],n=i+1;
  if(!l)continue;
  const header=HEADER.exec(l);
  if(header){
   const [,tag,id,rev]=header;
   if(ids.has(id))fail(n,'duplicate domain ID');
   ids.add(id);
   active={mode:tag==='?'?'5w1h':tag==='>'?'graph':'table',id:+id,revision:+rev,rows:[],records:[]};
   if(active.mode==='table')Object.assign(active,{columns:null,explicit:false,declared_width:null});
   arr.push(active);
   continue;
  }
  if(MALFORMED_HEADER.test(l))fail(n,'malformed domain header');
  if(!active)fail(n,'body before domain');
  const mode=active.mode;
  if(mode==='table'&&l.startsWith('\\#')){
   if(active.records.length||active.explicit)fail(n,'header after records');
   if(!l.startsWith('\\# '))fail(n,'header needs labels');
   const names=fields(l.slice(3),n);
   if(names.some(x=>typeof x!=='string'||!x))fail(n,'invalid column name');
   if(new Set(names).size!==names.length)fail(n,'duplicate column name');
   if(active.declared_width!=null&&names.length!==active.declared_width)fail(n,'column width mismatch');
   active.columns=names;active.explicit=true;active.rows.push(l);
   continue;
  }
  if(mode==='table'&&l.startsWith('\\%')){
   if(active.records.length||active.declared_width!=null)fail(n,'width duplicate / too late');
   if(!/^\\%[1-9][0-9]*$/.test(l))fail(n,'invalid width');
   let count=+l.slice(2);
   if(active.explicit&&active.columns.length!==count)fail(n,'column width mismatch');
   active.declared_width=count;
   if(!active.explicit)active.columns=Array.from({length:count},(_,j)=>'column'+(j+1));
   active.rows.push(l);
   continue;
  }
  if(l.startsWith('  '))fail(n,'consecutive leading spaces prohibited; use " N value"');
  if(l.startsWith(' ')){
   if(!active.records.length)fail(n,'continuation without record');
   const rest=l.slice(1);
   const vals=fields(rest,n);
   let col,v;
   if(vals.length===2){
    const idxToken=rest.slice(0,rest.indexOf(' '));
    if(!CONT_INDEX.test(idxToken))fail(n,'invalid continuation column index');
    col=+vals[0];
    v=vals[1];
    if(mode==='graph'&&col===0)fail(n,'cannot continue graph ID X0');
   }else if(vals.length===1){
    col=mode==='graph'?2:1;
    v=vals[0];
   }else{
    fail(n,'continuation must contain one field; escape inner spaces');
   }
   if(typeof v!=='string')fail(n,'continuation must be text');
   if(mode==='5w1h'&&col>=6)fail(n,'continuation outside 5w1h');
   if(mode==='table'&&col>=active.columns.length)fail(n,'continuation exceeds table width');
   const rec=active.records.at(-1),a=rec.values;
   while(a.length<=col)a.push(IMPLICIT_MISSING);
   let old=a[col];
   if(old===null)fail(n,'cannot append to null');
   if(typeof old==='object'){
    if(old===IMPLICIT_MISSING)a[col]=v;
    else fail(n,'cannot append to missing');
   }else a[col]=old+v;
   active.rows.push(l);
   continue;
  }
  const partList=tokenize(l,n);
  const vals=partList.map(p=>value(p,n));
  if(mode==='table'){
   if(active.columns==null)active.columns=Array.from({length:vals.length},(_,j)=>'column'+(j+1));
   if(vals.length>active.columns.length)fail(n,'record exceeds width');
   while(vals.length<active.columns.length)vals.push(IMPLICIT_MISSING);
   active.records.push({values:vals});
  }else if(mode==='5w1h'){
   if(vals.length>6)fail(n,'5w1h exceeds six fields');
   while(vals.length<6)vals.push(IMPLICIT_MISSING);
   active.records.push({values:vals});
  }else{
   const token=vals[0],firstParts=partList[0];
   if(typeof token!=='string')fail(n,'invalid graph ID');
   let rec;
   if(firstParts.some(([t,v])=>t==='raw'&&(v==='>'||v==='<'||v==='='))){
    if(firstParts.some(([t,v])=>t==='literal'&&(v==='>'||v==='<'||v==='=')))fail(n,'invalid edge expression');
    const m=EDGE.exec(token);
    if(!m||m[2]!==m[4])fail(n,'invalid edge expression');
    if(vals.length<2||(vals[1]!==null&&typeof vals[1]==='object'))fail(n,'edge name missing');
    rec={kind:'edge',id:m[3],from:m[2]==='<'?m[5]:m[1],to:m[2]==='<'?m[1]:m[5],operator:m[2],values:vals};
   }else{
    if(!NODE.test(token))fail(n,'invalid node ID');
    if(vals.length<2||(vals[1]!==null&&typeof vals[1]==='object'))fail(n,'node name missing');
    rec={kind:'node',id:token,values:vals};
   }
   active.records.push(rec);
  }
  active.rows.push(l);
 }
 for(const d of arr){
  if(d.mode!=='graph')continue;
  const nodes=new Set(),edges=new Set();
  for(const r of d.records){
   const pool=r.kind==='node'?nodes:edges;
   if(pool.has(r.id))fail(0,'duplicate '+r.kind+' ID: '+r.id);
   pool.add(r.id);
  }
  for(const id of nodes)if(edges.has(id))fail(0,'node edge ID overlap');
  for(const r of d.records)if(r.kind==='edge'&&(!nodes.has(r.from)||!nodes.has(r.to)))fail(0,'dangling edge '+r.id);
 }
 return arr;
}
root.NemaStrict={parse,fields};
})(typeof window!=='undefined'?window:globalThis);
