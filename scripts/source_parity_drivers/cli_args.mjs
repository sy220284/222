// True 2.1.88 CLI argument functions require two positional inputs.
import { readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

const original = await import(pathToFileURL(process.argv[2]).href);
const cases = JSON.parse(readFileSync(0, 'utf8'));
const observed = [];
for (const item of cases) {
  let value;
  if (item.export === 'eagerParseCliFlag') {
    value = original.eagerParseCliFlag(item.input.flag, item.input.argv);
  } else if (item.export === 'extractArgsAfterDoubleDash') {
    value = original.extractArgsAfterDoubleDash(item.input.command, item.input.args);
  } else {
    throw Error('unknown original CLI function: ' + item.export);
  }
  observed.push({id: item.id, expected: value === undefined ? null : value});
}
process.stdout.write(JSON.stringify(observed));
