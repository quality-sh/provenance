// The command line options of run.mjs, and the files that a run writes.
import { parseArgs } from 'node:util';

export const WAYS = 'HTTP,MCP,CLI';

/** The options of run.mjs from `args`. */
export function readOptions(args) {
  return parseArgs({
    args,
    options: {
      bin: { type: 'string' },
      source: { type: 'string', default: '.' },
      work: { type: 'string' },
      out: { type: 'string', default: 'results.json' },
      reads: { type: 'string', default: '20' },
      writes: { type: 'string', default: '10' },
      ways: { type: 'string', default: WAYS },
      // The ways that call the catalog write operations. The profile workflow uses MCP only, to fit its time limit.
      'route-writes': { type: 'string', default: WAYS },
      filter: { type: 'string', default: '' },
      windows: { type: 'string' },
    },
  }).values;
}

/** True when the way named `way` calls the catalog write operations. */
export function routesWrites(options, way) {
  return options['route-writes'].split(',').includes(way);
}

/**
 * Adds the process and the time window of one timed call to `list`, for the
 * profile. A result with no process id or no start time adds nothing. `cold`
 * marks the first call of a row.
 */
export function keepWindow(list, way, call, result, { cold = false } = {}) {
  if (result?.pid && result.at !== undefined) {
    list.push({ way, call, pid: result.pid, start: result.at, end: result.at + result.ms, ...(cold ? { cold } : {}) });
  }
}

/** The files of a run as [path, text] pairs. Without --windows, only the results file. */
export function outputFiles(options, { meta, calls, unmeasured, windows }) {
  const files = [[options.out, `${JSON.stringify({ meta, calls, unmeasured }, null, 2)}\n`]];
  if (options.windows) {
    files.push([options.windows, `${JSON.stringify({ meta, windows })}\n`]);
  }
  return files;
}
