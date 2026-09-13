import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

test('runtime-only release generation creates ignored source without rewriting metadata', t => {
  const root = mkdtempSync(join(tmpdir(), 'provenance-generated-source-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const targets = join(root, 'targets.json');
  const manifest = join(root, 'package.json');
  const runtime = join(root, 'runtime.ts');
  writeFileSync(targets, JSON.stringify([{ npm: { name: '@test/engine', os: ['linux'], cpu: ['x64'] } }]));
  const metadata = '{"version":"1.0.0","optionalDependencies":{"@test/engine":"1.0.0"}}';
  writeFileSync(manifest, metadata);
  const run = () => spawnSync(process.execPath, [fileURLToPath(new URL('./generate-release-consumers.js', import.meta.url)), '--runtime-only', '--targets', targets, '--package', manifest, '--runtime', runtime], { encoding: 'utf8' });
  let result = run();
  assert.equal(result.status, 0, result.stderr);
  assert.match(readFileSync(runtime, 'utf8'), /@test\/engine/);
  assert.equal(readFileSync(manifest, 'utf8'), metadata);
  writeFileSync(manifest, '{"version":"1.0.0","optionalDependencies":{}}');
  result = run();
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /stale/);
  assert.equal(readFileSync(manifest, 'utf8'), '{"version":"1.0.0","optionalDependencies":{}}');
});

test('source package entry points prepare ignored output without adding an install hook', () => {
  const manifest = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8'));
  for (const name of ['build', 'test:runtime', 'test:types', 'test:packed']) {
    assert.match(manifest.scripts[name], /^npm run prepare:generated && /, name);
  }
  for (const hook of ['prepare', 'install', 'postinstall']) assert.equal(manifest.scripts[hook], undefined);
  const prepare = readFileSync(new URL('./prepare-generated.js', import.meta.url), 'utf8');
  assert.match(prepare, /ensure-generated\.mjs/);
  assert.doesNotMatch(prepare, /npm|cargo/);
});
