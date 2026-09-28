import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { build } from 'esbuild';
import { effectFiles } from './effect.mjs';
import { operationValidators } from './validators.mjs';

const reference = name => ({ $ref: `#/components/schemas/${name}` });
const response = name => ({
  description: name,
  content: { 'application/json': { schema: reference(name) } },
});

function serverDocument() {
  const failure = {
    type: 'object',
    required: ['error', 'meta'],
    properties: {
      error: {
        oneOf: [
          { type: 'object', required: ['kind'], properties: { kind: { const: 'invalid_input' } } },
          { type: 'object', required: ['kind'], properties: { kind: { const: 'stale' } } },
        ],
      },
      meta: { type: 'object' },
    },
  };
  return {
    openapi: '3.1.0',
    info: { title: 'Server fixture', version: '1' },
    paths: {
      '/rules': {
        get: {
          operationId: 'listRules',
          parameters: [
            { name: 'query', in: 'query', required: true, schema: { type: 'string' } },
            { name: 'base', in: 'query', required: true, schema: { type: 'string' } },
            { name: 'file', in: 'query', required: true, schema: { type: 'string' } },
          ],
          'x-provenance-query-variants': [
            { selector: 'search', parameters: [{ name: 'query', in: 'query', required: true }], success: reference('ListRulesSuccess'), failure: reference('ListRulesFailure') },
            { selector: 'stale', parameters: [{ name: 'query', in: 'query', required: true }, { name: 'base', in: 'query', required: true }], success: reference('ListRulesSuccess'), failure: reference('ListRulesFailure') },
            { selector: 'resolve-symbol', parameters: [{ name: 'query', in: 'query', required: true }, { name: 'file', in: 'query', required: true }], success: reference('ListRulesSuccess'), failure: reference('ListRulesFailure') },
          ],
          responses: { 200: response('ListRulesSuccess'), 400: response('ListRulesFailure') },
        },
      },
      '/rules/{id}': {
        patch: {
          operationId: 'updateRule',
          parameters: [
            { name: 'id', in: 'path', required: true, schema: { type: 'string' } },
            { name: 'If-Match', in: 'header', required: true, schema: { type: 'string' } },
          ],
          requestBody: {
            required: true,
            content: { 'application/json': { schema: reference('UpdateRuleRequestInput') } },
          },
          responses: {
            200: response('UpdateRuleSuccess'),
            400: response('UpdateRuleFailure'),
            409: response('UpdateRuleFailure'),
          },
        },
      },
      '/sources': {
        get: {
          operationId: 'listSources',
          responses: { 200: response('ListSourcesSuccess'), 400: response('ListSourcesFailure') },
        },
      },
    },
    components: {
      schemas: {
        ListRulesSuccess: { type: 'object', required: ['data'], properties: { data: { type: 'array', items: { type: 'string' } } } },
        ListRulesFailure: failure,
        UpdateRuleRequestInput: { type: 'object', required: ['data'], properties: { data: { type: 'string' } } },
        UpdateRuleSuccess: { type: 'object', required: ['data'], properties: { data: { type: 'string' } } },
        UpdateRuleFailure: failure,
        ListSourcesSuccess: { type: 'object', required: ['data'], properties: { data: { type: 'array', items: { type: 'string' } } } },
        ListSourcesFailure: failure,
      },
    },
  };
}

test('Effect HttpApi output is shaped for independently served resource groups', async () => {
  const source = (await effectFiles(serverDocument()))['effect-contract.ts'];
  assert.match(source, /HttpApiGroup\.make\("rules"\)/);
  assert.match(source, /HttpApiGroup\.make\("sources"\)/);
  assert.doesNotMatch(source, /HttpApiGroup\.make\("default"\)/);
});

test('Effect HttpApi input schemas use server header casing and optional query keys', async () => {
  const source = (await effectFiles(serverDocument()))['effect-contract.ts'];
  assert.match(source, /UpdateRuleHeaders = Schema\.Struct\(\{ "if-match": Schema\.String \}\)/);
  assert.match(source, /ListRulesQuery = Schema\.Struct\(\{ "query": Schema\.String, "base": Schema\.optionalKey\(Schema\.String\), "file": Schema\.optionalKey\(Schema\.String\) \}\)/);
});

