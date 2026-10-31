// Executes unmodified original TypeScript in the contributor's private source workspace.
import {readFileSync} from 'node:fs';
import {pathToFileURL} from 'node:url';
const original=await import(pathToFileURL(process.argv[2]).href);
const cases=JSON.parse(readFileSync(0,'utf8'));const out=[];
function key(v,i,mode) {
  switch(mode) {
    case 'parity': return v%2===0?'even':'odd';
    case 'initial': return String(v).slice(0,1);
    case 'identity': return String(v);
    case 'index-mod3': return String(i%3);
    case 'all': return '__proto__';
    case 'type': return v===null?'null':typeof v;
    default: throw Error('unrecognized source-side selector');
  }
}
for(const c of cases) {
  let result;
  if(c.export==='objectGroupBy') {
    result=original.objectGroupBy(c.input.values,(v,i)=>key(v,i,c.input.mode));
  } else if(['difference','intersects','every','union'].includes(c.export)) {
    result=original[c.export](new Set(c.input.a),new Set(c.input.b));
    if(result instanceof Set)result=Array.from(result);
  } else throw Error('unrecognized source export: '+c.export);
  out.push({id:c.id,expected:result});
}
process.stdout.write(JSON.stringify(out));
