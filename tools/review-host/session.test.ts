import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createSession, type ReviewConfig } from './session.ts';

const config: ReviewConfig = {
  endpoint: 'http://127.0.0.1:1234',
  repositoryId: 'repo',
  scope: 'default',
  dispositionActorIds: ['maintainer'],
};

test('open passes all named connection fields and the selected Requirement', async () => {
  const mounted: unknown[] = [];
  const session = createSession({
    mount(options) { mounted.push(options); return () => {}; },
    connected() {},
    status() {},
  });
  await session.connect('secret', async () => config);
  session.open('req_root');
  assert.deepEqual(mounted, [{ ...config, bearer: 'secret', rootId: 'req_root' }]);
});

test('open passes an optional focused record to the renderer', async () => {
  const mounted: unknown[] = [];
  const session = createSession({
    mount(options) { mounted.push(options); return () => {}; },
    connected() {},
    status() {},
  });
  await session.connect('secret', async () => config);
  session.open('req_root', 'rule_focus');
  assert.deepEqual(mounted, [{
    ...config, bearer: 'secret', rootId: 'req_root', focusId: 'rule_focus',
  }]);
});

test('a new connection unmounts the page and a failed connection clears access', async () => {
  let unmounted = 0;
  const statuses: string[] = [];
  const session = createSession({
    mount: () => () => { unmounted++; },
    connected() {},
    status(message) { statuses.push(message); },
  });
  await session.connect('first', async () => config);
  session.open('req_root');
  await session.connect('second', async () => { throw new Error('private credential'); });
  assert.equal(unmounted, 1);
  assert.match(statuses.at(-1)!, /connection refused/i);
  assert.equal(session.open('req_other'), false);
  assert.ok(!statuses.join().includes('private credential'));
});

test('a superseded connection result cannot replace a later one', async () => {
  const connected: string[] = [];
  const session = createSession({
    mount: () => () => {},
    connected(value) { connected.push(value.repositoryId); },
    status() {},
  });
  let finish!: (value: ReviewConfig) => void;
  const pending = session.connect('older', () => new Promise(resolve => { finish = resolve; }));
  await session.connect('newer', async () => ({ ...config, repositoryId: 'newer' }));
  finish({ ...config, repositoryId: 'older' });
  await pending;
  assert.deepEqual(connected, ['newer']);
});

test('a superseded failure cannot replace the latest status', async () => {
  const statuses: string[] = [];
  const session = createSession({ mount: () => () => {}, connected() {}, status(message) { statuses.push(message); } });
  let reject!: (error: Error) => void;
  const pending = session.connect('older', () => new Promise((_, fail) => { reject = fail; }));
  await session.connect('newer', async () => config);
  const latest = [...statuses];
  reject(new Error('private credential'));
  await pending;
  assert.deepEqual(statuses, latest);
});
