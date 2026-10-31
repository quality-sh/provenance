#!/usr/bin/env node
// Runs two timing test binaries in turn, `--pairs` times, and saves each
// output with a runs.json index for timing-report.mjs. Odd pairs run the
// base first and even pairs run the head first, so a slow drift of the host
// does not favour one side.
import { spawnSync } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { parseArgs } from 'node:util';

export const TIMING_TEST = 'operations::queries::tests::comparison::timing_comparison_rows';

function run(binary, cwd, filter) {
  const started = performance.now();
  const result = spawnSync(binary, ['--ignored', '--exact', filter, '--nocapture'], {
    cwd,
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
  });
  const seconds = (performance.now() - started) / 1000;
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(`${binary} exited with status ${result.status}:\n${result.stdout}\n${result.stderr}`);
  }
  return { output: result.stdout, seconds };
}

// Cargo runs a test binary from its package directory, so `baseCwd` and
// `headCwd` name those directories; the default is the binary's directory.
export async function pairedTiming({ base, head, pairs, out, baseCwd, headCwd, filter = TIMING_TEST }) {
  await mkdir(out, { recursive: true });
  const sides = { base: resolve(base), head: resolve(head) };
  const cwds = { base: resolve(baseCwd ?? dirname(sides.base)), head: resolve(headCwd ?? dirname(sides.head)) };
  const record = { filter, base: sides.base, head: sides.head, pairs: [] };
  for (let index = 1; index <= pairs; index += 1) {
    const order = index % 2 === 1 ? ['base', 'head'] : ['head', 'base'];
    const pair = { order };
    for (const side of order) {
      const name = `${side}-${String(index).padStart(3, '0')}.txt`;
      const { output, seconds } = run(sides[side], cwds[side], filter);
      await writeFile(join(out, name), output);
      pair[side] = name;
      pair[`${side}_seconds`] = Number(seconds.toFixed(3));
      console.error(`pair ${index}/${pairs} ${side}: ${seconds.toFixed(1)} s`);
    }
    record.pairs.push(pair);
  }
  await writeFile(join(out, 'runs.json'), `${JSON.stringify(record, null, 2)}\n`);
  return record;
}

async function main() {
  const { values } = parseArgs({
    options: {
      base: { type: 'string' },
      head: { type: 'string' },
      pairs: { type: 'string', default: '15' },
      out: { type: 'string' },
      'base-cwd': { type: 'string' },
      'head-cwd': { type: 'string' },
      filter: { type: 'string', default: TIMING_TEST },
    },
  });
  for (const name of ['base', 'head', 'out']) if (!values[name]) throw new Error(`--${name} is necessary`);
  const pairs = Number(values.pairs);
  if (!Number.isInteger(pairs) || pairs < 1) throw new Error(`--pairs must be a positive integer, not ${values.pairs}`);
  await pairedTiming({ ...values, baseCwd: values['base-cwd'], headCwd: values['head-cwd'], pairs });
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch(error => {
    console.error(error.message);
    process.exit(2);
  });
}
