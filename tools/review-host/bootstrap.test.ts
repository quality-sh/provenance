import { test } from 'node:test';
import assert from 'node:assert/strict';
import { verifies } from '../../packages/provenance/src/rules.ts';
import { bootstrapReviewPage, SESSION_KEY, type BrowserElement } from './bootstrap.ts';
import { CONNECTION_REFUSED, type ReviewMountOptions } from './session.ts';

const origin = 'http://127.0.0.1:1234';
const config = { endpoint: origin, repositoryId: 'repo', scope: 'default', dispositionActorIds: ['maintainer'] };

function fakeElement(): BrowserElement {
  return { hidden: false, textContent: null, value: '', addEventListener() {}, focus() {} };
}

/** A fake of the page's own-origin local storage. */
function fakeStorage(initial: Record<string, string> = {}) {
  const items = new Map(Object.entries(initial));
  return {
    items,
    getItem: (key: string) => items.get(key) ?? null,
    setItem: (key: string, value: string) => { items.set(key, value); },
    removeItem: (key: string) => { items.delete(key); },
  };
}

const settle = () => new Promise(resolve => setImmediate(resolve));

function launchPage(hash = '#launch=code-1', storage = fakeStorage(), accepted = 'page-session') {
  const events: string[] = [];
  const receivers: unknown[] = [];
  const mounted: ReviewMountOptions[] = [];
  const sockets: string[] = [];
  const elements = new Map(['root', 'selection', 'requirement', 'status'].map(id => [id, fakeElement()]));
  let mountedPage!: () => void;
  const mounting = new Promise<void>(resolve => { mountedPage = resolve; });
  bootstrapReviewPage({
    document: { getElementById: id => elements.get(id) },
    location: { origin, pathname: '/', search: '?root=req_root&focus=rule_child', hash },
    history: { replaceState(_data, _unused, url) { events.push(`address ${url}`); } },
    async fetch(this: unknown, path, init) {
      receivers.push(this);
      const authorization = init.headers.authorization;
      events.push(`fetch ${path} ${init.body ?? authorization ?? ''}`.trim());
      if (path === '/review-launch/session') return { ok: true, json: async () => ({ session: 'page-session' }) };
      return { ok: authorization === `Bearer ${accepted}`, json: async () => config };
    },
    storage,
    mount(_root, options) {
      mounted.push(options);
      mountedPage();
      return { dispose() {}, refresh() {}, showRecord: async () => {} };
    },
    openEvents: session => {
      sockets.push(session);
      return { onmessage: null, onclose: null, send() {}, close() {} };
    },
    onWake() {},
  });
  return { events, receivers, mounted, mounting, sockets, storage, status: elements.get('status')! };
}

// Implementation aid: fake page boundaries check that bootstrap passes the session to the renderer.
test('a launch link mounts the record with the redeemed session', async () => {
  const page = launchPage();
  await page.mounting;
  assert.deepEqual(page.mounted, [
    { ...config, bearer: 'page-session', rootId: 'req_root', focusId: 'rule_child' },
  ]);
});

test('a launch link stores the session on its origin and opens events with it', async function storesTheLaunchSession() {
  verifies('rule_review_page_session_stays_on_its_origin', 'examples');
  const page = launchPage();
  await page.mounting;
  assert.deepEqual([...page.storage.items], [[SESSION_KEY, 'page-session']]);
  assert.deepEqual(page.sockets, ['page-session']);
  assert.deepEqual(page.events.slice(1), [
    'fetch /review-launch/session {"code":"code-1"}',
    'fetch /review-config Bearer page-session',
  ]);
});

// Implementation aid: security hardening keeps the code out of history and the Referer value.
test('the code leaves the address bar before the exchange', async () => {
  const page = launchPage();
  await page.mounting;
  assert.deepEqual(page.events.slice(0, 2), [
    'address /?root=req_root&focus=rule_child',
    'fetch /review-launch/session {"code":"code-1"}',
  ]);
});

// Regression aid: no Rule covers fetch receivers. A browser throws "Illegal invocation"
// when the page calls fetch with another receiver, and the page then fails to connect.
test('the page loads its configuration with a browser-compatible fetch receiver', async () => {
  const page = launchPage();
  await page.mounting;
  assert.deepEqual(page.receivers, [undefined, undefined]);
});

test('a page without a launch code connects with the stored session', async function reusesTheStoredSession() {
  verifies('rule_review_page_session_stays_on_its_origin', 'examples');
  const page = launchPage('', fakeStorage({ [SESSION_KEY]: 'page-session' }));
  await page.mounting;
  assert.deepEqual(page.events, ['fetch /review-config Bearer page-session']);
  assert.deepEqual(page.mounted, [
    { ...config, bearer: 'page-session', rootId: 'req_root', focusId: 'rule_child' },
  ]);
  assert.deepEqual(page.sockets, ['page-session']);
});

test('a refused stored session is removed and the page shows the refusal', async function forgetsARefusedSession() {
  verifies('rule_review_page_session_stays_on_its_origin', 'examples');
  const page = launchPage('', fakeStorage({ [SESSION_KEY]: 'old-session' }));
  await settle();
  await settle();
  assert.deepEqual(page.mounted, []);
  assert.deepEqual(page.sockets, []);
  assert.deepEqual([...page.storage.items], []);
  assert.equal(page.status.textContent, CONNECTION_REFUSED);
});

