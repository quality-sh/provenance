import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { effectFiles } from './effect.mjs';
import { operationValidators } from './validators.mjs';

const root = resolve(import.meta.dirname, '../..');
const reference = name => ({ $ref: `#/components/schemas/${name}` });
const response = name => ({ description: name, content: { 'application/json': { schema: reference(name) } } });

function clientDocument() {
  const failure = kind => ({
    type: 'object',
    required: ['error', 'meta'],
    properties: {
      error: { type: 'object', required: ['kind'], properties: { kind: { const: kind } } },
      meta: { type: 'object' },
    },
  });
  return {
    openapi: '3.1.0',
    info: { title: 'Client fixture', version: '1' },
    paths: {
      '/metadata': { get: { operationId: 'metadata', responses: {
        200: response('MetadataSuccess'), 400: response('MetadataFailure'),
      } } },
      '/documents/{id}': { get: {
        operationId: 'getDocument',
        parameters: [{ name: 'id', in: 'path', required: true, schema: { type: 'string' } }],
        responses: { 200: response('GetDocumentSuccess'), 400: response('GetDocumentFailure') },
      } },
    },
    components: { schemas: {
      MetadataSuccess: { type: 'object', required: ['data'], properties: { data: { type: 'string' } } },
      MetadataFailure: failure('unauthenticated'),
      GetDocumentSuccess: { type: 'object', required: ['data'], properties: { data: reference('ContractMarker') } },
      GetDocumentFailure: failure('resource_not_found'),
      ContractMarker: { type: 'string' },
    } },
  };
}

test('Effect client results and layer failures use the Effect contract family', async () => {
  const temporary = await mkdtemp(join(import.meta.dirname, '.effect-client-types-'));
  const generated = join(temporary, 'generated');
  try {
    await mkdir(generated);
    const document = clientDocument();
    for (const [name, source] of Object.entries(await effectFiles(document))) {
      await writeFile(join(generated, name), source);
    }
    for (const [name, source] of Object.entries(await operationValidators(document))) {
      await mkdir(dirname(join(generated, name)), { recursive: true });
      await writeFile(join(generated, name), source);
    }
    await writeFile(join(generated, 'client.ts'), `
export interface components { readonly schemas: {
  readonly MetadataFailure: { readonly error: { readonly kind: 'unauthenticated' }; readonly meta: unknown };
  readonly GetDocumentSuccess: { readonly data: unknown };
  readonly GetDocumentFailure: { readonly error: { readonly kind: 'resource_not_found' }; readonly meta: unknown };
} }
export type OperationFailure = { readonly mutable: string[] };
export { ConnectionError, IdentityMismatchError, MalformedResponseError, OperationError, ProtocolMismatchError } from './runtime.js';
export class HttpClient {
  static connect(_baseUrl: string, _fetch?: typeof fetch, _options?: unknown): Promise<HttpClient> { return Promise.resolve(new HttpClient()); }
  static connectWithBearer(_baseUrl: string, _bearer: string, _fetch?: typeof fetch, _options?: unknown): Promise<HttpClient> { return Promise.resolve(new HttpClient()); }
  getDocument(_call: { readonly id: string }, _options?: { readonly signal?: AbortSignal }): Promise<components['schemas']['GetDocumentSuccess']> {
    return Promise.resolve({ data: 'document' });
  }
}
`);
    await writeFile(join(generated, 'runtime.ts'), await readFile(join(root, 'tools/operation-codegen/templates/http-runtime.ts'), 'utf8'));
    await writeFile(join(temporary, 'effect-runtime.ts'), await readFile(join(root, 'packages/provenance/src/effect-runtime.ts'), 'utf8'));
    await writeFile(join(temporary, 'assertions.ts'), `
import type * as Effect from 'effect/Effect';
import type * as Layer from 'effect/Layer';
import { EffectHttpClient, ProvenanceClient } from './generated/effect-client.js';
import type { GetDocumentSuccess, GetDocumentFailure, MetadataFailure } from './generated/effect-contract.js';
import type { ClientFailure } from './effect-runtime.js';
declare const client: EffectHttpClient;
const result: Effect.Effect<GetDocumentSuccess, ClientFailure<GetDocumentFailure>> = client.getDocument({ id: 'requirement_x' });
const layer = ProvenanceClient.layer({ baseUrl: 'http://localhost' });
type Assert<T extends true> = T;
type Equal<A, B> = (<T>() => T extends A ? 1 : 2) extends (<T>() => T extends B ? 1 : 2) ? true : false;
type LayerFailure = Assert<Equal<Layer.Error<typeof layer>, ClientFailure<MetadataFailure>>>;
void result;
`);
    const result = spawnSync(process.execPath, [
      join(root, 'tools/operation-codegen/node_modules/typescript/bin/tsc'),
      '--strict', '--noEmit', '--skipLibCheck', '--target', 'es2022',
      '--module', 'nodenext', '--moduleResolution', 'nodenext',
      join(temporary, 'assertions.ts'),
    ], { cwd: root, encoding: 'utf8' });
    assert.equal(result.status, 0, result.stdout + result.stderr);
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
});
