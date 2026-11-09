import assert from 'node:assert/strict';
import { test } from 'node:test';
import { Labeler, Shares, dispersion, splitError, splitKey, stackSymbol, compareShares, findRow, functionName, largestChanges, parseFolded, rowFile, rowsFromWindows, shares, top } from './profile.mjs';

const windows = [
  { way: 'CLI', call: 'GET /rules', pid: 10, start: 1000, end: 1010 },
  { way: 'CLI', call: 'GET /rules', pid: 11, start: 1020, end: 1030 },
  { way: 'HTTP', call: 'GET /rules', pid: 20, start: 2000, end: 2005 },
  { way: 'HTTP', call: 'POST /rules', pid: 20, start: 2010, end: 2020 },
];

test('rows come from windows in order, with one key and one file name each', () => {
  const { rows, byPid } = rowsFromWindows(windows);
  assert.deepEqual(rows.map(row => [row.label, row.calls]), [
    ['CLI GET /rules', 2],
    ['HTTP GET /rules', 1],
    ['HTTP POST /rules', 1],
  ]);
  assert.ok(rows.every(row => row.key === row.file));
  assert.equal(byPid.get(20).length, 2);
  assert.equal(rows[0].file, rowFile('CLI GET /rules'));
  assert.match(rows[0].file, /^cli-get-rules-[0-9a-f]{8}$/);
  assert.notEqual(rowFile('CLI GET /rules/{id}'), rowFile('CLI GET /rules/{id'));
});

test('a sample goes to the window of its process that holds its time', () => {
  const { byPid } = rowsFromWindows(windows);
  assert.equal(findRow(byPid, 20, 2003), `${rowFile('HTTP GET /rules')}.a`);
  assert.equal(findRow(byPid, 20, 2015), `${rowFile('HTTP POST /rules')}.a`);
  assert.equal(findRow(byPid, 11, 1025), `${rowFile('CLI GET /rules')}.b`);
  assert.deepEqual(splitKey(`${rowFile('CLI GET /rules')}.b`), { file: rowFile('CLI GET /rules'), half: 'b' });
  assert.equal(findRow(byPid, 20, 2007), null);
  assert.equal(findRow(byPid, 10, 1020), null);
  assert.equal(findRow(byPid, 99, 1005), null);
});

test('the labeler keeps samples in windows with the row file name as the process name', () => {
  const labeler = new Labeler(rowsFromWindows(windows).byPid);
  const input = [
    '      provenance    10/10    1.005000: cpu-clock:ppp: ',
    '\t    55d1 main (/bin/provenance)',
    '',
    ' tokio-runtime-w    20/21    2.012000: cpu-clock:ppp: ',
    '\t    55d2 handle (/bin/provenance)',
    '\t    55d5 <Pin<P> as Future>::poll (/bin/provenance)',
    '\t    55d6 <Pin<P> as Future>::poll (/bin/provenance)',
    '\t    55d7 run (/bin/provenance)',
    '',
    '            node    30/30    2.012000: cpu-clock:ppp: ',
    '\t    55d3 uv_run (/bin/node)',
    '',
    '      provenance    20/20    2.007000: cpu-clock:ppp: ',
    '\t    55d4 idle (/bin/provenance)',
    '',
  ];
  const out = input.map(line => labeler.line(line)).filter(line => line !== null);
  const [cli, post] = [rowFile('CLI GET /rules'), rowFile('HTTP POST /rules')];
  assert.deepEqual(out, [
    `${cli}.a 10/10 1.005000: cpu-clock:ppp: `,
    '\t    55d1 main (/bin/provenance)',
    '',
    `${post}.a 20/21 2.012000: cpu-clock:ppp: `,
    '\t    55d2 handle (/bin/provenance)',
    '\t    55d5 <Pin<P> as Future>::poll (/bin/provenance)',
    '\t    55d7 run (/bin/provenance)',
    '',
  ]);
  assert.deepEqual(labeler.stats, { samples: 4, kept: 2, outsideWindows: 1, otherProcesses: 1 });
});

test('a function has one name in each build and on each runner CPU', () => {
  assert.equal(functionName('provenance_store::open::h0123456789abcdef'), 'provenance_store::open');
  assert.equal(functionName('[unknown]'), '[unknown]');
  assert.equal(functionName('cfree@GLIBC_2.2.5'), 'cfree');
  assert.equal(functionName('__memmove_avx_unaligned_erms'), '__memmove_simd_unaligned');
  assert.equal(functionName('__memmove_avx512_unaligned_erms'), '__memmove_simd_unaligned');
  assert.equal(functionName('__memcmp_evex_movbe'), functionName('__memcmp_avx2_movbe'));
  assert.equal(functionName('memchr::arch::x86_64::avx2::memchr::One::find_raw'), 'memchr::arch::x86_64::simd::memchr::One::find_raw');
  assert.equal(functionName('provenance_store::cache::advance'), 'provenance_store::cache::advance');
});

