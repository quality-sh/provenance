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

function metadata(compatibility) {
  return { data: { compatibility, package: { name: 'fixture', version: '0' }, repository: 'fixture', scope: 'default' }, meta: {} };
}

async function refusal(run, name, failure) {
  await assert.rejects(run(), error => {
    assert.equal(error.name, name);
    assert.equal(error._tag, name);
    assert.doesNotMatch(String(error) + JSON.stringify(error), /private-secret|private_body|secret-host-details/);
    if (failure) assert.deepEqual(error.failure, failure);
    return true;
  });
}

/** Run the same v2 connection and transport policy through each public client. */
export function clientPolicyTests(label, connect, compatibility, maxResponseBytes) {
  test(`${label}: compatibility mismatch prevents operation dispatch`, async () => {
    let calls = 0;
    await assert.rejects(connect({ baseUrl: 'http://localhost', fetch: async () => {
      calls++; return Response.json(metadata({ ...compatibility, wire: 0 }));
    } }), { name: 'ProtocolMismatchError', _tag: 'ProtocolMismatchError' });
    assert.equal(calls, 1);
  });

  test(`${label}: bound identity mismatch prevents operation dispatch`, async () => {
    let calls = 0;
    await assert.rejects(connect({
      baseUrl: 'http://localhost', repository: 'other', scope: 'default',
      fetch: async () => { calls++; return Response.json(metadata(compatibility)); },
    }), { name: 'IdentityMismatchError', _tag: 'IdentityMismatchError' });
    assert.equal(calls, 1);
  });

  for (const mutates of [false, true]) for (const status of [200, 400]) {
    test(`${label}: malformed ${status} response, mutation ${mutates}`, async () => {
      let calls = 0;
      const client = await connect({ baseUrl: 'http://localhost', fetch: async (_url, init) => {
        calls++;
        if (init?.method === undefined) return Response.json(metadata(compatibility));
        return Response.json({ private_body: 'secret-host-details' }, { status });
      } });
      await refusal(mutates ? client.write : client.read, 'MalformedResponseError');
      assert.equal(calls, 2);
    });
  }

  for (const mutates of [false, true]) test(`${label}: bounded response stream, mutation ${mutates}`, async () => {
    assert.equal(maxResponseBytes, inventory.max_response_bytes);
    let cancelled = false;
    const client = await connect({ baseUrl: 'http://localhost', fetch: async (_url, init) => {
      if (init?.method === undefined) return Response.json(metadata(compatibility));
      return new Response(new ReadableStream({
        pull(controller) { controller.enqueue(new Uint8Array(1024 * 1024)); },
        cancel() { cancelled = true; },
      }));
    } });
    await refusal(mutates ? client.write : client.read, 'MalformedResponseError');
    assert.equal(cancelled, true);
  });

  test(`${label}: redirects cannot forward credentials or replay a mutation`, async () => {
    let writes = 0;
    let forwarded = 0;
    await host((_request, response) => { forwarded++; response.end('{}'); }, async destination => {
      await host((request, response) => {
        assert.equal(request.headers.authorization, 'Bearer private-secret');
        response.setHeader('content-type', 'application/json');
        if (request.method === 'GET') response.end(JSON.stringify(metadata(compatibility)));
        else { writes++; response.writeHead(307, { location: destination }); response.end(); }
      }, async baseUrl => {
        const client = await connect({ baseUrl, bearer: 'private-secret' });
        await refusal(client.write, 'ConnectionError');
      });
    });
    assert.equal(writes, 1);
    assert.equal(forwarded, 0);
  });

  test(`${label}: a lost mutation response is a connection error and is not replayed`, async () => {
    let writes = 0;
    await host((request, response) => {
      if (request.method === 'GET') {
        response.setHeader('content-type', 'application/json');
        response.end(JSON.stringify(metadata(compatibility)));
      } else { writes++; request.socket.destroy(); }
    }, async baseUrl => {
      const client = await connect({ baseUrl });
      await refusal(client.write, 'ConnectionError');
    });
    assert.equal(writes, 1);
  });
}
