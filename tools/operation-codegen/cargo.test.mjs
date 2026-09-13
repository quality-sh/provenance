import assert from 'node:assert/strict';
import test from 'node:test';
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { join, sep } from 'node:path';
import { executableFromMessages, buildBinary } from './cargo.mjs';

const artifact = (name, executable, test = false) => JSON.stringify({
  reason: 'compiler-artifact', target: { name, kind: ['bin'] }, profile: { test }, executable,
});
test('uses the executable Cargo reports for a custom target directory', () => {
  const expected = '/shared/build cache/host/debug/provenance-codegen.exe';
  const messages = [
    artifact('other', '/shared/other'),
    artifact('provenance-codegen', '/shared/test-harness', true),
    artifact('provenance-codegen', expected),
    JSON.stringify({ reason: 'build-finished', success: true }),
    '',
  ].join('\n');
  assert.equal(executableFromMessages(messages, 'provenance-codegen'), expected);
});
test('fails if Cargo did not report the requested runnable binary', () => {
  assert.throws(() => executableFromMessages(artifact('provenance-codegen', null), 'provenance-codegen'), /provenance-codegen/);
});

test('runs a real Cargo binary built outside the checkout target directory', async t => {
  const root = await mkdtemp(join(tmpdir(), 'provenance-cargo-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  await mkdir(join(root, 'src'));
  await writeFile(join(root, 'Cargo.toml'), '[package]\nname = "generator-fixture"\nversion = "0.0.0"\nedition = "2021"\n[workspace]\n');
  await writeFile(join(root, 'src/main.rs'), 'fn main() { println!("fixture"); }');
  const target = join(root, 'custom target');
  const executable = buildBinary(root, ['--bin', 'generator-fixture', '--target-dir', target], 'generator-fixture');
  assert.ok(executable.startsWith(target + sep));
  const result = spawnSync(executable, [], { encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout.trim(), 'fixture');
});
