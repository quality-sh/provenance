import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync, mkdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { evaluate, parseMarker, watchedValues, WATCHED, MARKER_PATH } from './check.mjs';

const VERSION_FILE = 'crates/provenance-core/src/protocol.rs';
const GOOD_MARKER = 'authorized-by: Ben\ndate: 2026-09-13\nreason: wire restructure lands with the coordinated release\n';

function files(version) {
  return {
    [VERSION_FILE]: `pub const SDK_PROTOCOL_VERSION: u32 = ${version};\n`,
    'crates/provenance-core/src/model/ideation/lifecycle/aggregate_validation.rs': 'pub const SUPPORTED_SCHEMA_VERSION: SchemaVersion = SchemaVersion(2);\n',
    'crates/provenance-core/src/review.rs': 'pub const REVIEW_SCHEMA_VERSION: SchemaVersion = SchemaVersion(3);\n',
    'crates/provenance-store/src/operations/stamp.rs': 'pub const READ_DERIVATION: u32 = 3;\n',
    'packages/provenance/src/protocol.ts': 'export const STATE_SCHEMA_VERSION = 2;\n',
    'Cargo.toml': '[workspace.package]\nversion = "0.2.3"\n',
    'packages/provenance/package.json': '{\n  "version": "0.2.3"\n}\n',
    'packages/create-provenance/package.json': '{\n  "version": "0.2.3"\n}\n',
  };
}

test('the watched set binds every compatibility axis the contract declares', () => {
  const keys = WATCHED.map(entry => entry.key).join('|');
  for (const axis of ['wire', 'state', 'review_journal', 'read_derivation']) assert.ok(keys.includes(axis), axis);
  for (const path of ['packages/provenance/package.json', 'packages/create-provenance/package.json', 'Cargo.toml']) {
    assert.ok(WATCHED.some(entry => entry.path === path), path);
  }
});

test('watched values are extracted and missing files read as absent', () => {
  const values = watchedValues(files(9));
  assert.equal(values['wire (SDK_PROTOCOL_VERSION)'], '9');
  assert.equal(values['workspace crate version'], '0.2.3');
  assert.equal(values['packages/provenance version'], '0.2.3');
  const partial = watchedValues({});
  assert.ok(Object.values(partial).every(value => value === null));
});

test('an unchanged compatibility surface needs no marker', () => {
  const values = watchedValues(files(9));
  const result = evaluate({ base: values, head: values, baseMarker: null, headMarker: null });
  assert.equal(result.authorized, true);
  assert.deepEqual(result.changes, []);
});

test('a changed value without a marker fails', () => {
  const result = evaluate({ base: watchedValues(files(9)), head: watchedValues(files(10)), baseMarker: null, headMarker: null });
  assert.equal(result.authorized, false);
  assert.equal(result.changes.length, 1);
  assert.deepEqual(result.changes[0], { key: 'wire (SDK_PROTOCOL_VERSION)', from: '9', to: '10' });
  assert.ok(result.violations.some(v => /marker file is absent/.test(v)));
});

test('a fresh valid marker authorizes a change', () => {
  const result = evaluate({
    base: watchedValues(files(9)),
    head: watchedValues(files(10)),
    baseMarker: null,
    headMarker: GOOD_MARKER,
  });
  assert.equal(result.authorized, true);
});

test('a stale marker carried over from the base authorizes nothing', () => {
  const result = evaluate({
    base: watchedValues(files(9)),
    head: watchedValues(files(10)),
    baseMarker: GOOD_MARKER,
    headMarker: GOOD_MARKER,
  });
  assert.equal(result.authorized, false);
  assert.ok(result.violations.some(v => /unchanged from the base/.test(v)));
});

test('marker parsing demands authorizer, calendar date, and reason', () => {
  assert.equal(parseMarker(GOOD_MARKER).ok, true);
  for (const broken of ['', 'authorized-by: Ben\n', 'authorized-by: Ben\ndate: soon\nreason: because\n', 'authorized-by: Ben\ndate: 2026-02-31\nreason: because\n', 'authorized-by: Ben\ndate: 2026-09-13\n']) {
    const parsed = parseMarker(broken);
    assert.equal(parsed.ok, false, JSON.stringify(broken));
  }
});

test('end to end: clean pass, unmarked bump fails, marked bump passes, stale marker fails', { timeout: 120000 }, t => {
  const git = (repo, ...args) => {
    const run = spawnSync('git', ['-C', repo, ...args], { encoding: 'utf8' });
    assert.equal(run.status, 0, run.stderr);
    return run.stdout.trim();
  };
  const root = spawnSync('git', ['rev-parse', '--show-toplevel'], { encoding: 'utf8' }).stdout.trim();
  assert.ok(root, 'git repository expected for the integration test');

  const repo = mkdtempSync(join(tmpdir(), 'compat-gate-'));
  t.after(() => rmSync(repo, { recursive: true, force: true }));
  git(repo, 'init', '-q');
  git(repo, 'config', 'user.email', 'gate@example.invalid');
  git(repo, 'config', 'user.name', 'Gate Test');
  for (const [path, content] of Object.entries(files(9))) {
    mkdirSync(dirname(join(repo, path)), { recursive: true });
    writeFileSync(join(repo, path), content);
  }
  git(repo, 'add', '-A');
  git(repo, 'commit', '-qm', 'base');
  const base = git(repo, 'rev-parse', 'HEAD');

  const run = () => spawnSync(process.execPath, [join(root, 'tools', 'compat-gate', 'check.mjs'), '--base', base, '--head', 'HEAD'], { cwd: repo, encoding: 'utf8' });

  // Identical content: pass with no marker.
  let result = run();
  assert.equal(result.status, 0, result.stderr + result.stdout);
  assert.ok(result.stdout.includes('no compatibility change'));

  // Bump the wire version without a marker: fail.
  writeFileSync(join(repo, VERSION_FILE), files(10)[VERSION_FILE]);
  git(repo, 'add', '-A');
  git(repo, 'commit', '-qm', 'unmarked bump');
  result = run();
  assert.equal(result.status, 1, result.stdout);
  assert.ok(result.stderr.includes('Compatibility gate failed'), result.stderr);

  // A fresh human marker in the same change: pass.
  writeFileSync(join(repo, MARKER_PATH), GOOD_MARKER);
  git(repo, 'add', '-A');
  git(repo, 'commit', '-qm', 'marked bump');
  result = run({ COMPAT_BASE: base });
  assert.equal(result.status, 0, result.stderr + result.stdout);

  // A later bump reusing the unchanged marker: fail.
  const markedTree = git(repo, 'rev-parse', 'HEAD');
  writeFileSync(join(repo, VERSION_FILE), files(11)[VERSION_FILE]);
  git(repo, 'add', '-A');
  git(repo, 'commit', '-qm', 'replay attempt');
  const replayBase = spawnSync('git', ['-C', repo, 'rev-parse', `${markedTree}^{commit}`], { encoding: 'utf8' }).stdout.trim();
  const replay = spawnSync(process.execPath, [join(root, 'tools', 'compat-gate', 'check.mjs'), '--base', replayBase, '--head', 'HEAD'], { cwd: repo, encoding: 'utf8' });
  assert.equal(replay.status, 1, replay.stdout);
  assert.ok(replay.stderr.includes('unchanged from the base'), replay.stderr);
});
