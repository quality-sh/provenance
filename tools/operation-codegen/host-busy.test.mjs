// Implementation aid: the coverage scan has no Rule for busy-host failures.
import test from 'node:test';
import assert from 'node:assert/strict';
import { FAILURE_STATUS } from './grammar-lint.mjs';

test('host_busy is a declared 503 failure kind', () => {
  assert.equal(FAILURE_STATUS.get('host_busy'), 503);
});
