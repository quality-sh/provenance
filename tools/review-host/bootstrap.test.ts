import assert from 'node:assert/strict';
import { setImmediate } from 'node:timers/promises';
import test from 'node:test';
import { bootstrapReviewPage, type BrowserElement } from './bootstrap.ts';
import type { ReviewMountOptions } from './session.ts';

function page() {
  const listeners = new Map<string, (event: { preventDefault(): void }) => void>();
  const elements = new Map<string, BrowserElement>();
  for (const id of ['root', 'access', 'selection', 'credential', 'requirement', 'status']) {
    elements.set(id, {
      hidden: false, textContent: '', value: '',
      addEventListener(name, listener) { listeners.set(`${id}:${name}`, listener); },
      focus() {},
    });
  }
  const config = {
    endpoint: 'http://127.0.0.1:1234', repositoryId: 'repo', scope: 'default',
    dispositionActorIds: ['maintainer'],
  };
  const requests: unknown[] = [];
  const mounted: ReviewMountOptions[] = [];
  bootstrapReviewPage({
    document: { getElementById: id => elements.get(id) },
    location: { origin: config.endpoint, search: '?root=req_root' },
    async fetch(this: unknown, path, init) {
      if (this !== undefined && this !== globalThis) throw new TypeError('Illegal invocation');
      requests.push({ path, init });
      return { ok: true, json: async () => config };
    },
    mount(_root, options) { mounted.push(options); return () => {}; },
  });
  return {
    config, requests, mounted, elements,
    async connect(token: string) {
      elements.get('credential')!.value = token;
      listeners.get('access:submit')!({ preventDefault() {} });
      await setImmediate();
    },
  };
}

// Regression aid: no Rule covers fetch receivers. The fake rejects the receiver
// that caused valid credentials to fail when the page loaded its configuration.
test('the page loads its configuration with a browser-compatible fetch receiver', async () => {
  const fixture = page();
  await fixture.connect('secret');
  assert.equal(fixture.elements.get('status')!.textContent, 'Connected · repo / default');
  assert.deepEqual(fixture.requests, [{
    path: '/review-config',
    init: { headers: { authorization: 'Bearer secret' }, redirect: 'error', cache: 'no-store' },
  }]);
  assert.deepEqual(fixture.mounted, [{ ...fixture.config, bearer: 'secret', rootId: 'req_root' }]);
});
