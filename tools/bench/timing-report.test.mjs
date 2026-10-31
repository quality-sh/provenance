import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { compare, median, medianInterval, parseTiming, renderReport } from './timing-report.mjs';

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
  assert.deepEqual(stale, { key: 'repository_state stale summary', missing: 'base' });
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
