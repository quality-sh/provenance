import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { checkSourceControl } from './check-source-control.mjs';

test('rejects generated files even when force staged, but permits handwritten templates', async t => {
  const root = await mkdtemp(join(tmpdir(), 'provenance-index-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const git = (...args) => {
    const result = spawnSync('git', args, { cwd: root, encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
  };
  git('init', '-q');
  await mkdir(join(root, 'tools/operation-codegen/templates'), { recursive: true });
  await writeFile(join(root, 'tools/operation-codegen/templates/runtime.ts'), '// Generated file template');
  git('add', '.');
  assert.deepEqual(checkSourceControl(root), []);
  await mkdir(join(root, 'contracts/operations'), { recursive: true });
  await writeFile(join(root, '.gitignore'), 'contracts/operations/\n');
  await writeFile(join(root, 'contracts/operations/openapi.json'), '{}');
  git('add', '-f', 'contracts/operations/openapi.json');
  assert.deepEqual(checkSourceControl(root), ['contracts/operations/openapi.json']);
});
