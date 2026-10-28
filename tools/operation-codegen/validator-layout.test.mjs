import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { tmpdir } from 'node:os';
import { build } from 'esbuild';
import { effectFiles } from './effect.mjs';
import { operationValidators } from './validators.mjs';

const ENTRY_SIZE_LIMIT = 8_000;
const reference = name => ({ $ref: `#/components/schemas/${name}` });
const response = name => ({ description: name, content: { 'application/json': { schema: reference(name) } } });

const document = {
  openapi: '3.1.0',
  info: { title: 'Validator fixture', version: '1' },
  paths: {
    '/metadata': { get: { operationId: 'metadata', responses: { 200: response('MetadataSuccess'), 400: response('MetadataFailure') } } },
    '/rules': { get: { operationId: 'listRules', responses: { 200: response('ListRulesSuccess'), 400: response('ListRulesFailure') } } },
    '/sources': { get: { operationId: 'listSources', responses: { 200: response('ListSourcesSuccess'), 400: response('ListSourcesFailure') } } },
  },
  components: { schemas: {
    MetadataSuccess: { type: 'object', required: ['data'], properties: { data: { type: 'string' } } },
    MetadataFailure: { type: 'object', required: ['error'], properties: { error: { type: 'string' } } },
    ListRulesSuccess: { type: 'object', required: ['data'], properties: { data: { type: 'array', items: { type: 'string' } } } },
    ListRulesFailure: { type: 'object', required: ['error'], properties: { error: { type: 'string' } } },
    ListSourcesSuccess: { type: 'object', required: ['data'], properties: { data: { type: 'array', items: { type: 'string' } } } },
    ListSourcesFailure: { type: 'object', required: ['error'], properties: { error: { type: 'string' } } },
  } },
};

test('Effect entry keeps every operation validator in its own lazy chunk', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'validator-layout-'));
  const generated = join(directory, 'generated');
  try {
    await mkdir(generated);
    const files = {
      ...await effectFiles(document),
      ...await operationValidators(document),
    };
    for (const [name, source] of Object.entries(files)) {
      await mkdir(dirname(join(generated, name)), { recursive: true });
      await writeFile(join(generated, name), source);
    }
    await writeFile(join(generated, 'client.ts'), `
export class HttpClient {
  static connect(): Promise<HttpClient> { return Promise.resolve(new HttpClient()); }
  static connectWithBearer(): Promise<HttpClient> { return Promise.resolve(new HttpClient()); }
  listRules(): Promise<unknown> { return Promise.resolve({ data: [] }); }
  listSources(): Promise<unknown> { return Promise.resolve({ data: [] }); }
}
`);
    await writeFile(join(directory, 'effect-runtime.ts'), `
export class ClientRuntime { run(): never { throw new Error('not called'); } }
export const requestEffect = () => undefined;
export const connectionFailure = () => undefined;
export type ClientFailure<Failure> = Failure;
`);
    const entry = join(directory, 'entry.ts');
    await writeFile(entry, `
import * as Effect from 'effect/Effect';
import { EffectHttpClient } from './generated/effect-client.js';
export const result = Effect.flatMap(
  EffectHttpClient.connect({ baseUrl: 'http://localhost' }),
  client => client.listRules({}),
);
`);

    const result = await build({
      entryPoints: [entry],
      outdir: join(directory, 'bundle'),
      bundle: true,
      splitting: true,
      packages: 'external',
      platform: 'browser',
      format: 'esm',
      minify: true,
      metafile: true,
      write: false,
    });
    const outputs = Object.entries(result.metafile.outputs);
    const [entryName, entryOutput] = outputs.find(([, output]) => output.entryPoint?.endsWith('/entry.ts'));
    assert.ok(entryOutput.bytes < ENTRY_SIZE_LIMIT,
      `entry chunk is ${entryOutput.bytes} bytes; limit is ${ENTRY_SIZE_LIMIT}`);

    const chunks = new Map(['metadata', 'listRules', 'listSources'].map(operation => {
      const inputSuffix = `/validators/${operation}.mjs`;
      const matches = outputs.filter(([, output]) => Object.keys(output.inputs)
        .some(input => input.endsWith(inputSuffix)));
      assert.equal(matches.length, 1, `${operation} must have one validator chunk`);
      return [operation, matches[0][0]];
    }));
    assert.equal(new Set(chunks.values()).size, chunks.size,
      'each operation must have a separate validator chunk');
    assert.ok([...chunks.values()].every(chunk => chunk !== entryName),
      'the entry chunk must not contain validators');
    assert.ok(Object.keys(entryOutput.inputs).every(input => !input.includes('/validators/')),
      'the entry chunk must not include validator input');
    assert.ok(entryOutput.imports.some(item => item.kind === 'dynamic-import'
      && item.path === chunks.get('listSources')),
    'the unused listSources validator must stay a lazy chunk');
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test('importing one union matcher does not load schemas or validators', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'matcher-layout-'));
  try {
    const fixture = structuredClone(document);
    fixture.components.schemas.ListRulesSuccessGraphNode = {
      oneOf: [
        { type: 'object', required: ['node_type'], properties: { node_type: { const: 'rule' } } },
        { type: 'object', required: ['node_type'], properties: { node_type: { const: 'source' } } },
      ],
    };
    fixture.components.schemas.ListRulesSuccess.properties.data.items = reference('ListRulesSuccessGraphNode');
    const files = {
      ...await effectFiles(fixture),
      ...await operationValidators(fixture),
    };
    for (const [name, source] of Object.entries(files)) {
      await mkdir(dirname(join(directory, name)), { recursive: true });
      await writeFile(join(directory, name), source);
    }
    await writeFile(join(directory, 'effect.ts'), `
export * from './effect-contract.js';
export * from './effect-matchers.js';
`);
    await writeFile(join(directory, 'package.json'), JSON.stringify({ type: 'module', sideEffects: false }));
    await writeFile(join(directory, 'entry.ts'), `
import { matchListRulesSuccessGraphNode } from './effect.js';
export const match = matchListRulesSuccessGraphNode;
`);
    const result = await build({
      entryPoints: [join(directory, 'entry.ts')],
      outfile: join(directory, 'bundle.js'),
      bundle: true,
      packages: 'external',
      platform: 'browser',
      format: 'esm',
      minify: true,
      metafile: true,
      write: false,
    });
    const inputs = Object.keys(result.metafile.inputs);
    const included = Object.values(result.metafile.outputs)[0].inputs;
    assert.ok(inputs.some(input => input.endsWith('/effect-matchers.ts')));
    assert.equal(Object.entries(included).find(([input]) => input.endsWith('/effect-contract.ts'))?.[1].bytesInOutput ?? 0, 0,
      'a matcher must not load the Effect schema contract');
    assert.ok(Object.entries(included).filter(([input]) => input.includes('/validators/'))
      .every(([, value]) => value.bytesInOutput === 0),
      'a matcher must not load operation validators');
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
