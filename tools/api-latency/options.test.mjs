import assert from 'node:assert/strict';
import { test } from 'node:test';
import { keepWindow, outputFiles, readOptions, routesWrites } from './options.mjs';

test('by default each way calls the catalog writes and no windows file is named', () => {
  const options = readOptions(['--bin', 'provenance', '--work', 'work']);
  assert.equal(options['route-writes'], 'HTTP,MCP,CLI');
  assert.equal(options.windows, undefined);
  for (const way of ['HTTP', 'MCP', 'CLI']) {
    assert.equal(routesWrites(options, way), true);
  }
});

test('--route-writes selects the ways that call the catalog writes', () => {
  const options = readOptions(['--route-writes', 'MCP']);
  assert.equal(routesWrites(options, 'MCP'), true);
  assert.equal(routesWrites(options, 'HTTP'), false);
  assert.equal(routesWrites(options, 'CLI'), false);
  assert.equal(routesWrites(readOptions(['--route-writes', 'HTTP,CLI']), 'MCP'), false);
});

test('a window is kept only for a result with a process id and a start time', () => {
  const list = [];
  keepWindow(list, 'HTTP', 'GET /rules', { pid: 7, at: 100, ms: 5 });
  keepWindow(list, 'HTTP', 'GET /rules', { pid: 7, at: 200, ms: 3 }, { cold: true });
  keepWindow(list, 'HTTP', 'GET /rules', { at: 300, ms: 5 });
  keepWindow(list, 'HTTP', 'GET /rules', { pid: 7, ms: 5 });
  keepWindow(list, 'HTTP', 'GET /rules', null);
  assert.deepEqual(list, [
    { way: 'HTTP', call: 'GET /rules', pid: 7, start: 100, end: 105 },
    { way: 'HTTP', call: 'GET /rules', pid: 7, start: 200, end: 203, cold: true },
  ]);
});

test('without --windows the run writes only the results file, in the same form as before', () => {
  const meta = { commit: 'abc', reads: 20, writes: 10 };
  const calls = [{ way: 'HTTP', call: 'GET /rules', kind: 'read', cold: 4, warm: [2, 3] }];
  const unmeasured = [{ way: 'CLI', call: 'POST /rules', reason: 'setup: no id' }];
  const windows = [{ way: 'HTTP', call: 'GET /rules', pid: 7, start: 1, end: 5 }];
  const files = outputFiles(readOptions([]), { meta, calls, unmeasured, windows });
  assert.deepEqual(files, [['results.json', `${JSON.stringify({ meta, calls, unmeasured }, null, 2)}\n`]]);
  const both = outputFiles(readOptions(['--out', 'r.json', '--windows', 'w.json']), { meta, calls, unmeasured, windows });
  assert.deepEqual(both.map(([path]) => path), ['r.json', 'w.json']);
  assert.equal(both[0][1], files[0][1]);
  assert.deepEqual(JSON.parse(both[1][1]), { meta, windows });
});
