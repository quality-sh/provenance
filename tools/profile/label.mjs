// Gives each CPU sample of `perf script` output to its benchmark row.
//
//   perf script ... | node tools/profile/label.mjs --windows windows.json --stats stats.json
//
// windows.json is the file that tools/api-latency/run.mjs writes with
// --windows. A sample of a process that the row timed, inside the time window
// of a call, comes out with the row file name in place of the process name. inferno-collapse-perf then puts that name at the root of each stack.
// Other samples, for example of node, git or the setup calls, are dropped.
import { readFile, writeFile } from 'node:fs/promises';
import { createInterface } from 'node:readline';
import { parseArgs } from 'node:util';
import { Labeler, rowsFromWindows } from './profile.mjs';

const { values: options } = parseArgs({ options: { windows: { type: 'string' }, stats: { type: 'string' } } });
const { windows } = JSON.parse(await readFile(options.windows, 'utf8'));
const labeler = new Labeler(rowsFromWindows(windows).byPid);

const lines = createInterface({ input: process.stdin, crlfDelay: Infinity });
let buffer = [];
for await (const text of lines) {
  const out = labeler.line(text);
  if (out !== null) {
    buffer.push(out);
    if (buffer.length >= 4096) {
      if (!process.stdout.write(`${buffer.join('\n')}\n`)) {
        await new Promise(resolve => process.stdout.once('drain', resolve));
      }
      buffer = [];
    }
  }
}
process.stdout.write(buffer.length ? `${buffer.join('\n')}\n` : '');
process.stderr.write(`samples ${JSON.stringify(labeler.stats)}\n`);
if (options.stats) {
  await writeFile(options.stats, `${JSON.stringify(labeler.stats)}\n`);
}
