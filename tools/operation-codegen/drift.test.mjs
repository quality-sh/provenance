import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { compareTrees } from './inventory.mjs';

for (const change of ['changed', 'missing', 'stale', 'schema', 'oversize']) {
  test(`drift check rejects ${change} in a temporary copy`, async () => {
    const root = await mkdtemp(join(tmpdir(), 'operation-drift-'));
    try {
      const expected = join(root, 'expected'), actual = join(root, 'actual');
      await Promise.all([mkdir(expected), mkdir(actual)]);
      for (const directory of [expected, actual]) {
        await writeFile(join(directory, 'model.rs'), 'pub struct Model;\n');
        await writeFile(join(directory, 'schema.json'), '{}\n');
      }
      assert.deepEqual(await compareTrees(expected, actual), []);
      if (change === 'missing') await rm(join(actual, 'model.rs'));
      if (change === 'stale') await writeFile(join(actual, 'old.rs'), 'old');
      if (change === 'changed') await writeFile(join(actual, 'model.rs'), 'pub struct Changed;');
      if (change === 'schema') await writeFile(join(actual, 'schema.json'), '{"type":"null"}');
      if (change === 'oversize') {
        for (const directory of [expected, actual]) await writeFile(join(directory, 'model.rs'), '\n'.repeat(501));
      }
      assert.ok((await compareTrees(expected, actual)).length > 0);
    } finally { await rm(root, { recursive: true, force: true }); }
  });
}
