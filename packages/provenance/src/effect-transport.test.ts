import test from 'node:test';
import assert from 'node:assert/strict';
import * as Effect from 'effect/Effect';
import { EffectHttpClient, PROTOCOL_VERSION, MAX_RESPONSE_BYTES } from './effect.js';
import { clientPolicyTests } from '../../../tools/operation-codegen/client-policy.mjs';

async function execute<A, E>(effect: Effect.Effect<A, E>): Promise<A> {
  const result = await Effect.runPromise(Effect.match(effect, {
    onSuccess: value => ({ value }), onFailure: error => ({ error }),
  }));
  if ('error' in result) throw result.error;
  return result.value;
}

const context = { repository: 'fixture', scope: 'default' };
const write = { context, request: { id: 'req_x', scope_id: 'default', description: null, clear_fields: ['description' as const] } };

clientPolicyTests('Effect', async options => {
  const client = await execute(EffectHttpClient.connect(options));
  return {
    read: () => execute(client.plan({ context, request: { schema_version: 2, spec: 'fixture', declared_by: 'spec://fixture', requirements: [] } })),
    write: () => execute(client.completeVerification({ context, request: { run: 'run_x', status: 'passed' } })),
    unresolvedWrites: () => client.unresolvedWrites(),
  };
}, PROTOCOL_VERSION, MAX_RESPONSE_BYTES);

for (const baseUrl of ['file:///tmp/host', 'https://user:secret@example.test', 'https://example.test/?secret=x', 'https://example.test/#secret']) {
  test(`invalid destination is refused before transport: ${baseUrl.split(':')[0]}`, async () => {
    let requests = 0;
    const error = await Effect.runPromise(Effect.flip(EffectHttpClient.connect({ baseUrl, bearer: 'private-token', fetch: async () => { requests++; return Response.json({}); } })));
    assert.equal(error._tag, 'ConnectionError');
    assert.equal(requests, 0);
    assert.doesNotMatch(String(error) + JSON.stringify(error), /secret|private-token/);
  });
}

test('non-serializable mutation input fails before dispatch', async () => {
  let posts = 0;
  const client = await Effect.runPromise(EffectHttpClient.connect({ baseUrl: 'http://localhost', fetch: async (_url, init) => {
    if (init?.method === 'POST') posts++;
    return Response.json({ engine_version: 'test', protocol_version: PROTOCOL_VERSION });
  } }));
  const cyclic = { ...write, cycle: {} };
  cyclic.cycle = cyclic;
  const error = await Effect.runPromise(Effect.flip(client.updateRequirement(cyclic)));
  assert.equal(error._tag, 'InvalidRequestError');
  assert.equal(posts, 0);
  assert.deepEqual(client.unresolvedWrites(), []);
});
