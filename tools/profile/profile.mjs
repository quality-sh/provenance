// The parts of the API profile that do not read or write files: give each CPU
// sample to its benchmark row, count function shares per row, compare a row
// with the baseline, and write the Markdown. label.mjs and analyze.mjs read
// and write the files.
import { createHash } from 'node:crypto';

/** The smallest change of an inclusive share, in share points, that counts as over the noise. */
export const MIN_CHANGE = 0.02;
/** The smallest change, in standard errors, that counts as over the noise. */
export const MIN_Z = 3;
/** A row with fewer samples than this has a noisy profile. */
export const FEW_SAMPLES = 200;

/** A file name for a row that is the same in each run. */
export function rowFile(label) {
  const slug = label.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 80).replace(/-+$/, '');
  const hash = createHash('sha256').update(label).digest('hex').slice(0, 8);
  return `${slug}-${hash}`;
}

/**
 * The rows of a windows file and, for each process, its time windows sorted
 * by start. A row is a way in and a call. Its file name is a word with no
 * space or semicolon, the same in each run and each job.
 *
 * The calls of a row go to two halves in turn: the first, third, ... call to
 * half `a`, the second, fourth, ... to half `b`. The difference between the
 * halves measures how much the shares change from call to call. The key of a
 * window, `<file>.a` or `<file>.b`, replaces the process name in the perf
 * script output.
 */
export function rowsFromWindows(windows) {
  const rows = [];
  const byLabel = new Map();
  const byPid = new Map();
  for (const window of windows) {
    const label = `${window.way} ${window.call}`;
    let row = byLabel.get(label);
    if (!row) {
      row = { key: rowFile(label), way: window.way, call: window.call, label, calls: 0, wallMs: 0 };
      row.file = row.key;
      rows.push(row);
      byLabel.set(label, row);
    }
    const half = row.calls % 2 === 0 ? 'a' : 'b';
    row.calls += 1;
    row.wallMs += window.end - window.start;
    if (!byPid.has(window.pid)) {
      byPid.set(window.pid, []);
    }
    byPid.get(window.pid).push({ start: window.start, end: window.end, key: `${row.key}.${half}` });
  }
  for (const list of byPid.values()) {
    list.sort((a, b) => a.start - b.start);
  }
  return { rows, byPid };
}

/** The file name and half of a window key. */
export function splitKey(key) {
  const dot = key.lastIndexOf('.');
  return dot < 0 ? { file: key, half: null } : { file: key.slice(0, dot), half: key.slice(dot + 1) };
}

/** The key of the window of process `pid` that holds the time `ms`, or null. */
export function findRow(byPid, pid, ms) {
  const list = byPid.get(pid);
  if (!list) {
    return null;
  }
  let low = 0;
  let high = list.length - 1;
  while (low <= high) {
    const middle = (low + high) >> 1;
    const window = list[middle];
    if (ms < window.start) {
      high = middle - 1;
    } else if (ms > window.end) {
      low = middle + 1;
    } else {
      return window.key;
    }
  }
  return null;
}

// The first line of a sample in `perf script` output: the process name
// (right-aligned, and it can hold spaces), pid/tid, time in seconds, then the
// event. The stack lines that follow start with a tab.
const HEADER = /^\s*(\S.*?)\s+(\d+)\/(\d+)\s+(\d+\.\d+):(.*)$/;

/** The function name of a `perf script` stack line (`<address> <symbol> (<dso>)`). */
export function stackSymbol(text) {
  const body = text.trim();
  const space = body.indexOf(' ');
  const rest = space < 0 ? '' : body.slice(space + 1);
  const dso = rest.lastIndexOf(' (');
  return dso < 0 ? rest : rest.slice(0, dso);
}

/**
 * Reads `perf script` output line by line. A sample in a row window comes out
 * with the row file name in place of the process name; other samples are dropped.
 * A stack line with the same function as the line before it is dropped too:
 * deep async stacks repeat one poll function many times, and the repeats
 * change no inclusive or self share. `stats` counts the samples.
 */
