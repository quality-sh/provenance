#!/usr/bin/env node
// Reads the paired timing outputs that paired-timing.mjs saves and prints a
// Markdown report: per row, the median of each side, the median of the
// per-pair ratios head/base and a 95% order-statistic interval for it.
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { parseArgs } from 'node:util';

const SECTION = /^Timing over (\S+):/;
const SUMMARY = /^(\S+) summary (\d+(?:\.\d+)?)$/;
const TOTAL = /^(scan_ms|rebuild_ms|catch_up_ms) (\d+(?:\.\d+)?)$/;

// One timing output as a map from `<store> <row>` to milliseconds. Per-case
// rows and test harness lines are not rows.
export function parseTiming(text) {
  const rows = new Map();
  let store = null;
  for (const line of text.split(/\r?\n/).map(value => value.trim())) {
    const section = SECTION.exec(line);
    if (section) {
      store = section[1];
      continue;
    }
    if (store === null) continue;
    const summary = SUMMARY.exec(line);
    const total = TOTAL.exec(line);
    if (summary) rows.set(`${store} ${summary[1]} summary`, Number(summary[2]));
    else if (total) rows.set(`${store} ${total[1]}`, Number(total[2]));
  }
  return rows;
}

export function median(values) {
  const sorted = [...values].sort((a, b) => a - b);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 1 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2;
}

// The distribution-free interval for a median: the k-th smallest and k-th
// largest values, with the largest k for which P(Binomial(n, 1/2) < k) is at
// most 2.5%. Fewer than six values give no 95% interval.
export function medianInterval(values) {
  const sorted = [...values].sort((a, b) => a - b);
  const n = sorted.length;
  let k = 0;
  let below = 0;
  let probability = 0.5 ** n;
  while (k < n && below + probability <= 0.025) {
    below += probability;
    probability *= (n - k) / (k + 1);
    k += 1;
  }
  if (k === 0) return null;
  return [sorted[k - 1], sorted[n - k]];
}

function ratio(head, base) {
  if (base === 0) return head === 0 ? 1 : Infinity;
  return head / base;
}

// One statistics row per `<store> <row>` that both sides print in every pair.
export function compare(pairs) {
  const keys = [];
  for (const { base, head } of pairs) {
    for (const key of [...base.keys(), ...head.keys()]) if (!keys.includes(key)) keys.push(key);
  }
  return keys.map(key => {
    const present = pairs.filter(({ base, head }) => base.has(key) && head.has(key));
    if (present.length !== pairs.length) {
      const missing = {
        base: pairs.filter(({ base }) => !base.has(key)).length,
        head: pairs.filter(({ head }) => !head.has(key)).length,
      };
      return { key, missing, runs: pairs.length };
    }
    const base = present.map(pair => pair.base.get(key));
    const head = present.map(pair => pair.head.get(key));
    const ratios = present.map(pair => ratio(pair.head.get(key), pair.base.get(key)));
    return { key, baseMedian: median(base), headMedian: median(head), ratio: median(ratios), interval: medianInterval(ratios) };
  });
}

// The release criterion from docs/release.md on paired data: a gated row
// fails when the lower bound of its ratio interval is above 2 and its head
// median is more than 5 ms above the base median. Gated rows are the
// repository_state operation summaries and catch_up_ms.
export const RELEASE_RATIO = 2;
export const RELEASE_MS = 5;

export function isGated(key) {
  return /^repository_state (\S+ summary|catch_up_ms)$/.test(key);
}

// A row new in the candidate is in no base run and in every head run.
function candidateOnly(row) {
  return row.missing.base === row.runs && row.missing.head === 0;
}

// The sides and run counts that do not print a row, for example
// `base in 2 of 6 runs`. A side that prints the row in no run is only named.
export function describeMissing(row) {
  return ['base', 'head']
    .filter(side => row.missing[side] > 0)
    .map(side => (row.missing[side] === row.runs ? side : `${side} in ${row.missing[side]} of ${row.runs} runs`))
    .join(' and ');
}

