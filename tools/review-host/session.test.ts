import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createSession } from './session.ts';

function document(name: string) {
  return { name, disposed: 0, dispose() { this.disposed++; } };
}

test('refresh clears the previous view and refuses failed or superseded loads', async () => {
  const shown: string[] = [];
  let unmounted = 0;
  const status: string[] = [];
  const session = createSession<ReturnType<typeof document>>({
    mount(value) { shown.push(value.name); return () => { unmounted++; value.dispose(); }; },
    status(message) { status.push(message); },
  });
  const first = document('first');
  const older = document('older');
  const newer = document('newer');
  await session.refresh(async () => first);
  let finish!: (value: typeof older) => void;
  const pending = session.refresh(() => new Promise(resolve => { finish = resolve; }));
  assert.equal(unmounted, 1);
  await session.refresh(async () => newer);
  finish(older);
  await pending;
  assert.deepEqual(shown, ['first', 'newer']);
  assert.equal(older.disposed, 1);
  assert.equal(first.disposed, 1);
  assert.equal(newer.disposed, 0);
  await session.refresh(async () => { throw new Error('catch_up_failed'); });
  assert.equal(unmounted, 2);
  assert.equal(newer.disposed, 1);
  assert.match(status.at(-1)!, /complete document.*unavailable/i);
  assert.ok(!status.join().includes('catch_up_failed'));
});

test('uses only a supplied safe explanation for a known document refusal', async () => {
  const status: string[] = [];
  const session = createSession<ReturnType<typeof document>>({ mount: () => () => {}, status: message => { status.push(message); },
    failure: () => 'Catch-up failed. Refresh after the saved graph is valid.',
  });
  await session.refresh(async () => { throw new Error('private credential'); });
  assert.match(status.at(-1)!, /Catch-up failed/);
  assert.ok(!status.join().includes('private credential'));
});

test('mounting a store leaves document completeness and failure to the renderer', async () => {
  const status: string[] = [];
  const session = createSession<ReturnType<typeof document>>({
    mount: value => () => value.dispose(),
    status: message => { status.push(message); },
  });
  for (const state of ['partial', 'error', 'stale', 'complete']) {
    await session.refresh(async () => document(state));
    assert.match(status.at(-1)!, /document.*status.*below/i);
    assert.doesNotMatch(status.at(-1)!, /current|complete|saved working-copy/i);
  }
});

test('a superseded failure does not replace the latest status', async () => {
  const status: string[] = [];
  const session = createSession<ReturnType<typeof document>>({
    mount: value => () => value.dispose(),
    status: message => { status.push(message); },
  });
  let reject!: (error: Error) => void;
  const pending = session.refresh(() => new Promise((_, fail) => { reject = fail; }));
  await session.refresh(async () => document('newer'));
  const latest = [...status];
  reject(new Error('private credential'));
  await pending;
  assert.deepEqual(status, latest);
});
