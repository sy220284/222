// Real original 2.1.88 array.ts execution. No reimplemented array functions.
// Restrict test recipes to primitive integer arrays to avoid hiding JS
// reference-identity differences requiring separate correctness testing.
import {readFileSync} from 'node:fs';
import {pathToFileURL} from 'node:url';
const original = await import(pathToFileURL(process.argv[2]).href);
const cases = JSON.parse(readFileSync(0,'utf8'));
const observed = [];
for (const item of cases) {
  const {values,mode} = item.input;
  let expected;
  switch(item.export) {
    case 'intersperse':
      if (mode !== 'negative-index') throw Error('unsupported separator');
      expected = original.intersperse(values,i => -i);
      break;
    case 'count':
      if (mode !== 'positive' && mode !== 'even') throw Error('unsupported predicate');
      expected = original.count(values,value => mode==='positive' ? value>0 : value%2===0);
      break;
    case 'uniq': expected = original.uniq(values); break;
    default: throw Error('unrecognized original export '+item.export);
  }
  observed.push({id:item.id,expected});
}
process.stdout.write(JSON.stringify(observed));
