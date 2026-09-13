import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { mkdtemp, writeFile, rm, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import ts from 'typescript';
import { typescriptFiles } from './typescript.mjs';
import { clientPolicyTests } from './client-policy.mjs';

async function generatedClient() {
  const document = JSON.parse(await readFile(new URL('../../contracts/operations/openapi.json', import.meta.url), 'utf8'));
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

const clientModule = await generatedClient();

async function host(handler, action) {
  const server = createServer(handler);
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  try { await action(`http://127.0.0.1:${server.address().port}`); }
  finally { await new Promise(resolve => server.close(resolve)); }
}

test('typed refusal is preserved and operation is sent once', async () => {
  const { HttpClient, OperationError, PROTOCOL_VERSION } = clientModule;
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
  const { HttpClient, PROTOCOL_VERSION } = clientModule;
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

test('an aborted read cancels a response stream and releases its lock', async () => {
  const { HttpClient, PROTOCOL_VERSION } = clientModule;
  let cancelled = false;
  let body;
  let started;
  const reading = new Promise(resolve => { started = resolve; });
  const fetcher = async (_url, init) => {
    if (init.method !== 'POST') return Response.json({ engine_version: 'test', protocol_version: PROTOCOL_VERSION });
    body = new ReadableStream({ pull() { started(); }, cancel() { cancelled = true; } });
    return new Response(body);
  };
  const client = await HttpClient.connect('http://localhost', fetcher);
  const controller = new AbortController();
  const result = client.checkStatement({ request: { statement: 'Stop.' } }, { signal: controller.signal });
  const rejected = assert.rejects(result, { name: 'ConnectionError' });
  await reading;
  controller.abort();
  await new Promise(resolve => setTimeout(resolve, 10));
  assert.equal(cancelled, true);
  await rejected;
  assert.equal(body.locked, false);
});

clientPolicyTests('Promise', async ({ baseUrl, bearer, fetch }) => {
  const client = bearer === undefined
    ? await clientModule.HttpClient.connect(baseUrl, fetch)
    : await clientModule.HttpClient.connectWithBearer(baseUrl, bearer, fetch);
  const context = { repository: 'fixture', scope: 'default' };
  return {
    read: () => client.plan({ context, request: { schema_version: 2, spec: 'fixture', declared_by: 'spec://fixture', requirements: [] } }),
    write: () => client.completeVerification({ context, request: { run: 'run_x', status: 'passed' } }),
  };
}, clientModule.PROTOCOL_VERSION, clientModule.MAX_RESPONSE_BYTES);
