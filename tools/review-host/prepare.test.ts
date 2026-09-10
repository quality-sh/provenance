import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtemp, writeFile, rm, access } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

test('a changed archive cannot create composition output', async t => {
  const work = await mkdtemp(join(tmpdir(), 'review-archive-'));
  t.after(() => rm(work, { recursive: true, force: true }));
  const archive = join(work, 'changed.tar.gz');
  const output = join(work, 'output');
  await writeFile(archive, 'not the approved archive');
  const result = spawnSync(process.execPath, [fileURLToPath(new URL('./prepare.ts', import.meta.url)), 'unused-sdk', output], {
    encoding: 'utf8', env: { ...process.env, PROVENANCE_REVIEW_ARCHIVE: archive },
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /checksum mismatch/);
  await assert.rejects(access(output));
});
