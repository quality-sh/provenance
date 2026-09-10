import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createSession } from './session.ts';

test('refresh clears the previous view and refuses failed or superseded loads', async () => {
  const shown: string[] = [];
  let unmounted = 0;
  const status: string[] = [];
  const session = createSession<string>({
    mount(value) { shown.push(value); return () => { unmounted++; }; },
    status(message) { status.push(message); },
  });
  await session.refresh(async () => 'first');
  let finish!: (value: string) => void;
  const pending = session.refresh(() => new Promise(resolve => { finish = resolve; }));
  assert.equal(unmounted, 1);
  await session.refresh(async () => 'newer');
  finish('older');
  await pending;
  assert.deepEqual(shown, ['first', 'newer']);
  await session.refresh(async () => { throw new Error('catch_up_failed'); });
  assert.equal(unmounted, 2);
  assert.match(status.at(-1)!, /complete document.*unavailable/i);
  assert.ok(!status.join().includes('catch_up_failed'));
});

test('uses only a supplied safe explanation for a known document refusal', async () => {
  const status: string[] = [];
  const session = createSession<string>({ mount: () => () => {}, status: message => { status.push(message); },
    failure: () => 'Catch-up failed. Refresh after the saved graph is valid.',
  });
  await session.refresh(async () => { throw new Error('private credential'); });
  assert.match(status.at(-1)!, /Catch-up failed/);
  assert.ok(!status.join().includes('private credential'));
});
