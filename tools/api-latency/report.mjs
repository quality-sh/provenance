// Ranks the timed calls and writes the Markdown and JSON reports.
//
//   node tools/api-latency/report.mjs --input results.json --out report-dir
//
// `results.json` is the file that run.mjs writes. The Markdown goes to
// standard output and to report-dir/report.md; the ranked rows go to
// report-dir/report.json.
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { parseArgs } from 'node:util';

export const LIMIT_MS = 200;

export function median(values) {
  const sorted = [...values].sort((a, b) => a - b);
  if (!sorted.length) {
    return null;
  }
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2;
}

/** The nearest-rank percentile: the smallest value with at least `p` percent at or below it. */
export function percentile(values, p) {
  const sorted = [...values].sort((a, b) => a - b);
  if (!sorted.length) {
    return null;
  }
  const rank = Math.ceil((p / 100) * sorted.length);
  return sorted[Math.min(Math.max(rank, 1), sorted.length) - 1];
}

/** One ranked row from one timed call. */
export function summarize(call) {
  const p50 = median(call.warm);
  const p95 = percentile(call.warm, 95);
  const values = [call.cold, p50, p95].filter(value => value !== null && value !== undefined);
  return {
    way: call.way,
    call: call.call,
    kind: call.kind,
    cold: call.cold ?? null,
    p50,
    p95,
    repeats: call.warm.length,
    over: values.some(value => value > LIMIT_MS),
  };
}

/** Slowest first: by warm p50, then by cold, then by name. */
export function rank(rows) {
  return [...rows].sort((a, b) => (b.p50 ?? b.cold ?? 0) - (a.p50 ?? a.cold ?? 0)
    || (b.cold ?? 0) - (a.cold ?? 0)
    || a.way.localeCompare(b.way)
    || a.call.localeCompare(b.call));
}

function ms(value) {
  return value === null || value === undefined ? '' : value.toFixed(1);
}

function cell(text) {
  return String(text).replaceAll('|', '\\|').replaceAll('\n', ' ');
}

/** A Markdown table of ranked rows. `limit` keeps only the first rows. */
export function table(rows, limit = rows.length) {
  const lines = [
    '| Rank | Over 200 ms | Way in | Call | Cold (ms) | Warm p50 (ms) | Warm p95 (ms) | Repeats |',
    '| ---: | :---: | --- | --- | ---: | ---: | ---: | ---: |',
  ];
  rows.slice(0, limit).forEach((row, index) => {
    lines.push(`| ${index + 1} | ${row.over ? '🔴' : ''} | ${row.way} | ${cell(row.call)} | ${ms(row.cold)} | ${ms(row.p50)} | ${ms(row.p95)} | ${row.repeats} |`);
  });
  return lines.join('\n');
}

/** The complete report: the ranked rows and the calls that were not measured. */
export function buildReport(results) {
  const rows = rank(results.calls.map(summarize));
  const unmeasured = [...results.unmeasured].sort((a, b) => a.way.localeCompare(b.way) || a.call.localeCompare(b.call));
  return { meta: results.meta, limit_ms: LIMIT_MS, rows, unmeasured };
}

export function renderMarkdown(report) {
  const { meta, rows, unmeasured } = report;
  const over = rows.filter(row => row.over).length;
  const ways = [...new Set(rows.map(row => row.way))].map(way => `${way} ${rows.filter(row => row.way === way).length}`).join(', ');
  const lines = [
    '# API latency',
    '',
    `Commit ${meta.commit}, state from ${meta.input}, runner ${meta.runner}.`,
    `Binary: ${meta.binary}.`,
    `Reads repeat ${meta.reads} times after the cold call; writes and the reads after them repeat ${meta.writes} times.`,
    'Cold is the first call after the process starts. For the HTTP and MCP rows, the host restarts before each call.',
    'The CLI starts a new process for each call, so its warm rows include process start.',
    `A 🔴 mark shows a row where cold, p50 or p95 is over ${LIMIT_MS} ms. The rows are in order of warm p50, slowest first.`,
    '',
    `${rows.length} timed rows (${ways}). ${over} rows are over ${LIMIT_MS} ms. ${unmeasured.length} calls were not measured.`,
    '',
    table(rows),
    '',
    '## Calls that were not measured',
    '',
  ];
  if (unmeasured.length) {
    lines.push('| Way in | Call | Reason |', '| --- | --- | --- |');
    for (const item of unmeasured) {
      lines.push(`| ${item.way} | ${cell(item.call)} | ${cell(item.reason)} |`);
    }
  } else {
    lines.push('None.');
  }
  const added = meta.added ?? [];
  if (added.length) {
    lines.push(
      '',
      '## Records added to the input',
      '',
      'The state had no record for these read routes. Before the timed calls, the run made one with the catalog writes, on a clone that each way in then copies.',
      '',
      '| Read route | Record |',
      '| --- | --- |',
      ...added.map(item => `| ${cell(item.call)} | ${cell(item.path)} |`),
    );
  }
  return `${lines.join('\n')}\n`;
}

async function main() {
  const { values } = parseArgs({ options: { input: { type: 'string' }, out: { type: 'string' } } });
  const results = JSON.parse(await readFile(values.input, 'utf8'));
  const report = buildReport(results);
  const markdown = renderMarkdown(report);
  if (values.out) {
    await mkdir(values.out, { recursive: true });
    await writeFile(join(values.out, 'report.md'), markdown);
    await writeFile(join(values.out, 'report.json'), `${JSON.stringify(report, null, 2)}\n`);
  }
  process.stdout.write(markdown);
}

if (import.meta.url === `file://${process.argv[1]}`) {
  await main();
}
