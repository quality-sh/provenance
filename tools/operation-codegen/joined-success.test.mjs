import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { effectFiles } from './effect.mjs';
import { operationValidators } from './validators.mjs';

const root = resolve(import.meta.dirname, '../..');
const ref = name => ({ $ref: `#/components/schemas/${name}` });
const response = name => ({ description: name, content: { 'application/json': { schema: ref(name) } } });

function document() {
  const envelope = item => ({
    type: 'object', required: ['data', 'meta'], properties: {
      data: { type: 'object', required: ['items'], properties: { items: { type: 'array', items: item } } },
      meta: { type: 'object', properties: { stamp: ref('FreshnessStamp') } },
    },
  });
  const failure = {
    type: 'object', required: ['error', 'meta'], properties: {
      error: { type: 'object', required: ['kind'], properties: { kind: { const: 'invalid_input' } } },
      meta: { type: 'object' },
    },
  };
  return {
    openapi: '3.1.0', info: { title: 'Joined success fixture', version: '1' },
    paths: {
      '/metadata': { get: { operationId: 'metadata', responses: {
        200: response('MetadataSuccess'), 400: response('MetadataFailure'),
      } } },
      '/records': { get: {
        operationId: 'listRecords',
        'x-provenance-query-variants': [
          { selector: null, parameters: [], success: ref('ListRecordsBaseSuccess'), failure: ref('ListRecordsFailure') },
          { selector: 'search', parameters: [{ name: 'query', in: 'query', required: true }], success: ref('ListRecordsSearchSuccess'), failure: ref('ListRecordsFailure') },
        ],
        responses: { 200: response('ListRecordsSuccess'), 400: response('ListRecordsFailure') },
      } },
    },
    components: { schemas: {
      MetadataSuccess: { type: 'object', required: ['data'], properties: { data: { type: 'string' } } },
      MetadataFailure: failure,
      ListRecordsFailure: failure,
      RecordStamp: { type: 'object', additionalProperties: false, required: ['at', 'commit'], properties: { at: { type: 'string' }, commit: { type: 'string' } } },
      FreshnessStamp: { type: 'object', additionalProperties: false, required: ['serial'], properties: { serial: { type: 'integer' } } },
      RecordNode: { type: 'object', required: ['node_type'], properties: { node_type: { const: 'record' }, created: ref('RecordStamp') } },
      JoinedRecordNode: { type: 'object', required: ['node_type'], properties: { node_type: { const: 'record' }, created: ref('FreshnessStamp') } },
      ListRecordsBaseSuccess: envelope(ref('RecordNode')),
      ListRecordsSearchSuccess: envelope(ref('RecordNode')),
      ListRecordsSuccess: { anyOf: [envelope(ref('RecordNode')), envelope(ref('JoinedRecordNode'))] },
    } },
  };
}

test('joined query success uses the exact variant success types', async () => {
  const temporary = await mkdtemp(join(import.meta.dirname, '.joined-success-'));
  try {
    const fixture = document();
    for (const [name, source] of Object.entries({
      ...await effectFiles(fixture),
      ...await operationValidators(fixture),
    })) {
      await mkdir(dirname(join(temporary, name)), { recursive: true });
      await writeFile(join(temporary, name), source);
    }
    await writeFile(join(temporary, 'assertions.ts'), `
import type { ListRecordsSuccess } from './effect-contract.js';
type Equal<A, B> = (<T>() => T extends A ? 1 : 2) extends (<T>() => T extends B ? 1 : 2) ? true : false;
type Assert<T extends true> = T;
type Item = ListRecordsSuccess['data']['items'][number];
type Created<T> = T extends { readonly created?: infer Stamp } ? NonNullable<Stamp> : never;
type JoinedCreatedIsRecordStamp = Assert<Equal<Created<Item>, { readonly at: string; readonly commit: string }>>;
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