// Implementation aid: a page with neither a code nor a stored session asks for a new link.
test('a page without a launch code or a stored session shows the refusal', async () => {
  const page = launchPage('');
  await settle();
  assert.deepEqual(page.events, []);
  assert.equal(page.status.textContent, CONNECTION_REFUSED);
});

// Implementation aid: the Rule evidence is the real socket flow in cli_review_reuse.rs.
test('a review link shows its record in the mounted page and acknowledges it', async () => {
  const shown: unknown[] = [];
  const addresses: string[] = [];
  let mounts = 0;
  let socket!: { onmessage: ((event: { data: unknown }) => void) | null; sent: string[] };
  let mountedPage!: () => void;
  const mounting = new Promise<void>(resolve => { mountedPage = resolve; });
  const elements = new Map(['root', 'selection', 'requirement', 'status'].map(id => [id, fakeElement()]));
  bootstrapReviewPage({
    document: { getElementById: id => elements.get(id) },
    location: { origin, pathname: '/', search: '?root=req_root', hash: '' },
    history: { replaceState(_data, _unused, url) { addresses.push(url); } },
    fetch: async () => ({ ok: true, json: async () => config }),
    storage: fakeStorage({ [SESSION_KEY]: 'page-session' }),
    mount() {
      mounts++;
      mountedPage();
      return {
        dispose() {},
        refresh() {},
        showRecord: async (rootId, focusId) => { shown.push([rootId, focusId]); },
      };
    },
    openEvents: () => {
      socket = { onmessage: null, sent: [] };
      return { ...{ onclose: null, close() {} }, get onmessage() { return socket.onmessage; },
        set onmessage(value) { socket.onmessage = value; }, send(text: string) { socket.sent.push(text); } };
    },
    onWake() {},
  });
  await mounting;
  socket.onmessage?.({ data: JSON.stringify({ type: 'show-record', requestId: 'r1', rootId: 'req_other', focusId: 'rule_b' }) });
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(mounts, 1);
  assert.deepEqual(shown, [['req_other', 'rule_b']]);
  assert.deepEqual(addresses, ['/?root=req_other&focus=rule_b']);
  assert.deepEqual(socket.sent.map(text => JSON.parse(text)), [{ type: 'ack', requestId: 'r1' }]);
});

// Regression aid: exercise recovery through the host page without browser UI tests.
function livePage() {
  const elements = new Map(['root', 'selection', 'requirement', 'status'].map(id => [id, fakeElement()]));
  const sockets: import('./events.ts').EventSocket[] = [];
  let wake!: () => void;
  let actors = ['maintainer'];
  let reads = 0;
  let mounts = 0;
  let disposals = 0;
  bootstrapReviewPage({
    document: { getElementById: id => elements.get(id) },
    location: { origin, pathname: '/', search: '?root=req_root', hash: '' },
    history: { replaceState() {} },
    fetch: async () => {
      reads++;
      return { ok: true, json: async () => ({ ...config, dispositionActorIds: actors }) };
    },
    storage: fakeStorage({ [SESSION_KEY]: 'page-session' }),
    mount() {
      mounts++;
      return { dispose() { disposals++; }, refresh() {}, showRecord: async () => {} };
    },
    openEvents() {
      const socket = { onmessage: null, onclose: null, send() {}, close() {} };
      sockets.push(socket);
      return socket;
    },
    onWake(listener) { wake = listener; },
  });
  return {
    elements, sockets, wake: () => wake(), settings: () => { actors = ['other']; },
    state: () => ({ reads, mounts, disposals }),
    frame: (type: string) => sockets.at(-1)?.onmessage?.({ data: JSON.stringify({ type, epoch: 'host', sequence: 0, reason: 'observation-recovered' }) }),
  };
}
test('the first ready reads the page without reading its configuration twice', async () => {
  const page = livePage();
  await settle();
  page.frame('ready');
  await settle();
  assert.equal(page.state().reads, 1);
  page.frame('reset');
  await settle();
  assert.equal(page.state().reads, 2);
});

test('a paused linked page tells the person to focus it and reconnects without disposal', async () => {
  const page = livePage();
  await settle();
  page.sockets[0].onclose?.();
  page.sockets[1].onclose?.();
  assert.equal(page.elements.get('selection')?.hidden, true);
  assert.equal(page.elements.get('status')?.textContent,
    'Live updates stopped. Focus this page to reconnect. Unsaved drafts stay open.');
  page.wake();
  page.frame('ready');
  await settle();
  assert.equal(page.sockets.length, 3);
  assert.equal(page.state().mounts, 1);
  assert.equal(page.state().disposals, 0);
});

test('a settings notice gives a visible path after the person saves drafts', async () => {
  const page = livePage();
  await settle();
  page.settings();
  page.frame('reset');
  await settle();
  assert.equal(page.elements.get('status')?.textContent,
    'Reviewer settings changed. Save or discard your drafts, then reload this page to apply the settings.');
  assert.equal(page.state().disposals, 0);
});
