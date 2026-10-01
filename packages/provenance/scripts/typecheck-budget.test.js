import test from 'node:test';
import assert from 'node:assert/strict';
import { parseDiagnostics, checkBudgets, writeDiagnostics } from './typecheck-budget.js';

test('parseDiagnostics reads TypeScript memory and instantiation totals', () => {
  assert.deepEqual(parseDiagnostics(`Types: 468282
Instantiations: 2866310
Memory used: 1892592K
Check time: 39.31s`), {
    instantiations: 2_866_310,
    memoryKiB: 1_892_592,
  });
});

test('checkBudgets rejects compiler growth above either limit', () => {
  assert.throws(
    () => checkBudgets({ memoryKiB: 750_001, instantiations: 300_001 }),
    /memory 750001 KiB exceeds 750000 KiB; instantiations 300001 exceed 300000/,
  );
});

test('checkBudgets accepts diagnostics within both limits', () => {
  assert.doesNotThrow(() => checkBudgets({ memoryKiB: 750_000, instantiations: 300_000 }));
});

test('writeDiagnostics keeps machine-readable stdout clean', () => {
  let written = '';
  writeDiagnostics('Memory used: 100K\n', { write: value => { written += value; } });
  assert.equal(written, 'Memory used: 100K\n');
});