export class Labeler {
  constructor(byPid) {
    this.byPid = byPid;
    this.keep = false;
    this.stats = { samples: 0, kept: 0, outsideWindows: 0, otherProcesses: 0 };
  }

  /** Gives the output line for one input line, or null. */
  line(text) {
    if (text === '') {
      const out = this.keep ? '' : null;
      this.keep = false;
      return out;
    }
    if (text.startsWith('\t')) {
      if (!this.keep) {
        return null;
      }
      const symbol = stackSymbol(text);
      if (symbol === this.last) {
        return null;
      }
      this.last = symbol;
      return text;
    }
    const match = HEADER.exec(text);
    if (!match) {
      return this.keep ? text : null;
    }
    const [, , pid, tid, seconds, rest] = match;
    this.stats.samples += 1;
    const key = findRow(this.byPid, Number(pid), Number(seconds) * 1000);
    this.keep = key !== null;
    this.last = null;
    if (key) {
      this.stats.kept += 1;
      return `${key} ${pid}/${tid} ${seconds}:${rest}`;
    }
    if (this.byPid.has(Number(pid))) {
      this.stats.outsideWindows += 1;
    } else {
      this.stats.otherProcesses += 1;
    }
    return null;
  }
}

// The C library and some Rust crates have one version of a function for each
// set of CPU features, and the runner CPU chooses the version. These words
// name the feature set, between `_` or `::` separators.
const CPU_FEATURE = /(?<=[_:])(avx512[a-z]*|avx2|avx|evex(?:256|512)?|sse2|sse4_1|sse4_2|ssse3)(?=[_:]|$)/g;

/**
 * The name of a function for the counts: with no Rust hash suffix and no C
 * library symbol version, and with one name for all CPU feature versions
 * (`__memmove_avx_unaligned_erms` and `__memmove_avx512_unaligned_erms` are
 * both `__memmove_simd_unaligned`). So one function has one name in each
 * build and on each runner.
 */
export function functionName(frame) {
  let name = frame.replace(/::h[0-9a-f]{16}$/, '').replace(/@+GLIBC_[\d.]+$/, '').replace(CPU_FEATURE, 'simd');
  if (name.startsWith('__')) {
    name = name.replace(/(_erms)?(_rtm)?$/, '');
  }
  return name;
}

/** Reads folded stacks (`a;b;c count`). Gives a map of root frame to its stacks, root removed. */
export function parseFolded(text, { byRoot = true } = {}) {
  const out = new Map();
  for (const line of text.split('\n')) {
    const stack = foldedLine(line);
    if (!stack) {
      continue;
    }
    const root = byRoot ? stack.frames.shift() : '';
    if (!out.has(root)) {
      out.set(root, []);
    }
    out.get(root).push(stack);
  }
  return out;
}

/**
 * Sample counts per function for one row, one stack at a time. Inclusive
 * counts each sample once for each function on its stack; self counts the
 * sample for its top frame.
 */
export class Shares {
  total = 0;
  inclusive = new Map();
  self = new Map();
  /** The sample count of each half, and the inclusive counts of each function in each half. */
  halves = { totals: [0, 0], inclusive: new Map() };

  /** Adds one stack. `half` is 'a', 'b' or null. */
  add(frames, count, half = null) {
    this.total += count;
    const names = frames.map(functionName);
    const index = half === 'a' ? 0 : half === 'b' ? 1 : -1;
    if (index >= 0) {
      this.halves.totals[index] += count;
    }
    for (const name of new Set(names)) {
      this.inclusive.set(name, (this.inclusive.get(name) ?? 0) + count);
      if (index >= 0) {
        if (!this.halves.inclusive.has(name)) {
          this.halves.inclusive.set(name, [0, 0]);
        }
        this.halves.inclusive.get(name)[index] += count;
      }
    }
    const leaf = names.at(-1);
    if (leaf !== undefined) {
      this.self.set(leaf, (this.self.get(leaf) ?? 0) + count);
    }
  }
}

