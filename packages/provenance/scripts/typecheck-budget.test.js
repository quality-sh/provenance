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
    () => checkBudgets({ memoryKiB: 2_200_001, instantiations: 3_300_001 }),
    /memory 2200001 KiB exceeds 2200000 KiB; instantiations 3300001 exceed 3300000/,
  );
});

test('checkBudgets accepts diagnostics within both limits', () => {
  assert.doesNotThrow(() => checkBudgets({ memoryKiB: 2_200_000, instantiations: 3_300_000 }));
});

test('writeDiagnostics keeps machine-readable stdout clean', () => {
  let written = '';
  writeDiagnostics('Memory used: 100K\n', { write: value => { written += value; } });
  assert.equal(written, 'Memory used: 100K\n');
});
