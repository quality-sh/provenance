import assert from 'node:assert/strict';
import test from 'node:test';
import { configure, defineSpec } from './index.js';
import { UncertainWriteError } from './client.js';
import { recordingHost } from './http-recorder.test-helper.js';

for (const callbackFails of [false, true]) {
  test(`failed completion ${callbackFails ? 'preserves the callback error' : 'remains visible after a passing callback'}`, async t => {
    const recorder = await recordingHost({ 'complete-verification': {} });
    t.after(() => recorder.close());
    configure({ ...recorder.settings, localRoot: process.cwd() });
    const rule = defineSpec('completion').rule('ready').statement('The system is ready.');
    const original = new Error('callback failed');
    const pending = rule.verify('ready', () => { if (callbackFails) throw original; }, { file: 'check.ts' });
    await assert.rejects(pending, error => callbackFails ? error === original : error instanceof UncertainWriteError);
    assert.deepEqual(recorder.requests().map(request => request.command), ['begin-verification', 'complete-verification']);
  });
}

test('verification completion stays on the connection and context that began the run', async t => {
  const first = await recordingHost();
  const second = await recordingHost();
  t.after(() => first.close());
  t.after(() => second.close());
  configure({ ...first.settings, localRoot: process.cwd(), repositoryId: 'first', scope: 'original' });
  const rule = defineSpec('connection').rule('ready').statement('The system is ready.');
  await rule.verify('ready', () => {
    configure({ ...second.settings, localRoot: process.cwd(), repositoryId: 'second', scope: 'changed' });
  }, { file: 'check.ts' });
  assert.deepEqual(first.requests().map(request => request.command), ['begin-verification', 'complete-verification']);
  assert.deepEqual(first.requests().map(request => request.context), [
    { repository: 'first', scope: 'original' }, { repository: 'first', scope: 'original' },
  ]);
  assert.deepEqual(second.requests(), []);
});
