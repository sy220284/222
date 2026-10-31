'use strict';
/** Original source AST evidence; metadata-only output, never source content. */
const fs=require('fs'),path=require('path'),ts=require('typescript');
const [root,index,output]=process.argv.slice(2);
if(!root||!index||!output)throw Error('usage: node original_ast_semantics.cjs <private-src> <source-index.jsonl> <report.json>');
const types=new Set(['ArrowFunction','FunctionDeclaration','MethodDeclaration','GetAccessor','FunctionExpression','Constructor']);
const isFn=n=>ts.isFunctionDeclaration(n)||ts.isMethodDeclaration(n)||ts.isArrowFunction(n)||ts.isFunctionExpression(n)||ts.isConstructorDeclaration(n)||ts.isGetAccessorDeclaration(n);
const assigns=new Set([ts.SyntaxKind.EqualsToken,ts.SyntaxKind.PlusEqualsToken,ts.SyntaxKind.MinusEqualsToken,ts.SyntaxKind.AsteriskEqualsToken,ts.SyntaxKind.SlashEqualsToken,ts.SyntaxKind.AmpersandEqualsToken,ts.SyntaxKind.BarEqualsToken,ts.SyntaxKind.QuestionQuestionEqualsToken]);
const expected=new Map();
for(const line of fs.readFileSync(index,'utf8').split('\n')){
 if(!line)continue;
 const e=JSON.parse(line);if(!types.has(e.kind))continue;
 if(expected.has(e.source_id))throw Error('duplicate function ID '+e.source_id);
 expected.set(e.source_id,e);
}
const seen=new Set(),ordinal=new Map(),errors=[],gaps=[];
let files=0,member=0,direct=0;
function walk(folder){
 for(const d of fs.readdirSync(folder,{withFileTypes:true})){
  const fn=path.join(folder,d.name);
  if(d.isDirectory()){walk(fn);continue;}
  if(!/\.(ts|tsx|js)$/.test(fn))continue;
  const rel=path.relative(root,fn).replaceAll('\\','/');
  const sf=ts.createSourceFile(rel,fs.readFileSync(fn,'utf8'),ts.ScriptTarget.Latest,true,
    rel.endsWith('.tsx')?ts.ScriptKind.TSX:rel.endsWith('.js')?ts.ScriptKind.JS:ts.ScriptKind.TS);
  files++;if(sf.parseDiagnostics.length)errors.push('source syntax error: '+rel);
  function visit(n){
   if(isFn(n)){
    const first=sf.getLineAndCharacterOfPosition(n.getStart(sf)).line+1;
    const last=sf.getLineAndCharacterOfPosition(n.end).line+1;
    const prefix='src/'+rel+':'+ts.SyntaxKind[n.kind]+':'+first+'-'+last;
    const ord=(ordinal.get(prefix)||0)+1;ordinal.set(prefix,ord);
    const id=prefix+':occurrence-'+ord;
    if(seen.has(id))errors.push('duplicate AST node: '+id);
    seen.add(id);
    let write=0,state=0,ret=0,thr=0,cat=0,awa=0;
    function scan(ch){
     if(ch!==n && isFn(ch))return;
     if(ts.isBinaryExpression(ch)&&assigns.has(ch.operatorToken.kind)&&
       (ts.isPropertyAccessExpression(ch.left)||ts.isElementAccessExpression(ch.left))){
       write++;
       if(/^(?:STATE|this|session|context)(?:\.|\[)/.test(ch.left.getText(sf)))state++;
     }
     if(ts.isReturnStatement(ch))ret++;
     if(ts.isThrowStatement(ch))thr++;
     if(ts.isCatchClause(ch))cat++;
     if(ts.isAwaitExpression(ch))awa++;
     ts.forEachChild(ch,scan);
    }
    scan(n);member+=write;direct+=state;
    const e=expected.get(id);
    if(!e)errors.push('AST function absent from index: '+id);
    else{
      for(const [key,n] of [['显式返回数',ret],['抛错数',thr],['捕获数',cat],['await数',awa]])
       if(Number(e.static_metrics?.[key])!==n)errors.push('AST metric mismatch '+key+' '+id);
      if(state>0&&Number(e.untrusted_original_state_write_count)===0)
       gaps.push({source_id:id,direct_state_assignments:state});
    }
   }
   ts.forEachChild(n,visit);
  }
  visit(sf);
 }
}
walk(root);
for(const id of expected.keys())if(!seen.has(id))errors.push('index function absent from AST: '+id);
const report={schema:'original-ast-semantic-proof/v1',typescript_version:ts.version,
 source_content_included:false,files_parsed:files,expected_functions:expected.size,
 actual_functions:seen.size,errors:errors.slice(0,100),error_count:errors.length,
 all_member_assignments:member,direct_state_assignments:direct,
 false_zero_state_functions:gaps.length,
 false_zero_state_assignments:gaps.reduce((s,x)=>s+x.direct_state_assignments,0),
 missing_state_write_coverage:gaps};
fs.mkdirSync(path.dirname(output),{recursive:true});
fs.writeFileSync(output,JSON.stringify(report,null,2)+'\n');
console.log(JSON.stringify({files,functions:seen.size,ast_errors:errors.length,
 false_zero_functions:gaps.length,false_zero_assignments:report.false_zero_state_assignments}));
if(errors.length||files!==1902||seen.size!==20788)process.exitCode=1;
