import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';

export async function checkDiscussions({ HttpClient, OperationError }, fixture) {
  const client = await HttpClient.connectWithBearer(fixture.url, 'fixture-secret');
  const context = { repository: 'fixture', scope: 'default' };
  const request = { scope_id: 'default', parent: { node_type: 'question', node_id: 'question_ts' }, role: 'system', body: ' TypeScript text ' };
  const first = await client.postThreadMessage({ context, request });
  const second = await client.postThreadMessage({ context, request: { ...request, body: 'Second' } });
  assert.equal(first.thread.id, second.thread.id);
  assert.equal(first.message.body, ' TypeScript text ');
  assert.equal(first.message.role, 'system');
  const rows = async file => (await readFile(join(fixture.root, '.provenance/state/scopes/default/threads', file), 'utf8')).trim().split('\n').map(line => JSON.parse(line));
  assert.deepEqual(await client.listThreads({ context, request: null }), await rows('threads.jsonl'));
  assert.deepEqual(await client.listMessages({ context, request: null }), await rows('2026-07.jsonl'));
  for (const [input, kind] of [
    [{ ...request, body: ' ' }, 'empty_message_body'],
    [{ ...request, scope_id: 'other' }, 'scope_mismatch'],
    [{ ...request, parent: { ...request.parent, node_type: 'domain' } }, 'unsupported_thread_parent'],
  ]) {
    await assert.rejects(client.postThreadMessage({ context, request: input }), error => {
      assert.ok(error instanceof OperationError);
      assert.equal(error.failure.error.kind, kind);
      return true;
    });
  }
  assert.deepEqual(await client.listMessages({ context, request: null }), [first.message, second.message]);
}
