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


test('rejects a generated renderer even when force staged', async t => {
  const root = await mkdtemp(join(tmpdir(), 'provenance-renderer-index-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  spawnSync('git', ['init', '-q'], { cwd: root });
  const path = 'crates/provenance-cli/review-assets-generated/host.js';
  await mkdir(join(root, 'crates/provenance-cli/review-assets-generated'), { recursive: true });
  await writeFile(join(root, path), 'generated renderer');
  const staged = spawnSync('git', ['add', '-f', path], { cwd: root });
  assert.equal(staged.status, 0);
  assert.deepEqual(checkSourceControl(root), [path]);
});

// Implementation aid: the index gate must reject generated and installed skill files.
test('rejects bundled output paths and permits root sources and unrelated skills', async t => {
  const root = await mkdtemp(join(tmpdir(), 'provenance-skills-index-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const names = ['provenance-fork-tournament', 'provenance-grounded-writing', 'provenance-shaping', 'provenance-swarm-backtrace'];
  const outputs = names.flatMap(name => ['crates/provenance-cli/skills', '.agents/skills', '.claude/skills'].map(prefix => `${prefix}/${name}/SKILL.md`));
  const sources = names.map(name => `skills/${name}/SKILL.md`);
  for (const path of [...outputs, ...sources, '.agents/skills/unrelated/SKILL.md', '.claude/skills/unrelated/SKILL.md']) {
    await mkdir(join(root, path, '..'), { recursive: true });
    await writeFile(join(root, path), 'Fixture skill.\n');
  }
  assert.equal(spawnSync('git', ['init', '-q'], { cwd: root }).status, 0);
  assert.equal(spawnSync('git', ['add', '-f', '.'], { cwd: root }).status, 0);
  assert.deepEqual(checkSourceControl(root).sort(), outputs.sort());
});
