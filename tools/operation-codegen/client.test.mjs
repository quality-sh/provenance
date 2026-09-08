import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { mkdtemp, writeFile, rm, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import ts from 'typescript';
import { typescriptFiles } from './typescript.mjs';

async function generatedClient(mutates = false) {
  const document = JSON.parse(await readFile(new URL('../../contracts/operations/openapi.json', import.meta.url), 'utf8'));
  for (const route of Object.values(document.paths)) if (route.post) route.post['x-operation-mutates'] = mutates;
  const root = await mkdtemp(join(tmpdir(), 'operation-client-'));
  const path = join(root, 'client.js');
  await writeFile(join(root, 'package.json'), '{"type":"module"}');
  for (const [name, content] of Object.entries(await typescriptFiles(document))) {
    if (name.endsWith('.ts')) await writeFile(join(root, name.replace(/\.ts$/, '.js')), ts.transpileModule(content, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText);
    else await writeFile(join(root, name), content);
  }
  const module = await import(path);
  await rm(root, { recursive: true, force: true });
  return module;
}

async function host(handler, action) {
  const server = createServer(handler);
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  try { await action(`http://127.0.0.1:${server.address().port}`); }
  finally { await new Promise(resolve => server.close(resolve)); }
}

test('client rejects metadata protocol mismatch before submitting an operation', async () => {
  const { HttpClient, ProtocolMismatchError } = await generatedClient();
  let posts = 0;
  await host((request, response) => {
    if (request.method === 'POST') posts++;
    response.end(JSON.stringify({ engine_version: 'test', protocol_version: 0 }));
  }, async url => { await assert.rejects(HttpClient.connect(url), ProtocolMismatchError); });
  assert.equal(posts, 0);
});

test('typed refusal is preserved and operation is sent once', async () => {
  const { HttpClient, OperationError, PROTOCOL_VERSION } = await generatedClient();
  const failure = { protocol_version: PROTOCOL_VERSION, operation: 'check-statement', error: { kind: 'invalid_input', field: 'statement', reason: 'required' } };
  let posts = 0;
  await host((request, response) => {
    if (request.method === 'GET') response.end(JSON.stringify({ engine_version: 'test', protocol_version: PROTOCOL_VERSION }));
    else { posts++; response.writeHead(400); response.end(JSON.stringify(failure)); }
  }, async url => {
    const client = await HttpClient.connect(url);
    await assert.rejects(client.checkStatement({ request: { statement: '' } }), error => {
      assert.ok(error instanceof OperationError); assert.equal(error.status, 400); assert.deepEqual(error.failure, failure); return true;
    });
  });
  assert.equal(posts, 1);
});

test('bearer connection authenticates metadata and operation requests', async () => {
  const { HttpClient, PROTOCOL_VERSION } = await generatedClient();
  const observed = [];
  const report = { standard: 'ASD-STE100', issue: 9, analyzer_version: 'test', findings: [] };
  await host((request, response) => {
    observed.push(request.headers.authorization);
    response.end(JSON.stringify(request.method === 'GET' ? { engine_version: 'test', protocol_version: PROTOCOL_VERSION } : report));
  }, async url => {
    const client = await HttpClient.connectWithBearer(url, 'fixture-secret');
    assert.deepEqual(await client.checkStatement({ request: { statement: 'Stop.' } }), report);
  });
  assert.deepEqual(observed, ['Bearer fixture-secret', 'Bearer fixture-secret']);
});

for (const status of [200, 400]) test(`read rejects valid JSON with the wrong ${status} shape`, async () => {
  const { HttpClient, PROTOCOL_VERSION } = await generatedClient();
  await host((request, response) => {
    if (request.method === 'GET') response.end(JSON.stringify({ engine_version: 'test', protocol_version: PROTOCOL_VERSION }));
    else { response.writeHead(status); response.end('{"unexpected":"value"}'); }
  }, async url => {
    const client = await HttpClient.connect(url);
    await assert.rejects(client.checkStatement({ request: { statement: 'Stop.' } }), error => error.name === 'MalformedResponseError');
  });
});

for (const status of [200, 400]) test(`malformed ${status} after a write remains uncertain`, async () => {
  const { HttpClient, PROTOCOL_VERSION, UncertainWriteError } = await generatedClient(true);
  let submissions = 0;
  await host((request, response) => {
    if (request.method === 'GET') response.end(JSON.stringify({ engine_version: 'test', protocol_version: PROTOCOL_VERSION }));
    else { submissions++; response.writeHead(status); response.end('{"unexpected":"value"}'); }
  }, async url => {
    const client = await HttpClient.connect(url);
    await assert.rejects(client.checkStatement({ request: { statement: 'Stop.' } }), UncertainWriteError);
  });
  assert.equal(submissions, 1);
});

test('lost write response is uncertain and is not retried', async () => {
  const { HttpClient, PROTOCOL_VERSION, UncertainWriteError } = await generatedClient(true);
  let submissions = 0;
  await host((request, response) => {
    if (request.method === 'GET') response.end(JSON.stringify({ engine_version: 'test', protocol_version: PROTOCOL_VERSION }));
    else { submissions++; request.socket.destroy(); }
  }, async url => {
    const client = await HttpClient.connect(url);
    await assert.rejects(client.checkStatement({ request: { statement: 'Stop.' } }), UncertainWriteError);
  });
  assert.equal(submissions, 1);
});

test('write redirects do not replay or disclose credentials', async () => {
  const { HttpClient, PROTOCOL_VERSION, UncertainWriteError } = await generatedClient(true);
  let submissions = 0;
  let forwarded = 0;
  await host((request, response) => { forwarded++; response.end('{}'); }, async destination => {
    await host((request, response) => {
      if (request.method === 'GET') response.end(JSON.stringify({ engine_version: 'test', protocol_version: PROTOCOL_VERSION }));
      else { submissions++; response.writeHead(307, { location: destination }); response.end(); }
    }, async url => {
      const client = await HttpClient.connectWithBearer(url, 'private-secret');
      await assert.rejects(client.checkStatement({ request: { statement: 'Stop.' } }), UncertainWriteError);
    });
  });
  assert.equal(submissions, 1);
  assert.equal(forwarded, 0);
});

for (const mutates of [false, true]) test(`oversized response is bounded for ${mutates ? 'writes' : 'reads'}`, async () => {
  const module = await generatedClient(mutates);
  let cancelled = false;
  let pulls = 0;
  const fetcher = async (_url, init) => {
    if (init.method !== 'POST') return Response.json({ engine_version: 'test', protocol_version: module.PROTOCOL_VERSION });
    return new Response(new ReadableStream({
      pull(controller) { pulls++; controller.enqueue(new Uint8Array(1024 * 1024)); },
      cancel() { cancelled = true; },
    }));
  };
  const client = await module.HttpClient.connect('http://fixture.test', fetcher);
  await assert.rejects(client.checkStatement({ request: { statement: 'Stop.' } }), mutates ? module.UncertainWriteError : module.MalformedResponseError);
  assert.equal(cancelled, true);
  assert.ok(pulls <= module.MAX_RESPONSE_BYTES / (1024 * 1024) + 2);
});
