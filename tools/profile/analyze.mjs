// Writes the per-row profile files, the comparison with the baseline, the
// JSON file and the Markdown summaries.
//
//   node tools/profile/analyze.mjs --folded labeled.folded --windows windows.json \
//     --out api-profile [--baseline baseline-dir] [--stats label-stats.json]
//
// labeled.folded is the inferno-collapse-perf output of label.mjs: the root
// frame of each stack is the row key. The baseline directory is the
// `api-profile` artifact of an earlier run. README.md tells what each output
// file holds.
import { existsSync } from 'node:fs';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { parseArgs } from 'node:util';
import {
  FEW_SAMPLES,
  MIN_CHANGE,
  MIN_Z,
  cell,
  compareShares,
  foldedText,
  largestChanges,
  parseFolded,
  percent,
  points,
  rowsFromWindows,
  shares,
  short,
  top,
} from './profile.mjs';

const { values: options } = parseArgs({
  options: {
    folded: { type: 'string' },
    windows: { type: 'string' },
    stats: { type: 'string' },
    baseline: { type: 'string' },
    out: { type: 'string', default: 'api-profile' },
    frequency: { type: 'string', default: '4999' },
    commit: { type: 'string', default: process.env.GITHUB_SHA ?? '' },
    head: { type: 'string', default: '' },
    'run-id': { type: 'string', default: process.env.GITHUB_RUN_ID ?? '' },
    artifact: { type: 'string', default: 'api-profile' },
  },
});

const TOP = 25;
const CHANGES = 10;
const server = process.env.GITHUB_SERVER_URL ?? 'https://github.com';
const repository = process.env.GITHUB_REPOSITORY ?? '';
const runUrl = id => (id && repository ? `${server}/${repository}/actions/runs/${id}` : '');
const frequency = Number(options.frequency);

/** Placeholder for the artifact link. The workflow puts in the real link after the upload. */
const ARTIFACT_URL = 'ARTIFACT_URL';

async function readJson(path) {
  return JSON.parse(await readFile(path, 'utf8'));
}

async function loadBaseline(directory) {
  if (!directory || !existsSync(join(directory, 'profile.json'))) {
    return null;
  }
  const profile = await readJson(join(directory, 'profile.json'));
  const rows = new Map();
  for (const row of profile.rows) {
    const path = join(directory, 'rows', `${row.file}.folded`);
    const stacks = existsSync(path) ? (parseFolded(await readFile(path, 'utf8'), { byRoot: false }).get('') ?? []) : [];
    rows.set(row.label, shares(stacks));
  }
  return { commit: profile.commit, run: profile.run, rows };
}

function rowMarkdown(row) {
  const lines = [`# ${row.label}`, ''];
  lines.push(`- Way in: ${row.way}. Call: \`${row.call}\`.`);
  lines.push(`- Samples: ${row.samples} over ${row.calls} timed calls (about ${row.cpuMsPerCall.toFixed(1)} ms of CPU for each call at ${frequency} Hz).`);
  lines.push(`- Stacks: \`${row.file}.folded\`. Flame graph: \`${row.file}.svg\`.`);
  if (row.samples < FEW_SAMPLES) {
    lines.push(`- This row has fewer than ${FEW_SAMPLES} samples, so its shares are noisy.`);
  }
  for (const [title, list] of [['Top functions by inclusive share', row.top.inclusive], ['Top functions by self share', row.top.self]]) {
    lines.push('', `## ${title}`, '', '| Share | Samples | Function |', '|---:|---:|---|');
    for (const item of list) {
      lines.push(`| ${percent(item.share)} | ${item.samples} | ${cell(item.function, 200)} |`);
    }
  }
  if (row.baseline) {
    lines.push('', '## Inclusive share change from the baseline', '');
    lines.push(`Baseline samples: ${row.baseline.samples}. A change is over the noise when it is at least ${(MIN_CHANGE * 100).toFixed(1)} points and ${MIN_Z} standard errors.`);
    for (const [title, list] of [['Grew', row.baseline.grew], ['Shrank', row.baseline.shrank]]) {
      lines.push('', `### ${title}`, '', '| Baseline | Now | Change (points) | z | Over noise | Function |', '|---:|---:|---:|---:|---|---|');
      for (const item of list) {
        lines.push(`| ${percent(item.base)} | ${percent(item.now)} | ${points(item.change)} | ${item.z.toFixed(1)} | ${item.overNoise ? 'yes' : 'no'} | ${cell(item.function, 200)} |`);
      }
    }
  }
  return `${lines.join('\n')}\n`;
}

