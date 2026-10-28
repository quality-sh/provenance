import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { cp, mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const names = ['provenance-fork-tournament', 'provenance-grounded-writing', 'provenance-shaping', 'provenance-swarm-backtrace'];

// Implementation aid: exercise the documented package preparation command.
test('preparation replaces stale payloads with the complete root sources', async () => {
  const fixture = await mkdtemp(join(tmpdir(), 'skill-preparation-'));
  try {
    await mkdir(join(fixture, 'tools/skills'), { recursive: true });
    await cp(join(root, 'tools/skills/prepare.mjs'), join(fixture, 'tools/skills/prepare.mjs')).catch(error => {
      if (error.code !== 'ENOENT') throw error;
    });
    for (const name of names) {
      for (const prefix of ['skills', 'crates/provenance-cli/skills']) await mkdir(join(fixture, prefix, name), { recursive: true });
      await writeFile(join(fixture, 'skills', name, 'SKILL.md'), `Complete source for ${name}.\n`);
      await writeFile(join(fixture, 'crates/provenance-cli/skills', name, 'SKILL.md'), 'Stale bytes.\n');
    }
    const result = spawnSync(process.execPath, ['tools/skills/prepare.mjs'], { cwd: fixture, encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
    for (const name of names) assert.equal(await readFile(join(fixture, 'crates/provenance-cli/skills', name, 'SKILL.md'), 'utf8'), `Complete source for ${name}.\n`);
    await rm(join(fixture, 'skills', names[2], 'SKILL.md'));
    const missing = spawnSync(process.execPath, ['tools/skills/prepare.mjs'], { cwd: fixture, encoding: 'utf8' });
    assert.notEqual(missing.status, 0, 'an incomplete source set must fail');
    assert.match(missing.stderr, /provenance-shaping/);
  } finally {
    await rm(fixture, { recursive: true, force: true });
  }
});
