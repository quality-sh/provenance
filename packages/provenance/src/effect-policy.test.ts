import test from 'node:test';
import assert from 'node:assert/strict';
import * as Effect from 'effect/Effect';
import { EffectHttpClient, PROTOCOL_VERSION } from './effect.js';
import { verifies } from './rules.js';
const context = { repository: 'fixture', scope: 'default' };

test('document continuation retains an empty page and returns revision refusal without restart', async () => {
  verifies('rule_cursor_restarts_on_revision_change', 'examples');
  verifies('rule_review_partial_reads_remain_explicit', 'examples');
  verifies('rule_query_answer_carries_a_stamp', 'examples');
  const stamp = { instance_id: 'instance', serial: 1, digest: 'digest', derivation: 2, policy: 'catch_up', attested: [], live: [] };
  const page = { protocol_version: PROTOCOL_VERSION, operation: 'read-document', root_id: 'req_x', limit: 1, has_more: true, next_cursor: 'opaque-token', entries: [], stamp };
  const requests: unknown[] = [];
  const client = await Effect.runPromise(EffectHttpClient.connect({ baseUrl: 'http://localhost', fetch: async (_url, init) => {
    if (init?.method !== 'POST') return Response.json({ engine_version: 'test', protocol_version: PROTOCOL_VERSION });
    requests.push(JSON.parse(String(init.body)));
    return requests.length === 1 ? Response.json(page) : Response.json({ protocol_version: PROTOCOL_VERSION, operation: 'read-document', error: { kind: 'cursor_revision_changed' } }, { status: 409 });
  } }));
  const first = await Effect.runPromise(client.readDocument({ context, request: { id: 'req_x', limit: 1 } }));
  assert.deepEqual(first, page);
  const continuation = { context, request: { id: 'req_x', limit: 1, cursor: first.next_cursor } };
  const error = await Effect.runPromise(Effect.flip(client.readDocument(continuation)));
  assert.equal(error._tag, 'OperationError');
  if (error._tag === 'OperationError') assert.equal(error.failure.error.kind, 'cursor_revision_changed');
  assert.deepEqual(requests, [{ context, request: { id: 'req_x', limit: 1 } }, continuation]);
});