test('Effect HttpApi failure responses distinguish the status of each failure kind', async () => {
  const source = (await effectFiles(serverDocument()))['effect-contract.ts'];
  const declaration = source.split('\n').find(line => line.startsWith('export type UpdateRule409 ='));
  assert.match(declaration, /"stale"/);
  assert.doesNotMatch(declaration, /"invalid_input"/);
});

test('an in-memory Effect server accepts writes and optional rule searches with declared statuses', async () => {
  const directory = await mkdtemp(join(import.meta.dirname, '.effect-server-'));
  try {
    const document = serverDocument();
    for (const [name, source] of Object.entries({
      ...await effectFiles(document),
      ...await operationValidators(document),
    })) {
      await mkdir(dirname(join(directory, name)), { recursive: true });
      await writeFile(join(directory, name), source);
    }
    const entry = join(directory, 'round-trip.ts');
    await writeFile(entry, `
import * as Effect from 'effect/Effect';
import * as Layer from 'effect/Layer';
import { HttpRouter, HttpServer } from 'effect/unstable/http';
import { HttpApiBuilder, HttpApiTest } from 'effect/unstable/httpapi';
import { ProvenanceApi } from './effect-contract.js';
const handlers = HttpApiBuilder.group(ProvenanceApi, 'rules', group => group.handleAll({
  listRules: ({ query }) => Effect.succeed({ data: [query.base ?? query.file ?? 'optional'] }),
  updateRule: ({ headers, payload }) => headers['if-match'] !== 'revision-1'
    ? Effect.die(new Error('missing lowercase header'))
    : payload.data === 'conflict'
      ? Effect.fail({ error: { kind: 'stale' as const }, meta: {} })
      : Effect.succeed({ data: payload.data }),
}));
const sourceHandlers = HttpApiBuilder.group(ProvenanceApi, 'sources', group => group.handleAll({
  listSources: () => Effect.succeed({ data: [] }),
}));
const routes = HttpApiBuilder.layer(ProvenanceApi).pipe(Layer.provide(Layer.merge(handlers, sourceHandlers)));
const server = HttpRouter.toWebHandler(routes.pipe(Layer.provide(HttpServer.layerServices)));
export async function roundTrip() {
  try {
    const search = await server.handler(new Request('http://localhost/rules?query=search'));
    const write = await server.handler(new Request('http://localhost/rules/rule_a', {
      method: 'PATCH', headers: { 'content-type': 'application/json', 'If-Match': 'revision-1' },
      body: JSON.stringify({ data: 'changed' }),
    }));
    const conflict = await server.handler(new Request('http://localhost/rules/rule_a', {
      method: 'PATCH', headers: { 'content-type': 'application/json', 'If-Match': 'revision-1' },
      body: JSON.stringify({ data: 'conflict' }),
    }));
    return { search: [search.status, await search.json()], write: [write.status, await write.json()], conflict: [conflict.status, await conflict.json()] };
  } finally { await server.dispose(); }
}
export function selectedGroup() {
  return Effect.runPromise(Effect.scoped(Effect.gen(function*() {
    const client = yield* HttpApiTest.groups(ProvenanceApi, ['rules']);
    const response = yield* client.rules.listRules({ query: { query: 'search' } });
    return response;
  }).pipe(Effect.provide(handlers))));
}
`);
    const output = join(directory, 'round-trip.mjs');
    await build({ entryPoints: [entry], outfile: output, bundle: true, packages: 'external', platform: 'node', format: 'esm' });
    assert.deepEqual(await (await import(output)).roundTrip(), {
      search: [200, { data: ['optional'] }],
      write: [200, { data: 'changed' }],
      conflict: [409, { error: { kind: 'stale' }, meta: {} }],
    });
    assert.deepEqual(await (await import(output)).selectedGroup(), { data: ['optional'] });
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
