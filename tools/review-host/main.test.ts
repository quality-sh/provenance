import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

test('the current review excludes terminal records', async () => {
  const source = await readFile(fileURLToPath(new URL('./main.ts', import.meta.url)), 'utf8');
  assert.match(
    source,
    /getRequirementDocument\(\{ id, exclude_terminal: true, limit: 50, cursor \}\)/,
  );
});
