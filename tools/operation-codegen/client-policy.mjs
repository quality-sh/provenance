import test from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { readFileSync } from 'node:fs';

const inventory = JSON.parse(readFileSync(new URL('../../crates/provenance-http-client/src/client-policy-cases.json', import.meta.url), 'utf8'));

async function host(handler, action) {
  const server = createServer(handler);
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  try { await action(`http://127.0.0.1:${server.address().port}`); }
  finally { server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); }
}

async function refusal(client, mutates, name, failure) {
  await assert.rejects(mutates ? client.write() : client.read(), error => {
    assert.equal(error.name, name);
    assert.equal(error._tag, name);
    assert.doesNotMatch(String(error) + JSON.stringify(error), /private-secret|private_body|secret-host-details/);
    if (failure) assert.deepEqual(error.failure, failure);
    return true;
  });
  if (client.unresolvedWrites) {
    assert.equal(client.unresolvedWrites().length, mutates && name === 'UncertainWriteError' ? 1 : 0);
  }
}

/** Run the same wire-policy cases through each public client. */
export function clientPolicyTests(label, connect, protocolVersion, maxResponseBytes) {
  const metadata = () => Response.json({ engine_version: 'test', protocol_version: protocolVersion });

  test(`${label}: protocol mismatch prevents operation dispatch`, async () => {
    let posts = 0;
    await assert.rejects(connect({ baseUrl: 'http://localhost', fetch: async (_url, init) => {
      if (init?.method === 'POST') posts++;
      return Response.json({ engine_version: 'test', protocol_version: 0 });
    } }), { name: 'ProtocolMismatchError', _tag: 'ProtocolMismatchError' });
    assert.equal(posts, 0);
  });

  test(`${label}: refusal classification matches the shared inventory`, async () => {
    for (const entry of inventory.refusals) {
      let posts = 0;
      const failure = { protocol_version: protocolVersion, operation: entry.mutates ? 'complete-verification' : 'plan', error: { kind: entry.kind } };
      const client = await connect({ baseUrl: 'https://example.test/api', bearer: 'private-secret', fetch: async (_url, init) => {
        assert.equal(new Headers(init?.headers).get('authorization'), 'Bearer private-secret');
        assert.equal(init?.redirect, 'error');
        if (init?.method !== 'POST') return metadata();
        posts++; return Response.json(failure, { status: 500 });
      } });
      await refusal(client, entry.mutates, entry.uncertain ? 'UncertainWriteError' : 'OperationError', failure);
      assert.equal(posts, 1);
    }
  });

  for (const mutates of [false, true]) for (const status of [200, 400]) {
    test(`${label}: malformed ${status} response, mutation ${mutates}`, async () => {
      let posts = 0;
      const client = await connect({ baseUrl: 'http://localhost', fetch: async (_url, init) => {
        if (init?.method !== 'POST') return metadata();
        posts++; return Response.json({ private_body: 'secret-host-details' }, { status });
      } });
      await refusal(client, mutates, mutates ? 'UncertainWriteError' : 'MalformedResponseError');
      assert.equal(posts, 1);
    });
  }

  for (const mutates of [false, true]) test(`${label}: bounded response stream, mutation ${mutates}`, async () => {
    assert.equal(maxResponseBytes, inventory.max_response_bytes);
    let cancelled = false;
    let pulls = 0;
    const client = await connect({ baseUrl: 'http://localhost', fetch: async (_url, init) => {
      if (init?.method !== 'POST') return metadata();
      return new Response(new ReadableStream({
        pull(controller) { pulls++; controller.enqueue(new Uint8Array(1024 * 1024)); },
        cancel() { cancelled = true; },
      }));
    } });
    await refusal(client, mutates, mutates ? 'UncertainWriteError' : 'MalformedResponseError');
    assert.equal(cancelled, true);
    assert.ok(pulls <= maxResponseBytes / (1024 * 1024) + 2);
  });

  test(`${label}: redirects cannot forward credentials or replay a mutation`, async () => {
    let posts = 0;
    let forwarded = 0;
    await host((_request, response) => { forwarded++; response.end('{}'); }, async destination => {
      await host((request, response) => {
        assert.equal(request.headers.authorization, 'Bearer private-secret');
        if (request.method !== 'POST') response.end(JSON.stringify({ engine_version: 'test', protocol_version: protocolVersion }));
        else { posts++; response.writeHead(307, { location: destination }); response.end(); }
      }, async baseUrl => {
        const client = await connect({ baseUrl, bearer: 'private-secret' });
        await refusal(client, true, 'UncertainWriteError');
      });
    });
    assert.equal(posts, 1);
    assert.equal(forwarded, 0);
  });

  test(`${label}: a lost mutation response stays uncertain without replay`, async () => {
    let posts = 0;
    await host((request, response) => {
      if (request.method !== 'POST') response.end(JSON.stringify({ engine_version: 'test', protocol_version: protocolVersion }));
      else { posts++; request.socket.destroy(); }
    }, async baseUrl => {
      await refusal(await connect({ baseUrl }), true, 'UncertainWriteError');
    });
    assert.equal(posts, 1);
  });

}