/**
 * The standard error of one function's inclusive share in one run, from the
 * difference between the two halves of its calls. The mean of two halves has
 * a standard error of half their difference. Gives 0 when a half has no samples.
 */
export function splitError(counts, name) {
  const [totalA, totalB] = counts.halves.totals;
  if (!totalA || !totalB) {
    return 0;
  }
  const [a, b] = counts.halves.inclusive.get(name) ?? [0, 0];
  return Math.abs(a / totalA - b / totalB) / 2;
}

/** The shares of a list of stacks. */
export function shares(stacks) {
  const counts = new Shares();
  for (const { frames, count } of stacks) {
    counts.add(frames, count);
  }
  return counts;
}

/** Splits one folded line into its frames and count, or gives null. */
export function foldedLine(line) {
  const space = line.lastIndexOf(' ');
  if (space <= 0) {
    return null;
  }
  const count = Number(line.slice(space + 1));
  if (!Number.isFinite(count) || count <= 0) {
    return null;
  }
  return { frames: line.slice(0, space).split(';'), count };
}

/** The `limit` functions with the largest counts, as shares of the row. */
export function top(counts, total, limit) {
  return [...counts.entries()]
    .sort((a, b) => b[1] - a[1] || (a[0] < b[0] ? -1 : 1))
    .slice(0, limit)
    .map(([name, samples]) => ({ function: name, samples, share: total ? samples / total : 0 }));
}

/**
 * The change of each function's inclusive share from the baseline to now.
 * `z` is the change over its standard error: the larger of the error of a
 * two-proportion test on the sample counts and the error from the halves of
 * both runs (see splitError). The samples of one call are not independent, so
 * the halves give the larger error when the calls differ. A change is over the
 * noise when it is at least MIN_CHANGE and at least MIN_Z standard errors.
 */
export function compareShares(now, base) {
  const names = new Set([...now.inclusive.keys(), ...base.inclusive.keys()]);
  const changes = [];
  for (const name of names) {
    const a = base.inclusive.get(name) ?? 0;
    const b = now.inclusive.get(name) ?? 0;
    const shareBase = base.total ? a / base.total : 0;
    const shareNow = now.total ? b / now.total : 0;
    const change = shareNow - shareBase;
    const pooled = base.total + now.total ? (a + b) / (base.total + now.total) : 0;
    const counted = base.total && now.total ? Math.sqrt(pooled * (1 - pooled) * (1 / base.total + 1 / now.total)) : 0;
    const error = Math.max(counted, Math.hypot(splitError(now, name), splitError(base, name)));
    const z = error ? change / error : 0;
    changes.push({ function: name, base: shareBase, now: shareNow, change, z, overNoise: Math.abs(change) >= MIN_CHANGE && Math.abs(z) >= MIN_Z });
  }
  return changes;
}

/** The `limit` largest increases and decreases. */
export function largestChanges(changes, limit) {
  const grew = changes.filter(item => item.change > 0).sort((a, b) => b.change - a.change).slice(0, limit);
  const shrank = changes.filter(item => item.change < 0).sort((a, b) => a.change - b.change).slice(0, limit);
  return { grew, shrank };
}

export const percent = share => `${(share * 100).toFixed(1)}%`;
export const points = change => `${change >= 0 ? '+' : ''}${(change * 100).toFixed(1)}`;

/** A function name as Markdown code in a table cell. */
export function cell(name, max = 110) {
  const short = name.length > max ? `${name.slice(0, max - 1)}…` : name;
  return `\`${short.replace(/`/g, "'").replace(/\|/g, '\\|')}\``;
}

export const short = sha => (sha ? sha.slice(0, 10) : 'unknown');
