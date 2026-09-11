import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import * as Effect from 'effect/Effect';
import { EffectHttpClient, MAX_RESPONSE_BYTES, PROTOCOL_VERSION, type components } from './effect.js';
import { verifies } from './rules.js';
const inventory = JSON.parse(readFileSync(new URL('../../../crates/provenance-http-client/src/client-policy-cases.json', import.meta.url), 'utf8'));
const context = { repository: 'fixture', scope: 'default' };
const write = { context, request: { run: 'run_x', status: 'passed' as const } };
const read = { context, request: { schema_version: 2 as const, spec: 'fixture', declared_by: 'spec://fixture', requirements: [] } };
const metadata = () => Response.json({ engine_version: 'test', protocol_version: PROTOCOL_VERSION });

for (const entry of inventory.refusals) test(`Effect policy inventory: ${entry.kind}, mutation ${entry.mutates}`, async () => {
  let posts = 0;
  const failure = { protocol_version: PROTOCOL_VERSION, operation: entry.mutates ? 'complete-verification' : 'plan', error: { kind: entry.kind } };
  const client = await Effect.runPromise(EffectHttpClient.connect({ baseUrl: 'https://example.test/api', bearer: 'secret', fetch: async (_url, init) => {
    assert.equal(new Headers(init?.headers).get('authorization'), 'Bearer secret');
    assert.equal(init?.redirect, 'error');
    if (init?.method !== 'POST') return metadata();
    posts++; return Response.json(failure, { status: 500 });
  } }));
  const operation: Effect.Effect<unknown, Error> = entry.mutates ? client.completeVerification(write) : client.plan(read);
  const error = await Effect.runPromise(Effect.flip(operation));
  assert.equal(error.name, entry.uncertain ? 'UncertainWriteError' : 'OperationError');
  assert.equal(posts, 1);
  assert.equal(client.unresolvedWrites().length, entry.mutates && entry.uncertain ? 1 : 0);
});

for (const mutates of [false, true]) test(`Effect bounds response streams, mutation ${mutates}`, async () => {
  assert.equal(MAX_RESPONSE_BYTES, inventory.max_response_bytes);
  let cancelled = false;
  let pulls = 0;
  const client = await Effect.runPromise(EffectHttpClient.connect({ baseUrl: 'http://localhost', fetch: async (_url, init) => {
    if (init?.method !== 'POST') return metadata();
    return new Response(new ReadableStream({ pull(controller) { pulls++; controller.enqueue(new Uint8Array(1024 * 1024)); }, cancel() { cancelled = true; } }));
  } }));
  const operation: Effect.Effect<unknown, Error> = mutates ? client.completeVerification(write) : client.plan(read);
  const error = await Effect.runPromise(Effect.flip(operation));
  assert.equal(error.name, mutates ? 'UncertainWriteError' : 'MalformedResponseError');
  assert.equal(cancelled, true);
  assert.ok(pulls <= 18);
  assert.equal(client.unresolvedWrites().length, mutates ? 1 : 0);
});

test('document continuation retains an empty page and returns revision refusal without restart', async () => {
  verifies('rule_cursor_restarts_on_revision_change', 'examples');
  verifies('rule_review_partial_reads_remain_explicit', 'examples');
  verifies('rule_query_answer_carries_a_stamp', 'examples');
  const stamp = { instance_id: 'instance', serial: 1, digest: 'digest', derivation: 2, policy: 'catch_up', attested: [], live: [] };
  const page = { protocol_version: PROTOCOL_VERSION, operation: 'read-document', root_id: 'req_x', limit: 1, has_more: true, next_cursor: 'opaque-token', entries: [], stamp };
  const requests: unknown[] = [];
  const client = await Effect.runPromise(EffectHttpClient.connect({ baseUrl: 'http://localhost', fetch: async (_url, init) => {
    if (init?.method !== 'POST') return metadata();
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
