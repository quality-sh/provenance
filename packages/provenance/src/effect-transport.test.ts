import test from 'node:test';
import assert from 'node:assert/strict';
import { createServer, type RequestListener } from 'node:http';
import * as Effect from 'effect/Effect';
import { EffectHttpClient, PROTOCOL_VERSION, type ClientFailure } from './effect.js';

async function host(handler: RequestListener, action: (url: string) => Promise<void>) {
  const server = createServer(handler);
  await new Promise<void>(resolve => server.listen(0, '127.0.0.1', resolve));
  const address = server.address();
  assert.ok(address && typeof address === 'object');
  try { await action(`http://127.0.0.1:${address.port}`); }
  finally { server.closeAllConnections(); await new Promise<void>(resolve => server.close(() => resolve())); }
}
const context = { repository: 'fixture', scope: 'default' };
const write = { context, request: { id: 'req_x', scope_id: 'default', description: null, clear_fields: ['description' as const] } };

test('Effect refuses redirects without forwarding a bearer or replaying a mutation', async () => {
  let forwarded = 0;
  let posts = 0;
  await host((_request, response) => { forwarded++; response.end('{}'); }, async destination => {
    await host((request, response) => {
      assert.equal(request.headers.authorization, 'Bearer private-token');
      if (request.method !== 'POST') response.end(JSON.stringify({ engine_version: 'test', protocol_version: PROTOCOL_VERSION }));
      else { posts++; response.writeHead(307, { location: destination }); response.end(); }
    }, async baseUrl => {
      const client = await Effect.runPromise(EffectHttpClient.connect({ baseUrl, bearer: 'private-token' }));
      const error = await Effect.runPromise(Effect.flip(client.updateRequirement(write)));
      assert.equal(error._tag, 'UncertainWriteError');
      assert.equal(client.unresolvedWrites().length, 1);
      assert.doesNotMatch(String(error) + JSON.stringify(error), /private-token/);
    });
  });
  assert.equal(posts, 1);
  assert.equal(forwarded, 0);
});

for (const mutates of [false, true]) for (const status of [200, 400]) {
  test(`Effect rejects malformed ${status} bodies privately, mutation ${mutates}`, async () => {
    let posts = 0;
    const client = await Effect.runPromise(EffectHttpClient.connect({ baseUrl: 'http://localhost', fetch: async (_url, init) => {
      if (init?.method !== 'POST') return Response.json({ engine_version: 'test', protocol_version: PROTOCOL_VERSION });
      posts++; return Response.json({ private_body: 'secret-host-details' }, { status });
    } }));
    const operation: Effect.Effect<unknown, ClientFailure> = mutates ? client.updateRequirement(write) : client.checkStatement({ request: { statement: 'Stop.' } });
    const error = await Effect.runPromise(Effect.flip(operation));
    assert.equal(error._tag, mutates ? 'UncertainWriteError' : 'MalformedResponseError');
    assert.equal(posts, 1);
    assert.equal(client.unresolvedWrites().length, mutates ? 1 : 0);
    assert.doesNotMatch(String(error) + JSON.stringify(error), /secret-host-details|private_body/);
  });
}

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
