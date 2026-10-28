import { after, test } from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import ts from 'typescript';
import { typescriptFiles } from './typescript.mjs';

const reference = name => ({ $ref: `#/components/schemas/${name}` });
const responses = (success, failure) => ({
  200: { content: { 'application/json': { schema: reference(success) } } },
  400: { content: { 'application/json': { schema: reference(failure) } } },
});
const compatibility = { wire: 9, state: 2, review_journal: 1, read_derivation: 1 };

// An update with an optional If-Match header, the shape of the Rule, Source,
// Resolution, Domain and Boundary updates.
const document = {
  openapi: '3.1.0', info: { title: 'fixture', version: '0' },
  paths: {
    '/metadata': { get: {
      operationId: 'metadata', 'x-operation-mutates': false,
      responses: responses('MetadataSuccess', 'MetadataFailure'),
    } },
    '/fixtures/{id}': { patch: {
      operationId: 'updateFixture', 'x-operation-mutates': true,
      parameters: [
        { name: 'id', in: 'path', required: true, schema: { type: 'string' } },
        { name: 'If-Match', in: 'header', required: false, schema: { type: 'string' } },
      ],
      requestBody: { content: { 'application/json': { schema: reference('FixtureBody') } } },
      responses: responses('FixtureSuccess', 'FixtureFailure'),
    } },
  },
  components: { schemas: Object.fromEntries(
    ['MetadataSuccess', 'MetadataFailure', 'FixtureSuccess', 'FixtureFailure', 'FixtureBody']
      .map(name => [name, { type: 'object' }]),
  ) },
};

async function generatedClient() {
  const root = await mkdtemp(join(tmpdir(), 'operation-optional-header-'));
  await writeFile(join(root, 'package.json'), '{"type":"module"}');
  for (const [name, content] of Object.entries(await typescriptFiles(document, compatibility))) {
    await mkdir(dirname(join(root, name)), { recursive: true });
    if (name.endsWith('.ts')) await writeFile(join(root, name.replace(/\.ts$/, '.js')), ts.transpileModule(content, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText);
    else await writeFile(join(root, name), content);
  }
  return { module: await import(join(root, 'client.js')), root };
}

const generated = await generatedClient();
after(() => rm(generated.root, { recursive: true, force: true }));

async function patchHeaders(call) {
  const patches = [];
  const server = createServer((request, response) => {
    if (request.method === 'PATCH') patches.push(request.headers);
    response.end(JSON.stringify(request.method === 'GET'
      ? { data: { compatibility, package: { name: 'fixture', version: '0' }, repository: 'fixture', scope: 'default' }, meta: {} }
      : { data: {}, meta: {} }));
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  try {
    const client = await generated.module.HttpClient.connect(`http://127.0.0.1:${server.address().port}`);
    await client.updateFixture(call);
  } finally { await new Promise(resolve => server.close(resolve)); }
  assert.equal(patches.length, 1);
  return patches[0];
}

// Implementation aid: an omitted optional header is not sent as the text "undefined".
test('an update without if_match sends no If-Match header', async () => {
  const headers = await patchHeaders({ id: 'fixture_a', data: {} });
  assert.equal(headers['if-match'], undefined);
});

// Implementation aid: a supplied optional header is sent with its value.
test('an update with if_match sends the If-Match header', async () => {
  const headers = await patchHeaders({ id: 'fixture_a', if_match: '"etag-1"', data: {} });
  assert.equal(headers['if-match'], '"etag-1"');
});