test('inclusive shares count a function once for each sample, self shares count the top frame', () => {
  const folded = parseFolded('r0;main;run;parse 3\nr0;main;run;run 1\nr0;main;write 4\nr1;main 2\nbad line\n');
  assert.deepEqual([...folded.keys()], ['r0', 'r1']);
  const counts = shares(folded.get('r0'));
  assert.equal(counts.total, 8);
  assert.equal(counts.inclusive.get('main'), 8);
  assert.equal(counts.inclusive.get('run'), 4);
  assert.equal(counts.self.get('write'), 4);
  assert.equal(counts.self.get('run'), 1);
  assert.deepEqual(top(counts.self, counts.total, 2), [
    { function: 'write', samples: 4, share: 0.5 },
    { function: 'parse', samples: 3, share: 0.375 },
  ]);
});

test('a share change is over the noise only when it is large and many standard errors', () => {
  const base = shares([{ frames: ['main', 'a'], count: 500 }, { frames: ['main', 'b'], count: 500 }]);
  const grown = shares([{ frames: ['main', 'a'], count: 700 }, { frames: ['main', 'b'], count: 300 }, { frames: ['main', 'b', 'do_fault_[k]'], count: 1 }]);
  const changes = compareShares(grown, [base]);
  const a = changes.find(item => item.function === 'a');
  assert.ok(Math.abs(a.change - 0.2) < 1e-3);
  assert.ok(a.z > 3);
  assert.equal(a.overNoise, true);
  assert.equal(changes.find(item => item.function === 'main').overNoise, false);
  assert.equal(changes.find(item => item.function === 'do_fault_[k]'), undefined, 'kernel functions are not compared');
  const tiny = compareShares(shares([{ frames: ['a'], count: 530 }, { frames: ['b'], count: 470 }]), [shares([{ frames: ['a'], count: 500 }, { frames: ['b'], count: 500 }])]);
  assert.equal(tiny.find(item => item.function === 'a').overNoise, false, 'a 3 point change is under MIN_CHANGE');
  const small = compareShares(shares([{ frames: ['a'], count: 6 }, { frames: ['b'], count: 4 }]), [shares([{ frames: ['a'], count: 5 }, { frames: ['b'], count: 5 }])]);
  assert.equal(small.find(item => item.function === 'a').overNoise, false);
  const { grew, shrank } = largestChanges(changes, 1);
  assert.deepEqual([grew[0].function, shrank[0].function], ['a', 'b']);
});

test('the symbol of a stack line has no address and no module', () => {
  assert.equal(stackSymbol('\t    55d5 <Pin<P> as Future>::poll (/bin/provenance)'), '<Pin<P> as Future>::poll');
  assert.equal(stackSymbol('\t    ffffffff81 [unknown] ([kernel.kallsyms])'), '[unknown]');
});

test('the halves of the calls give the error when the calls differ', () => {
  const steady = new Shares();
  steady.add(['main', 'a'], 50, 'a');
  steady.add(['main', 'b'], 50, 'a');
  steady.add(['main', 'a'], 50, 'b');
  steady.add(['main', 'b'], 50, 'b');
  assert.equal(splitError(steady, 'a'), 0);
  const jumpy = new Shares();
  jumpy.add(['main', 'a'], 90, 'a');
  jumpy.add(['main', 'b'], 10, 'a');
  jumpy.add(['main', 'a'], 10, 'b');
  jumpy.add(['main', 'b'], 90, 'b');
  assert.ok(Math.abs(splitError(jumpy, 'a') - 0.4) < 1e-9);
  // The same share in both runs, but calls that differ: no change is over the noise.
  const other = new Shares();
  other.add(['main', 'a'], 200, 'a');
  other.add(['main', 'b'], 200, 'b');
  const change = compareShares(other, [steady]).find(item => item.function === 'a');
  assert.equal(change.change, 0);
  const moved = new Shares();
  moved.add(['main', 'a'], 120, 'a');
  moved.add(['main', 'b'], 80, 'a');
  moved.add(['main', 'a'], 40, 'b');
  moved.add(['main', 'b'], 160, 'b');
  const noisy = compareShares(moved, [steady]).find(item => item.function === 'a');
  assert.ok(Math.abs(noisy.change + 0.1) < 1e-9);
  assert.equal(noisy.overNoise, false, 'a 10 point change is in the noise when the halves differ by 40 points');
});

test('the baselines give the dispersion of a row, and a change within it is in the noise', () => {
  const run = share => shares([{ frames: ['main', 'a'], count: Math.round(1000 * share) }, { frames: ['main', 'b'], count: Math.round(1000 * (1 - share)) }]);
  assert.equal(dispersion([run(0.5)]), null);
  assert.equal(dispersion([run(0.5), run(0.5)]), 1);
  const spread = dispersion([run(0.4), run(0.6), run(0.5)]);
  assert.ok(spread > 10, `the dispersion is ${spread}`);
  // A 15 point change is far over a count test, but in the spread of the baselines.
  const wide = compareShares(run(0.65), [run(0.4), run(0.6), run(0.5)]).find(item => item.function === 'a');
  assert.ok(Math.abs(wide.change - 0.15) < 1e-9);
  assert.equal(wide.overNoise, false);
  // The same change over steady baselines is over the noise.
  const steady = compareShares(run(0.70), [run(0.5), run(0.5), run(0.51)]).find(item => item.function === 'a');
  assert.equal(steady.overNoise, true);
});
