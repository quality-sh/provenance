import { test } from 'node:test';
import assert from 'node:assert/strict';
import Ajv2020 from 'ajv/dist/2020.js';
import { clientTypeSchema, typescriptSchema } from './typescript-schema.mjs';
import { validators } from './validators.mjs';

test('reference adapter retains unevaluated property scope and existing conjunctions', () => {
  const document = { components: { schemas: {
    Base: { type: 'object', properties: { value: { type: 'string' } }, required: ['value'] },
    Tagged: { $ref: '#/components/schemas/Base', type: 'object', properties: { kind: { const: 'tagged' } }, required: ['kind'], allOf: [{ maxProperties: 2 }], unevaluatedProperties: false },
  } } };
  const adapted = typescriptSchema(document);
  const compile = doc => new Ajv2020({ strict: false }).compile({ $ref: '#/components/schemas/Tagged', ...doc });
  const original = compile(document);
  const converted = compile(adapted);
  for (const [value, expected] of [[{ value: 'x', kind: 'tagged' }, true], [{ value: 'x', kind: 'other' }, false], [{ value: 'x', kind: 'tagged', extra: 1 }, false]]) {
    assert.equal(original(value), expected);
    assert.equal(converted(value), expected);
  }
  assert.equal(document.components.schemas.Tagged.$ref, '#/components/schemas/Base');
});

test('archive refinement stays in wire validation when client types use optional fields', async () => {
  const rule = {
    type: 'object',
    properties: {
      status: { enum: ['active', 'archived'] },
      archived_in_commit: { anyOf: [
        { type: 'object', properties: { commit: { type: 'string', pattern: '^[a-f0-9]{40}$' } }, required: ['commit'] },
        { type: 'null' },
      ] },
    },
    required: ['status'],
    'x-provenance-validation-only-any-of': true,
    anyOf: [
      { properties: { status: { const: 'active' }, archived_in_commit: { type: 'null' } } },
      { properties: { status: { const: 'archived' }, archived_in_commit: { type: 'object' } }, required: ['archived_in_commit'] },
    ],
  };
  const document = { components: { schemas: { Rule: rule } } };
  const original = structuredClone(document);
  for (const adapt of [clientTypeSchema, typescriptSchema]) {
    const adapted = adapt(document).components.schemas.Rule;
    assert.equal(adapted.anyOf, undefined);
    assert.deepEqual(adapted.properties, rule.properties);
    assert.deepEqual(adapted.required, ['status']);
  }
  const files = await validators(document, ['Rule'], 'archive', false);
  const wire = await import(`data:text/javascript;base64,${Buffer.from(files['archive.mjs']).toString('base64')}`);
  const stamp = { commit: 'a'.repeat(40) };
  for (const [value, expected] of [
    [{ status: 'active' }, true],
    [{ status: 'active', archived_in_commit: null }, true],
    [{ status: 'active', archived_in_commit: stamp }, false],
    [{ status: 'archived' }, false],
    [{ status: 'archived', archived_in_commit: null }, false],
    [{ status: 'archived', archived_in_commit: {} }, false],
    [{ status: 'archived', archived_in_commit: stamp }, true],
  ]) assert.equal(wire.Rule(value), expected);
  assert.deepEqual(document, original);
});
