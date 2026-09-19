import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile, mkdtemp, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import ts from 'typescript';
import { typescriptClient, rustClientFiles } from './templates.mjs';
import { effectClient } from './effect.mjs';
import { validators } from './validators.mjs';

const reference = name => ({ $ref: `#/components/schemas/${name}` });
const parameter = (name, required, schema = { type: 'string' }) => ({
  name, in: name === 'id' ? 'path' : 'query', required, schema,
});

const object = properties => ({ type: 'object', additionalProperties: false, required: Object.keys(properties), properties });
const success = kind => object({ data: object({ kind: { type: 'string', const: kind } }), meta: object({}) });
const failure = kind => object({ error: object({ kind: { type: 'string', const: kind } }), meta: object({}) });
const compatibility = { wire: 2, state: 1, review_journal: 1, read_derivation: 1 };

const document = {
  paths: {
    '/metadata': { get: {
      operationId: 'metadata', responses: { 200: { content: { 'application/json': { schema: reference('MetadataSuccess') } } } },
    } },
    '/rules/{id}': { get: {
    operationId: 'getRule',
    'x-operation-mutates': false,
    parameters: [
      parameter('id', true), parameter('query', false, { type: 'string', enum: ['trace'] }),
      parameter('direction', true),
    ],
    responses: {
      200: { content: { 'application/json': { schema: reference('GetRuleSuccess') } } },
      400: { content: { 'application/json': { schema: reference('GetRuleFailure') } } },
    },
    'x-provenance-query-variants': [
      { selector: null, parameters: [parameter('id', true)], success: reference('GetRuleBaseSuccess'), failure: reference('GetRuleBaseFailure') },
      { selector: 'trace', parameters: [parameter('id', true), parameter('query', true, { type: 'string', const: 'trace' }), parameter('direction', true)], success: reference('GetRuleTraceSuccess'), failure: reference('GetRuleTraceFailure') },
    ],
    } },
  },
  components: { schemas: {
    MetadataSuccess: object({ data: object({
      compatibility: object({
        wire: { type: 'integer' }, state: { type: 'integer' },
        review_journal: { type: 'integer' }, read_derivation: { type: 'integer' },
      }),
      repository: { type: ['string', 'null'] }, scope: { type: ['string', 'null'] },
    }), meta: object({}) }),
    GetRuleSuccess: { anyOf: [reference('GetRuleBaseSuccess'), reference('GetRuleTraceSuccess')] },
    GetRuleFailure: { anyOf: [reference('GetRuleBaseFailure'), reference('GetRuleTraceFailure')] },
    GetRuleBaseSuccess: success('base'),
    GetRuleTraceSuccess: success('trace'),
    GetRuleBaseFailure: failure('base_failure'),
    GetRuleTraceFailure: failure('trace_failure'),
  } },
};

const optionalDirectionDocument = structuredClone(document);
const optionalDirectionOperation = optionalDirectionDocument.paths['/rules/{id}'].get;
optionalDirectionOperation.parameters.find(parameter => parameter.name === 'direction').required = false;
optionalDirectionOperation['x-provenance-query-variants'][1].parameters
  .find(parameter => parameter.name === 'direction').required = false;

