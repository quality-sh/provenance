import assert from 'node:assert/strict';
import test from 'node:test';
import { bootstrapReviewPage, type BrowserElement } from './bootstrap.ts';

class ElementFixture implements BrowserElement {
  hidden = false;
  textContent = '';
  value = '';
  focused = false;
  readonly listeners = new Map<string, (event: { preventDefault(): void }) => void>();

  addEventListener(name: string, listener: (event: { preventDefault(): void }) => void) {
    this.listeners.set(name, listener);
  }

  focus() { this.focused = true; }

  submit() {
    this.listeners.get('submit')!({ preventDefault() {} });
  }
}

function fixture(search: string) {
  const elements = Object.fromEntries(
    ['root', 'selection', 'requirement', 'status']
      .map(id => [id, new ElementFixture()]),
  );
  const mounted: unknown[] = [];
  const fetched: Array<{ path: string; authorization?: string }> = [];
  const replaced: string[] = [];
  bootstrapReviewPage({
    document: { getElementById: id => elements[id] },
    location: { origin: 'http://127.0.0.1:1234', search },
    history: { replaceState: (_state, _unused, url) => { replaced.push(url); } },
    fetch: async (path, init) => {
      fetched.push({ path, authorization: init.headers?.authorization });
      return {
        ok: true,
        async json() {
          if (path === '/review-launch/exchange') return { bearer: 'secret' };
          return {
            endpoint: 'http://127.0.0.1:1234', repositoryId: 'repo', scope: 'default',
            dispositionActorIds: ['maintainer'],
          };
        },
        async text() { return ''; },
      };
    },
    mount: (_root, options) => { mounted.push(options); return () => {}; },
  });
  return { elements, fetched, mounted, replaced };
}

test('a launch code is removed before exchange and opens the linked record', async () => {
  const { fetched, mounted, replaced } = fixture('?code=launch-secret&root=req_root&focus=rule_focus');
  await new Promise(resolve => setImmediate(resolve));

  assert.deepEqual(replaced, ['/?root=req_root&focus=rule_focus']);
  assert.deepEqual(fetched, [
    { path: '/review-launch/exchange', authorization: undefined },
    { path: '/review-config', authorization: 'Bearer secret' },
  ]);
  assert.deepEqual(mounted, [{
    endpoint: 'http://127.0.0.1:1234', repositoryId: 'repo', scope: 'default',
    dispositionActorIds: ['maintainer'], bearer: 'secret', rootId: 'req_root',
    focusId: 'rule_focus',
  }]);
});

test('manual Requirement selection remains available without a linked root', async () => {
  const { elements, mounted } = fixture('?code=launch-secret');
  await new Promise(resolve => setImmediate(resolve));

  assert.equal(elements.selection.hidden, false);
  assert.equal(elements.requirement.focused, true);
  elements.requirement.value = 'req_manual';
  elements.selection.submit();
  assert.equal((mounted.at(-1) as { rootId: string }).rootId, 'req_manual');
});

test('a page without a launch code asks for a fresh link without a token input', async () => {
  const { elements, fetched } = fixture('?root=req_root');
  await new Promise(resolve => setImmediate(resolve));

  assert.deepEqual(fetched, []);
  assert.match(elements.status.textContent!, /get --review-link/);
});
