import test from 'node:test';
import assert from 'node:assert/strict';
import { typescriptFiles } from './typescript.mjs';
import { effectFiles } from './effect.mjs';

const reference = name => ({ $ref: `#/components/schemas/${name}` });
const response = name => ({ description: name, content: { 'application/json': { schema: reference(name) } } });

test('generated clients load only the validator module for the called operation', async () => {
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
  const files = await typescriptFiles(document, {});
  assert.ok(files['validators/metadata.mjs']);
  assert.ok(files['validators/listRules.mjs']);
  assert.ok(files['validators/listSources.mjs']);
  assert.equal(files['validators.mjs'], undefined);
  assert.doesNotMatch(files['client.ts'], /import \* as validate/);
  assert.match(files['client.ts'], /import\('\.\/validators\/listRules\.mjs'\)/);
  assert.match(files['client.ts'], /import\('\.\/validators\/listSources\.mjs'\)/);

  const effect = await effectFiles(document);
  assert.match(effect['effect-client.ts'], /import type \{[^}]+\} from '\.\/effect-contract\.js';/s);
  assert.match(effect['effect-client.ts'], /import\('\.\/validators\/listRules\.mjs'\)/);
  assert.match(effect['effect-client.ts'], /import\('\.\/validators\/listSources\.mjs'\)/);
  assert.doesNotMatch(effect['effect-client.ts'], /import \* as wire|import \* as validate/);
});
