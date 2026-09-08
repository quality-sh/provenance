import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { mkdtemp, writeFile, rm, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import ts from 'typescript';
import { typescriptClient } from './templates.mjs';

async function generatedClient() {
  const document = JSON.parse(await readFile(new URL('../../contracts/operations/openapi.json', import.meta.url), 'utf8'));
  const root = await mkdtemp(join(tmpdir(), 'operation-client-'));
  const path = join(root, 'client.mjs');
  await writeFile(path, ts.transpileModule(typescriptClient(document), { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText);
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
