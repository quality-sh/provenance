import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { chmod, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { TIMING_TEST } from './paired-timing.mjs';

const here = dirname(fileURLToPath(import.meta.url));

// A stand-in test binary: it appends its side to a shared log, checks the
// arguments, and prints one repository_state section.
async function fakeBinary(directory, side, getMs) {
  const path = join(directory, `${side}-test`);
  await writeFile(path, `#!/usr/bin/env node
const { appendFileSync } = require('node:fs');
const expected = ['--ignored', '--exact', ${JSON.stringify(TIMING_TEST)}, '--nocapture'];
if (JSON.stringify(process.argv.slice(2)) !== JSON.stringify(expected)) process.exit(3);
appendFileSync(${JSON.stringify(join(directory, 'calls.log'))}, ${JSON.stringify(side)} + ' ' + process.cwd() + '\\n');
console.log('running 1 test');
console.log('Timing over repository_state: operation request served_ms');
console.log('get summary ${getMs}');
console.log('catch_up_ms 10.0');
console.log('test ${TIMING_TEST} ... ok');
`);
  await chmod(path, 0o755);
  return path;
}

test('the runner interleaves the two binaries and the report reads its outputs', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'paired-timing-'));
  try {
    const base = await fakeBinary(directory, 'base', '2.0');
    const head = await fakeBinary(directory, 'head', '3.0');
    const out = join(directory, 'runs');
    const run = spawnSync(process.execPath, [join(here, 'paired-timing.mjs'), '--base', base, '--head', head, '--pairs', '6', '--out', out, '--base-cwd', tmpdir()], { encoding: 'utf8' });
    assert.equal(run.status, 0, run.stderr);
    const calls = (await readFile(join(directory, 'calls.log'), 'utf8')).trim().split('\n').map(line => line.split(' '));
    assert.deepEqual(calls.map(([side]) => side), ['base', 'head', 'head', 'base', 'base', 'head', 'head', 'base', 'base', 'head', 'head', 'base']);
    assert.ok(calls.filter(([side]) => side === 'base').every(([, cwd]) => cwd === tmpdir()));
    assert.ok(calls.filter(([side]) => side === 'head').every(([, cwd]) => cwd === directory));
    const runs = JSON.parse(await readFile(join(out, 'runs.json'), 'utf8'));
    assert.equal(runs.pairs.length, 6);
    assert.deepEqual(runs.pairs[1].order, ['head', 'base']);
    assert.match(await readFile(join(out, runs.pairs[1].head), 'utf8'), /get summary 3\.0/);

    const report = spawnSync(process.execPath, [join(here, 'timing-report.mjs'), '--runs', out, '--baseline', 'aaa', '--candidate', 'bbb'], { encoding: 'utf8' });
    assert.equal(report.status, 0, report.stderr);
    assert.match(report.stdout, /Pairs: 6/);
    assert.match(report.stdout, /\| `repository_state get summary` \| 2\.0 \| 3\.0 \| 1\.50 \| 1\.50 to 1\.50 \|/);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test('the runner stops when a binary fails', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'paired-timing-'));
  try {
    const failing = join(directory, 'failing-test');
    await writeFile(failing, '#!/usr/bin/env node\nconsole.log("partial");\nprocess.exit(101);\n');
    await chmod(failing, 0o755);
    const head = await fakeBinary(directory, 'head', '3.0');
    const run = spawnSync(process.execPath, [join(here, 'paired-timing.mjs'), '--base', failing, '--head', head, '--pairs', '2', '--out', join(directory, 'runs')], { encoding: 'utf8' });
    assert.equal(run.status, 2);
    assert.match(run.stderr, /exited with status 101/);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