/** The largest changes over all rows, at most two for each row. */
function rankedChanges(rows, filter) {
  const all = [];
  for (const row of rows) {
    if (!row.baseline) {
      continue;
    }
    const mine = [...row.baseline.grew, ...row.baseline.shrank].filter(filter).sort((a, b) => Math.abs(b.change) - Math.abs(a.change)).slice(0, 2);
    all.push(...mine.map(item => ({ row, item })));
  }
  return all.sort((a, b) => Math.abs(b.item.change) - Math.abs(a.item.change));
}

function changeTable(entries) {
  const lines = ['| Row | Samples (baseline, now) | Function | Inclusive share (baseline, now) | Change (points) | z |', '|---|---:|---|---:|---:|---:|'];
  for (const { row, item } of entries) {
    lines.push(`| ${row.label.replace(/\|/g, '\\|')} | ${row.baseline.samples}, ${row.samples} | ${cell(item.function)} | ${percent(item.base)}, ${percent(item.now)} | ${points(item.change)} | ${item.z.toFixed(1)} |`);
  }
  return lines;
}

function header(profile) {
  const lines = [];
  const run = profile.run.url ? `[${profile.run.id}](${profile.run.url})` : 'local';
  const head = profile.head && profile.head !== profile.commit ? ` (pull request head \`${short(profile.head)}\` merged into its base)` : '';
  lines.push(`Commit \`${short(profile.commit)}\`${head}, run ${run}. ${profile.rows.length} rows, ${profile.samples.kept ?? 'unknown'} samples at ${frequency} Hz.`);
  if (profile.baseline) {
    const base = profile.baseline;
    const baseRun = base.run?.url ? `[${base.run.id}](${base.run.url})` : 'unknown';
    lines.push(`Baseline: main commit \`${short(base.commit)}\`, run ${baseRun}.`);
  } else {
    lines.push('Baseline: none found, so this run has no comparison.');
  }
  lines.push(`Shares are percent of the CPU samples of one row, not milliseconds, because runner speed changes from run to run. A change is over the noise when it is at least ${(MIN_CHANGE * 100).toFixed(1)} points and ${MIN_Z} standard errors.`);
  return lines;
}

function changesSection(profile, limit) {
  if (!profile.baseline) {
    return [];
  }
  const lines = ['', '### Largest inclusive share changes', ''];
  const over = rankedChanges(profile.rows, item => item.overNoise).slice(0, limit);
  if (over.length) {
    lines.push(...changeTable(over));
  } else {
    lines.push('No change is over the noise. The largest changes, all in the noise:', '');
    lines.push(...changeTable(rankedChanges(profile.rows, () => true).slice(0, 5)));
  }
  if (profile.newRows.length) {
    lines.push('', `Rows with no baseline: ${profile.newRows.length} (${profile.newRows.slice(0, 5).join('; ')}${profile.newRows.length > 5 ? '; ...' : ''}).`);
  }
  if (profile.goneRows.length) {
    lines.push('', `Baseline rows not in this run: ${profile.goneRows.length} (${profile.goneRows.slice(0, 5).join('; ')}${profile.goneRows.length > 5 ? '; ...' : ''}).`);
  }
  return lines;
}

