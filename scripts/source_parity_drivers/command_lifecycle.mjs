// Reviewable original-source driver for commandLifecycle.ts.
// Source is passed from the submitter's private 2.1.88 workspace, never copied.
import {readFileSync} from 'node:fs';
import {pathToFileURL} from 'node:url';

const originalPath = process.argv[2];
if (!originalPath) throw Error('missing original TypeScript module path');
const original = await import(pathToFileURL(originalPath).href);
const cases = JSON.parse(readFileSync(0, 'utf8'));
const output = [];
for (const item of cases) {
  const observed = [];
  original.setCommandLifecycleListener(null);
  try {
    for (const step of item.input.steps) {
      if (step.op === 'set') {
        const listener = step.listener;
        original.setCommandLifecycleListener((uuid, state) => {
          observed.push({listener, uuid, state});
        });
      } else if (step.op === 'clear') {
        original.setCommandLifecycleListener(null);
      } else if (step.op === 'notify') {
        original.notifyCommandLifecycle(step.uuid, step.state);
      } else {
        throw Error('unknown source scenario operation: ' + step.op);
      }
    }
    output.push({id: item.id, expected: observed});
  } finally {
    original.setCommandLifecycleListener(null);
  }
}
process.stdout.write(JSON.stringify(output));
