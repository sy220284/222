// Execute the original TypeScript production implementation from a private source tree.
// No original code is copied into the repository or this adapter.
import { readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';
import { resolve } from 'node:path';

const SOURCE_ID = 'src/utils/slashCommandParsing.ts:FunctionDeclaration:25-60:occurrence-1';
const sourcePath = process.argv[2];
if (!sourcePath) throw new Error('original source path is required');
const source = await import(pathToFileURL(resolve(sourcePath)).href);
if (typeof source.parseSlashCommand !== 'function') {
  throw new Error('original parseSlashCommand export is unavailable');
}
const payload = JSON.parse(readFileSync(0, 'utf8'));
if (typeof payload.id !== 'string' || !payload.id || typeof payload.input?.command !== 'string') {
  throw new Error('invalid slash command parity input');
}
const parsed = source.parseSlashCommand(payload.input.command);
process.stdout.write(JSON.stringify({
  schema: 'source-rust-observation/v1',
  case_id: payload.id,
  executed_source_ids: [SOURCE_ID],
  outcome: { parsed },
  errors: [], state: {}, calls: ['parseSlashCommand'], permissions: [],
  events: ['started', 'finished'], side_effects: [], persistence: {},
  cancellation: { cancelled: false },
}) + '\n');
