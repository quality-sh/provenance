import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { compare, median, medianInterval, parseTiming, releaseResult, renderReport } from './timing-report.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const fixture = await readFile(join(here, 'fixtures/timing-1beedc23.txt'), 'utf8');

function rows(entries) {
  return new Map(Object.entries(entries));
}

test('the parser reads the four sections of the 0.3.1 output', () => {
  const parsed = parseTiming(fixture);
  const stores = new Set([...parsed.keys()].map(key => key.split(' ')[0]));
  assert.deepEqual([...stores], ['seeded_queries', 'cache_seeded', 'cache_owner_rows', 'repository_state']);
  const operations = [...parsed.keys()].filter(key => key.startsWith('repository_state ') && key.endsWith(' summary'));
  assert.deepEqual(operations.map(key => key.split(' ')[1]), ['get', 'neighbors', 'trace', 'impact', 'evidence', 'search', 'resolve_symbol']);
  assert.equal(parsed.get('repository_state catch_up_ms'), 10.3);
  assert.equal(parsed.get('seeded_queries stale summary'), 13.1);
  assert.equal(parsed.size, 40);
});

test('the parser ignores per-case rows and test harness lines', () => {
  const parsed = parseTiming([
    'running 1 test',
    'Timing over repository_state: operation request served_ms',
    'get id=rule_a 1.2',
    'get summary 1.4',
    'search query="summary 9" 3.0',
    'catch_up_ms 10.3',
    'test operations::queries::tests::comparison::timing_comparison_rows ... ok',
  ].join('\n'));
  assert.deepEqual([...parsed], [['repository_state get summary', 1.4], ['repository_state catch_up_ms', 10.3]]);
});

test('the median of an even count is the mean of the two middle values', () => {
  assert.equal(median([3, 1, 2]), 2);
  assert.equal(median([4, 1, 3, 2]), 2.5);
});

test('the interval uses the order statistics that give at least 95% coverage', () => {
  const values = Array.from({ length: 15 }, (_, index) => index + 1);
  // P(Binomial(15, 1/2) <= 3) = 0.0176 and P(<= 4) = 0.0592, so k = 4.
  assert.deepEqual(medianInterval(values.toReversed()), [4, 12]);
  // P(Binomial(6, 1/2) = 0) = 0.0156, so k = 1: the minimum and the maximum.
  assert.deepEqual(medianInterval([6, 1, 5, 2, 4, 3]), [1, 6]);
  // P(Binomial(20, 1/2) <= 5) = 0.0207 and P(<= 6) = 0.0577, so k = 6.
  assert.deepEqual(medianInterval(Array.from({ length: 20 }, (_, index) => index)), [5, 14]);
  assert.equal(medianInterval([1, 2, 3, 4, 5]), null);
});

test('compare pairs each run with its partner and reports missing rows', () => {
  const pairs = [1, 2, 3, 4, 5, 6].map(value => ({
    base: rows({ 'repository_state get summary': 2, 'repository_state scan_ms': 0 }),
    head: rows({ 'repository_state get summary': 2 * value, 'repository_state scan_ms': 0, 'repository_state stale summary': 1 }),
  }));
  const [get, scan, stale] = compare(pairs);
  assert.deepEqual(get, { key: 'repository_state get summary', baseMedian: 2, headMedian: 7, ratio: 3.5, interval: [1, 6] });
  assert.equal(scan.ratio, 1);
  assert.deepEqual(stale, { key: 'repository_state stale summary', missing: { base: 6, head: 0 }, runs: 6 });
});

test('the report names the commits and flags a changed harness', () => {
  const pairs = Array.from({ length: 6 }, () => ({ base: parseTiming(fixture), head: parseTiming(fixture) }));
  const info = { baseline: 'aaa', candidate: 'bbb', input: 'ccc', runner: 'ubuntu24 20261027.1', pairs: 6 };
  const plain = renderReport(compare(pairs), { ...info, harnessChanged: false });
  assert.match(plain, /Baseline: `aaa`/);
  assert.match(plain, /Candidate: `bbb`/);
  assert.match(plain, /Input state: `ccc`/);
  assert.match(plain, /Runner image: ubuntu24 20261027\.1/);
  assert.match(plain, /\| `repository_state catch_up_ms` \| 10\.3 \| 10\.3 \| 1\.00 \| 1\.00 to 1\.00 \|/);
  assert.doesNotMatch(plain, /Warning/);
  const changed = renderReport(compare(pairs), { ...info, harnessChanged: true });
  assert.match(changed, /Warning:\*\* the comparison test files differ/);
});

function statistics(key, interval, baseMedian, headMedian) {
  return { key, baseMedian, headMedian, ratio: (interval[0] + interval[1]) / 2, interval };
}

