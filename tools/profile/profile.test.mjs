import assert from 'node:assert/strict';
import { test } from 'node:test';
import { Labeler, compareShares, findRow, functionName, largestChanges, parseFolded, rowFile, rowsFromWindows, shares, top } from './profile.mjs';

const windows = [
  { way: 'CLI', call: 'GET /rules', pid: 10, start: 1000, end: 1010 },
  { way: 'CLI', call: 'GET /rules', pid: 11, start: 1020, end: 1030 },
  { way: 'HTTP', call: 'GET /rules', pid: 20, start: 2000, end: 2005 },
  { way: 'HTTP', call: 'POST /rules', pid: 20, start: 2010, end: 2020 },
];

test('rows come from windows in order, with one key and one file name each', () => {
  const { rows, byPid } = rowsFromWindows(windows);
  assert.deepEqual(rows.map(row => [row.key, row.label, row.calls]), [
    ['r0', 'CLI GET /rules', 2],
    ['r1', 'HTTP GET /rules', 1],
    ['r2', 'HTTP POST /rules', 1],
  ]);
  assert.equal(byPid.get(20).length, 2);
  assert.equal(rows[0].file, rowFile('CLI GET /rules'));
  assert.match(rows[0].file, /^cli-get-rules-[0-9a-f]{8}$/);
  assert.notEqual(rowFile('CLI GET /rules/{id}'), rowFile('CLI GET /rules/{id'));
});

test('a sample goes to the window of its process that holds its time', () => {
  const { byPid } = rowsFromWindows(windows);
  assert.equal(findRow(byPid, 20, 2003), 'r1');
  assert.equal(findRow(byPid, 20, 2015), 'r2');
  assert.equal(findRow(byPid, 20, 2007), null);
  assert.equal(findRow(byPid, 10, 1020), null);
  assert.equal(findRow(byPid, 99, 1005), null);
});

test('the labeler keeps samples in windows with the row key as the process name', () => {
  const labeler = new Labeler(rowsFromWindows(windows).byPid);
  const input = [
    '      provenance    10/10    1.005000: cpu-clock:ppp: ',
    '\t    55d1 main (/bin/provenance)',
    '',
    ' tokio-runtime-w    20/21    2.012000: cpu-clock:ppp: ',
    '\t    55d2 handle (/bin/provenance)',
    '',
    '            node    30/30    2.012000: cpu-clock:ppp: ',
    '\t    55d3 uv_run (/bin/node)',
    '',
    '      provenance    20/20    2.007000: cpu-clock:ppp: ',
    '\t    55d4 idle (/bin/provenance)',
    '',
  ];
  const out = input.map(line => labeler.line(line)).filter(line => line !== null);
  assert.deepEqual(out, [
    'r0 10/10 1.005000: cpu-clock:ppp: ',
    '\t    55d1 main (/bin/provenance)',
    '',
    'r2 20/21 2.012000: cpu-clock:ppp: ',
    '\t    55d2 handle (/bin/provenance)',
    '',
  ]);
  assert.deepEqual(labeler.stats, { samples: 4, kept: 2, outsideWindows: 1, otherProcesses: 1 });
});

test('Rust hash suffixes are removed from function names', () => {
  assert.equal(functionName('provenance_store::open::h0123456789abcdef'), 'provenance_store::open');
  assert.equal(functionName('[unknown]'), '[unknown]');
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
  const grown = shares([{ frames: ['main', 'a'], count: 700 }, { frames: ['main', 'b'], count: 300 }]);
  const changes = compareShares(grown, base);
  const a = changes.find(item => item.function === 'a');
  assert.ok(Math.abs(a.change - 0.2) < 1e-9);
  assert.ok(a.z > 3);
  assert.equal(a.overNoise, true);
  assert.equal(changes.find(item => item.function === 'main').overNoise, false);
  const small = compareShares(shares([{ frames: ['a'], count: 6 }, { frames: ['b'], count: 4 }]), shares([{ frames: ['a'], count: 5 }, { frames: ['b'], count: 5 }]));
  assert.equal(small.find(item => item.function === 'a').overNoise, false);
  const { grew, shrank } = largestChanges(changes, 1);
  assert.deepEqual([grew[0].function, shrank[0].function], ['a', 'b']);
});