test('Promise generation overloads selectors and validates the selected contracts', () => {
  const source = typescriptClient(document, { wire: 2, state: 1, review_journal: 1, read_derivation: 1 });
  assert.match(source, /export type GetRuleBaseInput = \{ "id": string; "query"\?: undefined \}/);
  assert.match(source, /export type GetRuleTraceInput = \{ "id": string; "query": 'trace'; "direction": string \}/);
  assert.match(source, /async getRule\(call: \{ "id": string; "query"\?: 'trace'; "direction"\?: string \}/);
  assert.match(source, /getRule\(call: GetRuleTraceInput.*Promise<GetRuleTraceSuccess>/);
  assert.match(source, /validate\.GetRuleTraceSuccess/);
  assert.match(source, /validate\.GetRuleTraceFailure/);
});

test('Rust generation uses a closed query input and decodes the selected result', () => {
  const files = rustClientFiles(document, { wire: 2, state: 1, review_journal: 1, read_derivation: 1 });
  const source = files['operations/get_rule.rs'];
  assert.match(source, /pub enum GetRuleInput<'a>/);
  assert.match(source, /Base\(Box<GetRuleBaseSuccess>\)/);
  assert.match(source, /GetRuleInput::Base \{ id \} => self\.get_rule_base\(id\)\.await/);
  assert.match(source, /async fn get_rule_base\(&self, id: &str\)/);
  assert.match(source, /let url = format!\("\{\}\/rules\/\{\}", self\.base_url, runtime::path\(id\)\);/);
  assert.doesNotMatch(source, /"\/rules\/\{id\}"/);
  assert.match(source, /runtime::validate\(&value, "GetRuleTraceSuccess"/);
  assert.match(source, /GetRuleOutput::Trace/);
});

test('Rust generation distinguishes required and optional query fields', () => {
  const required = rustClientFiles(document, compatibility)['operations/get_rule.rs'];
  const optional = rustClientFiles(optionalDirectionDocument, compatibility)['operations/get_rule.rs'];
  assert.match(required, /Trace \{ id: &'a str, direction: &'a str \}/);
  assert.match(optional, /Trace \{ id: &'a str, direction: Option<&'a str> \}/);
});

test('Effect generation retains selector-specific success and failure types', () => {
  const source = effectClient(document);
  assert.match(source, /getRule\(call: GetRuleTraceInput\): Effect\.Effect<GetRuleTraceSuccess, ClientFailure<GetRuleTraceFailure>>/);
  assert.match(source, /getRule\(call: GetRuleBaseInput\): Effect\.Effect<GetRuleBaseSuccess, ClientFailure<GetRuleBaseFailure>>/);
  assert.match(source, /type GetRuleTraceSuccess.*type GetRuleTraceFailure.*from '\.\/client\.js'/);
  assert.match(source, /export type \{ GetRuleBaseInput, GetRuleTraceInput \} from '\.\/client\.js'/);
  assert.doesNotMatch(source, /export type \{[^}]*GetRuleTraceSuccess/);
});

async function generatedClient() {
  const directory = await mkdtemp(join(tmpdir(), 'query-correspondence-'));
  const source = typescriptClient(document, compatibility);
  const runtime = await readFile(new URL('./templates/http-runtime.ts', import.meta.url), 'utf8');
  const compiled = value => ts.transpileModule(value, {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 },
  }).outputText;
  await writeFile(join(directory, 'package.json'), '{"type":"module"}');
  await writeFile(join(directory, 'client.js'), compiled(source));
  await writeFile(join(directory, 'runtime.js'), compiled(runtime));
  await writeFile(join(directory, 'schema.js'), 'export {};\n');
  for (const [name, content] of Object.entries(await validators(document))) {
    await writeFile(join(directory, name), content);
  }
  return { module: await import(join(directory, 'client.js')), close: () => rm(directory, { recursive: true, force: true }) };
}

const metadata = { data: { compatibility, repository: null, scope: null }, meta: {} };

test('selected runtime validators reject another query result contract', async () => {
  const generated = await generatedClient();
  try {
    for (const [status, body] of [
      [200, { data: { kind: 'base' }, meta: {} }],
      [400, { error: { kind: 'base_failure' }, meta: {} }],
    ]) {
      const client = await generated.module.HttpClient.connect('https://example.test', async (_url, init) =>
        init?.method === 'GET' ? Response.json(body, { status }) : Response.json(metadata));
      await assert.rejects(
        client.getRule({ id: 'rule_a', query: 'trace', direction: 'in' }),
        generated.module.MalformedResponseError,
      );
    }
  } finally {
    await generated.close();
  }
});

test('selected calls encode their discriminator and reject unknown selectors', async () => {
  const generated = await generatedClient();
  let requested;
  try {
    const client = await generated.module.HttpClient.connect('https://example.test', async (url, init) => {
      if (init?.method !== 'GET') return Response.json(metadata);
      requested = String(url);
      return Response.json({ data: { kind: 'trace' }, meta: {} });
    });
    assert.deepEqual(await client.getRule({ id: 'rule/a', query: 'trace', direction: 'in' }), {
      data: { kind: 'trace' }, meta: {},
    });
    assert.equal(requested, 'https://example.test/rules/rule%2Fa?query=trace&direction=in');
    await assert.rejects(client.getRule({ id: 'rule_a', query: 'invented' }), TypeError);
  } finally {
    await generated.close();
  }
});