test('a gated row fails only above twice and 5 ms', () => {
  const key = 'repository_state get summary';
  assert.equal(releaseResult(statistics(key, [2.3, 2.9], 4, 10)), 'Fail');
  assert.equal(releaseResult(statistics(key, [2.3, 2.9], 2, 5)), 'Pass');
  assert.equal(releaseResult(statistics(key, [1.8, 2.6], 20, 60)), 'Pass');
  assert.equal(releaseResult(statistics('repository_state catch_up_ms', [2.3, 2.9], 4, 10)), 'Fail');
  assert.equal(releaseResult({ key, missing: { base: 6, head: 0 }, runs: 6 }), 'Pass');
  assert.equal(releaseResult({ key, missing: { base: 0, head: 6 }, runs: 6 }), 'Fail');
  assert.equal(releaseResult({ key, missing: { base: 2, head: 0 }, runs: 6 }), 'Fail');
  assert.equal(releaseResult({ key, missing: { base: 6, head: 1 }, runs: 6 }), 'Fail');
});

test('ungated rows never fail', () => {
  for (const key of ['repository_state scan_ms', 'repository_state rebuild_ms', 'seeded_queries stale summary', 'cache_seeded catch_up_ms']) {
    assert.equal(releaseResult(statistics(key, [10, 20], 10, 200)), 'Report only');
  }
});

async function writeRuns(directory, base, head) {
  const pairs = [];
  for (let index = 1; index <= 6; index += 1) {
    await writeFile(join(directory, `base-${index}.txt`), base);
    await writeFile(join(directory, `head-${index}.txt`), head);
    pairs.push({ base: `base-${index}.txt`, head: `head-${index}.txt` });
  }
  await writeFile(join(directory, 'runs.json'), JSON.stringify({ pairs }));
}

test('the release gate sets the exit status and only reports without it', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'timing-report-'));
  try {
    const section = 'Timing over repository_state: operation request served_ms\n';
    await writeRuns(directory, `${section}get summary 4.0\nrebuild_ms 100.0\n`, `${section}get summary 10.0\nrebuild_ms 900.0\n`);
    const report = script => spawnSync(process.execPath, [join(here, 'timing-report.mjs'), '--runs', directory, ...script], { encoding: 'utf8' });
    const gated = report(['--gate', 'release']);
    assert.equal(gated.status, 1, gated.stderr);
    assert.match(gated.stdout, /Release gate: \*\*fail\*\* \(1 of 1 gated rows\)/);
    assert.match(gated.stdout, /\| `repository_state get summary` \| 4\.0 \| 10\.0 \| 2\.50 \| 2\.50 to 2\.50 \| Fail \|/);
    assert.match(gated.stdout, /\| `repository_state rebuild_ms` .* Report only \|/);
    const plain = report([]);
    assert.equal(plain.status, 0, plain.stderr);
    assert.doesNotMatch(plain.stdout, /Release gate/);
    await writeRuns(directory, `${section}get summary 4.0\n`, `${section}get summary 4.4\n`);
    const passing = report(['--gate', 'release']);
    assert.equal(passing.status, 0, passing.stderr);
    assert.match(passing.stdout, /Release gate: pass\./);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

async function gateOnRuns(bases, heads) {
  const directory = await mkdtemp(join(tmpdir(), 'timing-report-'));
  try {
    const pairs = [];
    for (let index = 0; index < bases.length; index += 1) {
      await writeFile(join(directory, `base-${index}.txt`), bases[index]);
      await writeFile(join(directory, `head-${index}.txt`), heads[index]);
      pairs.push({ base: `base-${index}.txt`, head: `head-${index}.txt` });
    }
    await writeFile(join(directory, 'runs.json'), JSON.stringify({ pairs }));
    return spawnSync(process.execPath, [join(here, 'timing-report.mjs'), '--runs', directory, '--gate', 'release'], { encoding: 'utf8' });
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
}

const section = 'Timing over repository_state: operation request served_ms\n';
const withGet = `${section}catch_up_ms 10.0\nget summary 4.0\n`;
const withoutGet = `${section}catch_up_ms 10.0\n`;

test('the release gate fails a gated row that some baseline runs do not print', async () => {
  const bases = [withGet, withoutGet, withGet, withoutGet, withGet, withGet];
  const result = await gateOnRuns(bases, Array(6).fill(withGet));
  assert.equal(result.status, 1, result.stderr);
  assert.match(result.stdout, /Release gate: \*\*fail\*\* \(1 of 2 gated rows\)/);
  assert.match(result.stdout, /`repository_state get summary` is missing in base in 2 of 6 runs\./);
  assert.match(result.stdout, /\| `repository_state get summary` \| \| \| \| missing in base in 2 of 6 runs \| Fail \|/);
});

test('the release gate fails a gated row that some candidate runs do not print', async () => {
  const heads = [withGet, withGet, withGet, withGet, withGet, withoutGet];
  const result = await gateOnRuns(Array(6).fill(withGet), heads);
  assert.equal(result.status, 1, result.stderr);
  assert.match(result.stdout, /`repository_state get summary` is missing in head in 1 of 6 runs\./);
});

test('the release gate passes a gated row that only the candidate prints', async () => {
  const result = await gateOnRuns(Array(6).fill(withoutGet), Array(6).fill(withGet));
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /Release gate: pass\./);
  assert.match(result.stdout, /\| `repository_state get summary` \| \| \| \| missing in base \| Pass \|/);
  assert.doesNotMatch(result.stdout, /is missing in/);
});
