import { test } from 'node:test';
import assert from 'node:assert/strict';
import { connectEvents, parseFrame, type EventSocket } from './events.ts';
import { BROWSER_RESET_REASONS } from './session.ts';
import type { ReviewChange } from './session.ts';

class FakeSocket implements EventSocket {
  onmessage: ((event: { data: unknown }) => void) | null = null;
  onclose: (() => void) | null = null;
  sent: string[] = [];
  closed = false;
  send(text: string) { this.sent.push(text); }
  close() { this.closed = true; }
  frame(value: unknown) { this.onmessage?.({ data: JSON.stringify(value) }); }
  drop() { this.onclose?.(); }
}

function client(show: (rootId: string, focusId?: string) => Promise<void> = async () => {}) {
  const sockets: FakeSocket[] = [];
  const changes: ReviewChange[] = [];
  const notices: string[] = [];
  const events = connectEvents({
    open: () => {
      const socket = new FakeSocket();
      sockets.push(socket);
      return socket;
    },
    refresh: change => { changes.push(change); },
    showRecord: show,
    live: () => { notices.push('live'); },
    paused: () => { notices.push('paused'); },
  });
  events.start();
  return { events, sockets, changes, notices };
}

const changed = (sequence: number, epoch = 'run-1') => ({
  type: 'changed', epoch, sequence,
  scopes: [{ scope: 'default', records: [{ kind: 'requirement', id: 'req_a' }], families: ['requirement'] }],
});

// Implementation aid: the page reads named in the frame; the page Rule evidence is in provenance-web.
test('ready reads everything and the next changed frame reads its records', () => {
  const { sockets, changes } = client();
  sockets[0]!.frame({ type: 'ready', epoch: 'run-1', sequence: 4 });
  sockets[0]!.frame(changed(5));
  assert.deepEqual(changes, [
    { _tag: 'Everything', reason: 'ready' },
    { _tag: 'Records', records: [{ kind: 'requirement', id: 'req_a' }] },
  ]);
});

// Implementation aid: the page reads named in the frame; the page Rule evidence is in provenance-web.
test('a reset, a sequence gap and a new epoch read everything', () => {
  const { sockets, changes } = client();
  const socket = sockets[0]!;
  socket.frame({ type: 'ready', epoch: 'run-1', sequence: 0 });
  socket.frame({ type: 'reset', epoch: 'run-1', sequence: 1, reason: 'file-events-lost' });
  socket.frame(changed(3));
  socket.frame(changed(4, 'run-2'));
  assert.deepEqual(changes.slice(1), [
    { _tag: 'Everything', reason: 'file-events-lost' },
    { _tag: 'Everything', reason: 'sequence-gap' },
    { _tag: 'Everything', reason: 'sequence-gap' },
  ]);
});

// Implementation aid: the client has no timer, so it reconnects once and then waits for the page.
test('a dropped feed reconnects once at once, then waits for the page to wake', () => {
  const { events, sockets, changes, notices } = client();
  sockets[0]!.frame({ type: 'ready', epoch: 'run-1', sequence: 0 });
  sockets[0]!.drop();
  assert.equal(sockets.length, 2);
  sockets[1]!.drop();
  assert.equal(sockets.length, 2);
  assert.deepEqual(notices, ['live', 'paused']);

  events.wake();
  assert.equal(sockets.length, 3);
  sockets[2]!.frame(changed(9));
  assert.deepEqual(changes.at(-1), { _tag: 'Everything', reason: 'sequence-gap' });
});

// Implementation aid: a stopped client must not reconnect after its own close.
test('a stopped client closes its socket and does not reconnect', () => {
  const { events, sockets } = client();
  events.stop();
  assert.equal(sockets[0]!.closed, true);
  sockets[0]!.drop();
  events.wake();
  assert.equal(sockets.length, 1);
});

