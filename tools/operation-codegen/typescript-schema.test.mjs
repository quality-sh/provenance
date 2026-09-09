import { test } from 'node:test';
import assert from 'node:assert/strict';
import Ajv2020 from 'ajv/dist/2020.js';
import { typescriptSchema } from './typescript-schema.mjs';

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