export function releaseResult(row) {
  if (!isGated(row.key)) return 'Report only';
  // A row new in the candidate is not a regression; a row that any run of
  // either side did not print is not a measured pass.
  if (row.missing) return candidateOnly(row) ? 'Pass' : 'Fail';
  const fails = row.interval[0] > RELEASE_RATIO && row.headMedian - row.baseMedian > RELEASE_MS;
  return fails ? 'Fail' : 'Pass';
}

function ms(value) {
  return value.toFixed(1);
}

function times(value) {
  return Number.isFinite(value) ? value.toFixed(2) : 'inf';
}

export function renderReport(rows, info, gate = 'none') {
  const lines = ['# Paired timing report', ''];
  lines.push(`- Baseline: \`${info.baseline}\``);
  lines.push(`- Candidate: \`${info.candidate}\``);
  lines.push(`- Input state: \`${info.input}\``);
  lines.push(`- Runner image: ${info.runner}`);
  lines.push(`- Pairs: ${info.pairs}, in alternate order`);
  lines.push('');
  if (info.harnessChanged) {
    lines.push('> **Warning:** the comparison test files differ between the two commits, so the two sides do not time the same steps.', '');
  }
  const release = gate === 'release';
  if (release) {
    const failed = rows.filter(row => releaseResult(row) === 'Fail').length;
    const gated = rows.filter(row => isGated(row.key)).length;
    lines.push(`Release gate: ${failed === 0 ? 'pass' : `**fail** (${failed} of ${gated} gated rows)`}. A gated row fails when the lower bound of its ratio interval is above ${RELEASE_RATIO} and its head median is more than ${RELEASE_MS} ms above the base median.`, '');
    const unmeasured = rows.filter(row => row.missing && releaseResult(row) === 'Fail');
    for (const row of unmeasured) lines.push(`- Fail: \`${row.key}\` is missing in ${describeMissing(row)}.`);
    if (unmeasured.length > 0) lines.push('');
  }
  lines.push(`| Row | Base median ms | Head median ms | Ratio head/base | 95% interval |${release ? ' Gate |' : ''}`);
  lines.push(`|---|---:|---:|---:|---|${release ? '---|' : ''}`);
  for (const row of rows) {
    const result = release ? ` ${releaseResult(row)} |` : '';
    if (row.missing) {
      lines.push(`| \`${row.key}\` | | | | missing in ${describeMissing(row)} |${result}`);
      continue;
    }
    const interval = row.interval ? `${times(row.interval[0])} to ${times(row.interval[1])}` : 'too few pairs';
    lines.push(`| \`${row.key}\` | ${ms(row.baseMedian)} | ${ms(row.headMedian)} | ${times(row.ratio)} | ${interval} |${result}`);
  }
  return `${lines.join('\n')}\n`;
}

export async function readRuns(directory) {
  const runs = JSON.parse(await readFile(join(directory, 'runs.json'), 'utf8'));
  const pairs = [];
  for (const pair of runs.pairs) {
    pairs.push({
      base: parseTiming(await readFile(join(directory, pair.base), 'utf8')),
      head: parseTiming(await readFile(join(directory, pair.head), 'utf8')),
    });
  }
  return pairs;
}

async function main() {
  const { values } = parseArgs({
    options: {
      runs: { type: 'string' },
      gate: { type: 'string', default: 'none' },
      baseline: { type: 'string', default: 'unknown' },
      candidate: { type: 'string', default: 'unknown' },
      input: { type: 'string', default: 'unknown' },
      runner: { type: 'string', default: 'unknown' },
      'harness-changed': { type: 'boolean', default: false },
    },
  });
  if (!values.runs) throw new Error('--runs <dir> is necessary');
  if (!['none', 'release'].includes(values.gate)) throw new Error(`unknown gate: ${values.gate}`);
  const pairs = await readRuns(values.runs);
  const rows = compare(pairs);
  if (values.gate === 'release' && rows.some(row => !row.missing && !row.interval)) {
    throw new Error('the release gate needs at least six pairs');
  }
  process.stdout.write(renderReport(rows, { ...values, harnessChanged: values['harness-changed'], pairs: pairs.length }, values.gate));
  if (values.gate === 'release' && rows.some(row => releaseResult(row) === 'Fail')) process.exitCode = 1;
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch(error => {
    console.error(error.message);
    process.exit(2);
  });
}