// Implementation aid: the Rule evidence is the real socket flow in cli_review_reuse.rs.
test('a review link is acknowledged after the page shows the record', async () => {
  let finish!: () => void;
  const shown: unknown[] = [];
  const { sockets } = client((rootId, focusId) => {
    shown.push([rootId, focusId]);
    return new Promise(resolve => { finish = resolve; });
  });
  sockets[0]!.frame({ type: 'show-record', requestId: 'request-1', rootId: 'req_root', focusId: 'rule_a' });
  assert.deepEqual(shown, [['req_root', 'rule_a']]);
  await Promise.resolve();
  assert.deepEqual(sockets[0]!.sent, []);
  finish();
  await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(sockets[0]!.sent.map(text => JSON.parse(text)), [{ type: 'ack', requestId: 'request-1' }]);
});

// Implementation aid: an acknowledgement belongs to the connection that asked for it.
test('a review link that the page shows after a disconnect is not acknowledged', async () => {
  let finish!: () => void;
  const { sockets } = client(() => new Promise(resolve => { finish = resolve; }));
  sockets[0]!.frame({ type: 'show-record', requestId: 'request-1', rootId: 'req_root' });
  sockets[0]!.drop();
  finish();
  await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(sockets.flatMap(socket => socket.sent), []);
});

// Implementation aid: an unknown frame closes the feed instead of guessing.
test('a frame of another shape closes the socket', () => {
  const { sockets } = client();
  sockets[0]!.onmessage?.({ data: '{"type":"changed","epoch":"run-1"}' });
  assert.equal(sockets[0]!.closed, true);
  assert.equal(parseFrame('not json'), undefined);
  assert.equal(parseFrame({ type: 'ready' }), undefined);
});

// Implementation aid: reset reasons must fit the renderer contract before a refresh.
test('an unknown reset reason closes the feed without refreshing the page', () => {
  const { sockets, changes } = client();
  sockets[0]!.frame({ type: 'reset', epoch: 'run-1', sequence: 1, reason: 'unknown-reason' });
  assert.equal(sockets[0]!.closed, true);
  assert.deepEqual(changes, []);
});

// Implementation aid: pin the browser reset contract and check each reason through the client.
test('the fixed browser reset reasons reread the page', async t => {
  const reasons = [
    'ready', 'sequence-gap', 'file-events-lost', 'configuration-changed',
    'observation-failed', 'observation-recovered', 'subscriber-loss',
  ];
  assert.deepEqual(BROWSER_RESET_REASONS, reasons);
  for (const reason of reasons) {
    await t.test(reason, () => {
      const { sockets, changes } = client();
      sockets[0]!.frame({ type: 'reset', epoch: 'run-1', sequence: 1, reason });
      assert.equal(sockets[0]!.closed, false);
      assert.deepEqual(changes, [{ _tag: 'Everything', reason }]);
    });
  }
});

// Implementation aid: the native capture failure must not enter the browser feed.
test('a native-only reset reason closes the feed without refreshing the page', () => {
  const { sockets, changes } = client();
  sockets[0]!.frame({ type: 'reset', epoch: 'run-1', sequence: 1, reason: 'page-capture-failed' });
  assert.equal(sockets[0]!.closed, true);
  assert.deepEqual(changes, []);
});

// Implementation aid: the host closes an unavailable feed after its observation-failed reset.
test('an observation failure rereads the page and pauses after reconnect fails', () => {
  const { sockets, changes, notices } = client();
  sockets[0]!.frame({ type: 'ready', epoch: 'run-1', sequence: 0 });
  sockets[0]!.frame({ type: 'reset', epoch: 'run-1', sequence: 1, reason: 'observation-failed' });
  assert.equal(sockets[0]!.closed, false);
  assert.deepEqual(changes.at(-1), { _tag: 'Everything', reason: 'observation-failed' });
  sockets[0]!.drop();
  sockets[1]!.drop();
  assert.deepEqual(notices, ['live', 'paused']);
});
