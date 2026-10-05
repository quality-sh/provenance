import { test } from 'node:test';
import assert from 'node:assert/strict';
import { verifies } from '../../packages/provenance/src/rules.ts';
import { bootstrapReviewPage, type BrowserElement } from './bootstrap.ts';
import type { ReviewMountOptions } from './session.ts';

const origin = 'http://127.0.0.1:1234';
const config = { endpoint: origin, repositoryId: 'repo', scope: 'default', dispositionActorIds: ['maintainer'] };

function fakeElement(): BrowserElement {
  return { hidden: false, textContent: null, value: '', addEventListener() {}, focus() {} };
}

function launchPage() {
  const events: string[] = [];
  const receivers: unknown[] = [];
  const mounted: ReviewMountOptions[] = [];
  const elements = new Map(['root', 'selection', 'requirement', 'status'].map(id => [id, fakeElement()]));
  let mountedPage!: () => void;
  const mounting = new Promise<void>(resolve => { mountedPage = resolve; });
  bootstrapReviewPage({
    document: { getElementById: id => elements.get(id) },
    location: { origin, pathname: '/', search: '?root=req_root&focus=rule_child', hash: '#launch=code-1' },
    history: { replaceState(_data, _unused, url) { events.push(`address ${url}`); } },
    async fetch(this: unknown, path, init) {
      receivers.push(this);
      events.push(`fetch ${path} ${init.body ?? ''}`.trim());
      const body = path === '/review-launch/session' ? { bearer: 'session-bearer' } : config;
      return { ok: true, json: async () => body };
    },
    mount(_root, options) { mounted.push(options); mountedPage(); return () => {}; },
  });
  return { events, receivers, mounted, mounting };
}

test('a launch link mounts the record with the redeemed session', async () => {
  verifies('rule_review_link_opens_signed_in', 'examples');
  const page = launchPage();
  await page.mounting;
  assert.deepEqual(page.mounted, [
    { ...config, bearer: 'session-bearer', rootId: 'req_root', focusId: 'rule_child' },
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