function summaryMarkdown(profile) {
  const lines = ['## API call CPU profile', '', ...header(profile), ...changesSection(profile, 30)];
  lines.push('', '### Rows', '', 'Most samples first. Per-row files are in the artifact under `rows/`.', '');
  lines.push('| Row | Samples | CPU ms for each call | Top self function | Largest change (points) | File |', '|---|---:|---:|---|---:|---|');
  for (const row of [...profile.rows].sort((a, b) => b.samples - a.samples)) {
    const self = row.top.self[0];
    const biggest = row.baseline ? [...row.baseline.grew, ...row.baseline.shrank].sort((a, b) => Math.abs(b.change) - Math.abs(a.change))[0] : null;
    const change = biggest ? `${points(biggest.change)}${biggest.overNoise ? '' : ' (noise)'}` : '';
    lines.push(`| ${row.label.replace(/\|/g, '\\|')} | ${row.samples} | ${row.cpuMsPerCall.toFixed(1)} | ${self ? `${cell(self.function, 70)} ${percent(self.share)}` : ''} | ${change} | \`${row.file}\` |`);
  }
  lines.push('', `Artifact: [${options.artifact}](${ARTIFACT_URL}). tools/profile/README.md tells how to read it.`);
  return `${lines.join('\n')}\n`;
}

function commentMarkdown(profile) {
  const lines = ['<!-- api-profile -->', '## API call CPU profile', '', ...header(profile), ...changesSection(profile, 15)];
  lines.push('', `Full data: [run](${profile.run.url}), artifact [${options.artifact}](${ARTIFACT_URL}): \`profile.json\`, and for each row \`rows/<file>.md\`, \`.folded\` and \`.svg\`. tools/profile/README.md tells how to read them.`);
  return `${lines.join('\n')}\n`;
}

async function main() {
  const { windows } = await readJson(options.windows);
  const { rows } = rowsFromWindows(windows);
  const folded = parseFolded(await readFile(options.folded, 'utf8'));
  const stats = options.stats && existsSync(options.stats) ? await readJson(options.stats) : {};
  const baseline = await loadBaseline(options.baseline);
  await mkdir(join(options.out, 'rows'), { recursive: true });
  const outRows = [];
  for (const row of rows) {
    const stacks = folded.get(row.key) ?? [];
    const counts = shares(stacks);
    const out = {
      way: row.way,
      call: row.call,
      label: row.label,
      file: row.file,
      calls: row.calls,
      wallMs: row.wallMs,
      samples: counts.total,
      cpuMsPerCall: row.calls ? (counts.total / frequency) * 1000 / row.calls : 0,
      top: { inclusive: top(counts.inclusive, counts.total, TOP), self: top(counts.self, counts.total, TOP) },
      baseline: null,
    };
    const base = baseline?.rows.get(row.label);
    if (base && base.total && counts.total) {
      const changes = compareShares(counts, base);
      out.baseline = { samples: base.total, ...largestChanges(changes, CHANGES), overNoise: changes.filter(item => item.overNoise).length };
    }
    if (stacks.length) {
      await writeFile(join(options.out, 'rows', `${row.file}.folded`), foldedText(stacks));
    }
    await writeFile(join(options.out, 'rows', `${row.file}.md`), rowMarkdown(out));
    outRows.push(out);
  }
  const labels = new Set(outRows.map(row => row.label));
  const profile = {
    commit: options.commit,
    head: options.head || options.commit,
    run: { id: options['run-id'], url: runUrl(options['run-id']) },
    frequency,
    samples: stats,
    limits: { minChange: MIN_CHANGE, minZ: MIN_Z, fewSamples: FEW_SAMPLES },
    baseline: baseline ? { commit: baseline.commit, run: baseline.run } : null,
    newRows: baseline ? outRows.filter(row => !baseline.rows.has(row.label)).map(row => row.label) : [],
    goneRows: baseline ? [...baseline.rows.keys()].filter(label => !labels.has(label)) : [],
    rows: outRows,
  };
  await writeFile(join(options.out, 'profile.json'), `${JSON.stringify(profile, null, 1)}\n`);
  await writeFile(join(options.out, 'summary.md'), summaryMarkdown(profile));
  await writeFile(join(options.out, 'comment.md'), commentMarkdown(profile));
  const graphs = outRows.filter(row => row.samples).map(row => `${row.file}\t${row.label} (${row.samples} samples)`);
  await writeFile(join(options.out, 'flamegraphs.tsv'), graphs.length ? `${graphs.join('\n')}\n` : '');
  process.stderr.write(`${outRows.length} rows, baseline ${baseline ? short(baseline.commit) : 'none'}\n`);
}

await main();
