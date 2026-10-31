// Execute the ORIGINAL 2.1.88 TypeScript class in the contributor's workspace.
// Original constructor parameter properties require Node 22's transform-types.
import { readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';
const original = await import(pathToFileURL(process.argv[2]).href);
const cases = JSON.parse(readFileSync(0, 'utf8'));
const results = [];
for (const item of cases) {
  const buffer = new original.CircularBuffer(item.input.capacity);
  const observations = [];
  for (const step of item.input.steps) {
    let value = null;
    switch (step.op) {
      case 'add': buffer.add(step.value); break;
      case 'addAll': buffer.addAll(step.values); break;
      case 'getRecent': value = buffer.getRecent(step.count); break;
      case 'toArray': value = buffer.toArray(); break;
      case 'length': value = buffer.length(); break;
      case 'clear': buffer.clear(); break;
      default: throw Error('unrecognized ORIGINAL method: ' + step.op);
    }
    observations.push({op:step.op,value,snapshot:buffer.toArray(),length:buffer.length()});
  }
  results.push({id:item.id,expected:observations});
}
process.stdout.write(JSON.stringify(results));
