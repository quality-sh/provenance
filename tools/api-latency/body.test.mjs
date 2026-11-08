import assert from 'node:assert/strict';
import { test } from 'node:test';
import { findKey, freshId, patchField, requestBody, schemaValue } from './body.mjs';

const context = {
  text: 'This text is for a test.',
  idText: 'api_latency',
  fields: { actor: 'maintainer', id: 'req_new' },
};

const createRequirement = {
  $defs: {
    Status: { enum: ['active', 'discovery'], type: 'string' },
    StableId: { pattern: '^[a-z0-9_-]+$', type: 'string' },
  },
  properties: {
    actor: { type: 'string' },
    depends_on: { items: { $ref: '#/$defs/StableId' }, type: 'array' },
    description: { type: ['string', 'null'] },
    id: { $ref: '#/$defs/StableId' },
    statement: { type: 'string' },
    status: { $ref: '#/$defs/Status' },
    requirement_id: { $ref: '#/$defs/StableId' },
  },
  required: ['actor', 'id', 'statement', 'status', 'depends_on', 'requirement_id'],
  type: 'object',
};

test('a body takes fixed fields first, then seed values of the right type, then the schema', () => {
  const seed = { id: 'req_old', statement: 'The seed statement.', status: 'archived_word', depends_on: ['req_x'], description: 'Not required.' };
  assert.deepEqual(requestBody(createRequirement, seed, context), {
    actor: 'maintainer',
    id: 'req_new',
    statement: 'The seed statement.',
    status: 'active',
    depends_on: ['req_x'],
    requirement_id: 'api_latency',
  });
});

test('a reference field takes a record id from the context', () => {
  const body = requestBody(createRequirement, null, { ...context, reference: name => (name === 'requirement_id' ? 'req_seed' : undefined) });
  assert.equal(body.requirement_id, 'req_seed');
  assert.equal(body.statement, 'This text is for a test.');
  assert.deepEqual(body.depends_on, []);
});

test('a oneOf or anyOf body uses the top-level properties and the branch that fits the seed', () => {
  const schema = {
    properties: { id: { type: 'string' }, status: { type: 'string' } },
    required: ['id'],
    anyOf: [
      { properties: { status: { enum: ['draft', 'active'] } } },
      { properties: { status: { const: 'archived' }, archived_in_commit: { type: 'object' } }, required: ['status', 'archived_in_commit'] },
    ],
  };
  assert.deepEqual(requestBody(schema, null, context), { id: 'req_new' });
  assert.equal(requestBody(null, null, context), null);
});

test('schema values follow const, enum, default and type', () => {
  assert.equal(schemaValue({}, { const: 'x' }, context), 'x');
  assert.equal(schemaValue({}, { enum: ['a', 'b'] }, context), 'a');
  assert.equal(schemaValue({}, { default: 7, type: 'integer' }, context), 7);
  assert.equal(schemaValue({}, { type: 'integer', minimum: 3 }, context), 3);
  assert.equal(schemaValue({}, { type: ['boolean', 'null'] }, context), false);
  assert.deepEqual(schemaValue({}, { type: 'array', minItems: 1, items: { type: 'string' } }, context), ['This text is for a test.']);
  assert.deepEqual(schemaValue({}, { anyOf: [{ type: 'null' }, { type: 'string', pattern: '^a$' }] }, context), 'api_latency');
});

test('a new record id keeps the prefix of the seed id', () => {
  assert.equal(freshId('req_agent_runs', 'bench_http_1'), 'req_bench_http_1');
  assert.equal(freshId(undefined, 'Bench HTTP'), 'bench_http');
});

test('a PATCH changes the first text field of the schema', () => {
  assert.equal(patchField({ properties: { status: { type: 'string' }, name: { type: ['string', 'null'] }, title: { type: 'integer' } } }), 'name');
  assert.equal(patchField({ properties: { status: { type: 'string' } } }), null);
});

test('findKey reads the first value of a key depth first', () => {
  assert.equal(findKey({ data: { edit: { etag: 'sha256:1' } } }, 'etag'), 'sha256:1');
  assert.equal(findKey({ items: [{ file: 'a.rs' }] }, 'file'), 'a.rs');
  assert.equal(findKey({ etag: null }, 'etag'), undefined);
});

test('a fixed value fills the id of an object field', () => {
  const schema = {
    $defs: { Actor: { properties: { id: { type: 'string' }, identity_type: { enum: ['human', 'agent'] } }, required: ['identity_type', 'id'], type: 'object' } },
    properties: { actor: { $ref: '#/$defs/Actor' }, decision: { enum: ['accepted', 'rejected'] } },
    required: ['actor', 'decision'],
  };
  assert.deepEqual(requestBody(schema, null, context), { actor: { identity_type: 'human', id: 'maintainer' }, decision: 'accepted' });
});
