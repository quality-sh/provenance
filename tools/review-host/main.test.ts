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
    ['root', 'access', 'selection', 'credential', 'requirement', 'status']
      .map(id => [id, new ElementFixture()]),
  );
  const mounted: Record<string, unknown>[] = [];
  bootstrapReviewPage({
    document: { getElementById: id => elements[id] },
    location: { origin: 'http://127.0.0.1:1234', search },
    fetch: async () => ({
      ok: true,
      async json() {
        return {
          endpoint: 'http://127.0.0.1:1234', repositoryId: 'repo', scope: 'default',
          dispositionActorIds: ['maintainer'],
        };
      },
    }),
    mount: (_root, options) => { mounted.push(options); return () => {}; },
  });
  elements.credential.value = 'secret';
  return { elements, mounted };
}

test('the linked root and focus reach the renderer after connection', async () => {
  const { elements, mounted } = fixture('?root=req_root&focus=rule_focus');
  elements.access.submit();
  await new Promise(resolve => setImmediate(resolve));

  assert.equal(elements.access.hidden, true);
  assert.equal(elements.selection.hidden, true);
  assert.deepEqual(mounted, [{
    endpoint: 'http://127.0.0.1:1234', repositoryId: 'repo', scope: 'default',
    dispositionActorIds: ['maintainer'], bearer: 'secret', rootId: 'req_root',
    focusId: 'rule_focus',
  }]);
});

test('manual Requirement selection remains available without a linked root', async () => {
  const { elements, mounted } = fixture('');
  elements.access.submit();
  await new Promise(resolve => setImmediate(resolve));

  assert.equal(elements.selection.hidden, false);
  assert.equal(elements.requirement.focused, true);
  elements.requirement.value = 'req_manual';
  elements.selection.submit();
  assert.equal(mounted.at(-1)?.rootId, 'req_manual');
});
