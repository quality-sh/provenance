import test from 'node:test';
import assert from 'node:assert/strict';
import { validators } from './validators.mjs';

const reference = name => ({ $ref: `#/components/schemas/${name}` });

// A paging success carries the producer's page facts in meta; a member
// success carries none. Both come from the same exported envelope shapes.
const stamp = {
  type: 'object',
  additionalProperties: false,
  required: ['serial', 'digest', 'instance_id', 'derivation', 'policy', 'attested', 'live'],
  properties: {
    serial: { type: 'integer' }, digest: { type: 'string' }, instance_id: { type: 'string' },
    derivation: { type: 'integer' }, policy: { type: 'string', const: 'catch_up' },
    attested: { type: 'array', items: { type: 'string' } }, live: { type: 'array', items: { type: 'string' } },
  },
};

const document = {
  paths: {
    '/metadata': { get: {
      operationId: 'metadata', responses: { 200: { content: { 'application/json': { schema: reference('MetadataSuccess') } } } },
    } },
    '/sources': { get: {
      operationId: 'listSources', 'x-operation-mutates': false,
      responses: {
        200: { content: { 'application/json': { schema: reference('ListSourcesSuccess') } } },
        400: { content: { 'application/json': { schema: reference('ListSourcesFailure') } } },
      },
    } },
    '/sources/{id}': { get: {
      operationId: 'getSource', 'x-operation-mutates': false,
      parameters: [{ name: 'id', in: 'path', required: true, schema: { type: 'string' } }],
      responses: {
        200: { content: { 'application/json': { schema: reference('GetSourceSuccess') } } },
        400: { content: { 'application/json': { schema: reference('GetSourceFailure') } } },
      },
    } },
  },
  components: { schemas: {
    MetadataSuccess: {
      type: 'object', additionalProperties: false, required: ['data', 'meta'],
      properties: { data: { type: 'object' }, meta: { type: 'object', additionalProperties: false, properties: {} } },
    },
    ListSourcesSuccess: {
      type: 'object', additionalProperties: false, required: ['data', 'meta'],
      properties: {
        data: { type: 'object', additionalProperties: false, required: ['items'], properties: { items: { type: 'array', items: { type: 'object' } } } },
        meta: {
          type: 'object', additionalProperties: false, required: ['limit', 'has_more'],
          properties: {
            stamp: { anyOf: [stamp, { type: 'null' }] },
            freshness_error: { type: ['string', 'null'] },
            limit: { type: 'integer', minimum: 0 },
            has_more: { type: 'boolean' },
            next_cursor: { type: ['string', 'null'] },
          },
        },
      },
    },
    ListSourcesFailure: {
      type: 'object', additionalProperties: false, required: ['error', 'meta'],
      properties: { error: { type: 'object' }, meta: { type: 'object', additionalProperties: false, properties: {} } },
    },
    GetSourceSuccess: {
      type: 'object', additionalProperties: false, required: ['data', 'meta'],
      properties: {
        data: { type: 'object', additionalProperties: false, required: ['id'], properties: { id: { type: 'string' } } },
        meta: {
          type: 'object', additionalProperties: false,
          properties: {
            stamp: { anyOf: [stamp, { type: 'null' }] },
            freshness_error: { type: ['string', 'null'] },
          },
        },
      },
    },
    GetSourceFailure: {
      type: 'object', additionalProperties: false, required: ['error', 'meta'],
      properties: { error: { type: 'object' }, meta: { type: 'object', additionalProperties: false, properties: {} } },
    },
  } },
};

test('runtime validators refuse page responses without the required page facts', async () => {
  const generated = await validators(document);
  const module = await import(
    /* webpackIgnore: true */ `data:text/javascript;base64,${Buffer.from(generated['validators.mjs']).toString('base64')}`
  );
  assert.equal(module.ListSourcesSuccess({ data: { items: [] }, meta: {} }), false);
  assert.equal(module.ListSourcesSuccess({ data: { items: [] }, meta: { limit: 50 } }), false);
  assert.equal(
    module.ListSourcesSuccess({ data: { items: [] }, meta: { limit: 50, has_more: false, next_cursor: null } }),
    true,
  );
  assert.equal(
    module.ListSourcesSuccess({ data: { items: [] }, meta: { limit: 50, has_more: true, next_cursor: 'next' } }),
    true,
  );
  // A metadata-free member response stays valid, with or without a stamp.
  assert.equal(module.GetSourceSuccess({ data: { id: 'source_a' }, meta: {} }), true);
  // Compatibility stays readable for the metadata handshake.
  assert.equal(module.MetadataSuccess({ data: {}, meta: {} }), true);
});
