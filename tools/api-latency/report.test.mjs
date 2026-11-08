import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { buildReport, median, percentile, rank, renderMarkdown, summarize, table } from './report.mjs';

const here = dirname(fileURLToPath(import.meta.url));

const meta = { commit: 'abc', input: 'abc', runner: 'ubuntu24 1', binary: 'provenance 0.3.2', reads: 20, writes: 10, added: [] };

test('the median of an even count is the mean of the two middle values', () => {
  assert.equal(median([3, 1, 2]), 2);
  assert.equal(median([4, 1, 3, 2]), 2.5);
  assert.equal(median([]), null);
});

test('p95 is the nearest-rank value', () => {
  const values = Array.from({ length: 20 }, (_, index) => index + 1);
  // 95% of 20 is 19, so the 19th smallest value.
  assert.equal(percentile(values.toReversed(), 95), 19);
  assert.equal(percentile([5], 95), 5);
  // 95% of 10 is 9.5, so the 10th smallest value.
  assert.equal(percentile(Array.from({ length: 10 }, (_, index) => index), 95), 9);
  assert.equal(percentile([], 95), null);
});

test('a row is over the limit when cold, p50 or p95 is over 200 ms', () => {
  const row = values => summarize({ way: 'HTTP', call: 'GET /rules', kind: 'read', ...values });
  assert.equal(row({ cold: 10, warm: [10, 10, 10] }).over, false);
  assert.equal(row({ cold: 200, warm: [200, 200] }).over, false);
  assert.equal(row({ cold: 201, warm: [10, 10] }).over, true);
  // The 19th of 20 values is p95: one slow value is not enough, two are.
  assert.equal(row({ cold: 10, warm: Array(19).fill(10).concat([250]) }).over, false);
  assert.equal(row({ cold: 10, warm: Array(18).fill(10).concat([250, 250]) }).over, true);
  assert.equal(row({ cold: 10, warm: [250, 250, 10] }).over, true);
  assert.deepEqual(row({ cold: 5, warm: [1, 3, 2] }), {
    way: 'HTTP', call: 'GET /rules', kind: 'read', cold: 5, p50: 2, p95: 3, repeats: 3, over: false,
  });
});

test('rows rank by warm p50, then cold, then way and call', () => {
  const rows = [
    { way: 'HTTP', call: 'b', cold: 1, p50: 5 },
    { way: 'CLI', call: 'a', cold: 9, p50: 5 },
    { way: 'MCP', call: 'c', cold: 1, p50: 50 },
    { way: 'CLI', call: 'start', cold: 30, p50: null },
    { way: 'CLI', call: 'z', cold: 1, p50: 5 },
    { way: 'CLI', call: 'y', cold: 1, p50: 5 },
  ];
  assert.deepEqual(rank(rows).map(row => row.call), ['c', 'start', 'a', 'y', 'z', 'b']);
});

test('the table marks each row over the limit and escapes the call', () => {
  const rows = [
    { way: 'CLI', call: 'GET /a|b', cold: 300, p50: 250.04, p95: 260, repeats: 20, over: true },
    { way: 'HTTP', call: 'GET /c', cold: 3, p50: 2, p95: 2.5, repeats: 20, over: false },
  ];
  const lines = table(rows).split('\n');
  assert.equal(lines.length, 4);
  assert.equal(lines[2], '| 1 | 🔴 | CLI | GET /a\\|b | 300.0 | 250.0 | 260.0 | 20 |');
  assert.equal(lines[3], '| 2 |  | HTTP | GET /c | 3.0 | 2.0 | 2.5 | 20 |');
  assert.equal(table(rows, 1).split('\n').length, 3);
});

test('the report ranks the rows and lists the calls that were not measured', () => {
  const report = buildReport({
    meta,
    calls: [
      { way: 'HTTP', call: 'GET /fast', kind: 'read', cold: 4, warm: [1, 2, 3] },
      { way: 'CLI', call: 'GET /slow', kind: 'read', cold: 400, warm: [300, 310, 320] },
    ],
    unmeasured: [
      { way: 'MCP', call: 'GET /b', reason: 'the cold call failed: invalid_input' },
      { way: 'HTTP', call: 'GET /a', reason: 'the state has no record' },
    ],
  });
  assert.deepEqual(report.rows.map(row => row.call), ['GET /slow', 'GET /fast']);
  assert.deepEqual(report.unmeasured.map(item => item.way), ['HTTP', 'MCP']);
  const markdown = renderMarkdown(report);
  assert.match(markdown, /2 timed rows \(CLI 1, HTTP 1\)\. 1 rows are over 200 ms\. 2 calls were not measured\./);
  assert.match(markdown, /\| HTTP \| GET \/a \| the state has no record \|/);
  assert.match(markdown, /Reads repeat 20 times after the cold call; writes and the reads after them repeat 10 times\./);
});

test('the report names the records that the run added to the input', () => {
  const report = buildReport({ meta: { ...meta, added: [{ call: 'GET /boundaries/{id}', path: '/boundaries/bench_0' }] }, calls: [], unmeasured: [] });
  const markdown = renderMarkdown(report);
  assert.match(markdown, /\| GET \/boundaries\/\{id\} \| \/boundaries\/bench_0 \|/);
  assert.match(markdown, /## Calls that were not measured\n\nNone\./);
});

test('the command writes the Markdown and JSON reports', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'api-latency-report-'));
  try {
    const input = join(directory, 'results.json');
    await writeFile(input, JSON.stringify({ meta, calls: [{ way: 'MCP', call: 'tool get', kind: 'tool', cold: 9, warm: [3] }], unmeasured: [] }));
    const result = spawnSync(process.execPath, [join(here, 'report.mjs'), '--input', input, '--out', join(directory, 'out')], { encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /\| 1 \|  \| MCP \| tool get \| 9\.0 \| 3\.0 \| 3\.0 \| 1 \|/);
    assert.equal(await readFile(join(directory, 'out/report.md'), 'utf8'), result.stdout);
    const json = JSON.parse(await readFile(join(directory, 'out/report.json'), 'utf8'));
    assert.equal(json.limit_ms, 200);
    assert.equal(json.rows[0].call, 'tool get');
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
