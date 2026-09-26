import test from 'node:test';
import assert from 'node:assert/strict';
import { rustClientFiles, typescriptClient } from './templates.mjs';

test('generated clients type and encode array query parameters', () => {
  const schema = name => ({ $ref: `#/components/schemas/${name}` });
  const document = { paths: { '/fixtures': { get: {
    operationId: 'listFixtures', 'x-operation-mutates': false,
    parameters: [{
      name: 'relations', in: 'query', required: false,
      style: 'form', explode: false,
      schema: { type: 'array', items: { type: 'string' } },
    }],
    responses: {
      200: { content: { 'application/json': { schema: schema('FixtureSuccess') } } },
      400: { content: { 'application/json': { schema: schema('FixtureFailure') } } },
    },
  } } } };
  const compatibility = { wire: 9, state: 2, review_journal: 1, read_derivation: 1 };
  const typescript = typescriptClient(document, compatibility);
  const rust = Object.values(rustClientFiles(document, compatibility)).join('\n');
  assert.match(typescript, /"relations"\?: string\[\]/);
  assert.match(typescript, /call\["relations"\]!\.join\(','\)/);
  assert.match(rust, /relations: Option<&\[&str\]>/);
  assert.match(rust, /value\.join\(","\)/);
});

test('array items of a declared enum keep the closed element type', () => {
  const schema = name => ({ $ref: `#/components/schemas/${name}` });
  const document = { paths: { '/fixtures': { get: {
    operationId: 'listFixtures', 'x-operation-mutates': false,
    parameters: [{
      name: 'relations', in: 'query', required: false,
      style: 'form', explode: false,
      schema: { type: 'array', items: { type: 'string', enum: ['out', 'in', 'both'] } },
    }],
    responses: {
      200: { content: { 'application/json': { schema: schema('FixtureSuccess') } } },
      400: { content: { 'application/json': { schema: schema('FixtureFailure') } } },
    },
  } } } };
  const compatibility = { wire: 9, state: 2, review_journal: 1, read_derivation: 1 };
  const typescript = typescriptClient(document, compatibility);
  const rust = Object.values(rustClientFiles(document, compatibility)).join('\n');
  assert.match(typescript, /"relations"\?: \('out' \| 'in' \| 'both'\)\[\]/);
  assert.match(rust, /relations: Option<&\[Relations\]>/);
  assert.match(rust, /value\.iter\(\)\.map\(\|item\| item\.as_str\(\)\)\.collect::<Vec<_>>\(\)\.join\(","\)/);
});
