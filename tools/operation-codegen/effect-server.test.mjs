import test from 'node:test';
import assert from 'node:assert/strict';
import { effectFiles } from './effect.mjs';

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
