import test from 'node:test';
import assert from 'node:assert/strict';
import { typescriptClient, rustClientFiles } from './templates.mjs';
import { effectClient } from './effect.mjs';

const reference = name => ({ $ref: `#/components/schemas/${name}` });
const parameter = (name, required, schema = { type: 'string' }) => ({
  name, in: name === 'id' ? 'path' : 'query', required, schema,
});

const document = {
  paths: { '/rules/{id}': { get: {
    operationId: 'getRule',
    'x-operation-mutates': false,
    parameters: [
      parameter('id', true), parameter('query', false, { type: 'string', enum: ['trace'] }),
      parameter('direction', false),
    ],
    responses: {
      200: { content: { 'application/json': { schema: reference('GetRuleSuccess') } } },
      400: { content: { 'application/json': { schema: reference('GetRuleFailure') } } },
    },
    'x-provenance-query-variants': [
      { selector: null, parameters: [parameter('id', true)], success: reference('GetRuleBaseSuccess'), failure: reference('GetRuleBaseFailure') },
      { selector: 'trace', parameters: [parameter('id', true), parameter('query', true, { type: 'string', const: 'trace' }), parameter('direction', false)], success: reference('GetRuleTraceSuccess'), failure: reference('GetRuleTraceFailure') },
    ],
  } } },
  components: { schemas: {} },
};

test('Promise generation overloads selectors and validates the selected contracts', () => {
  const source = typescriptClient(document, { wire: 2, state: 1, review_journal: 1, read_derivation: 1 });
  assert.match(source, /export type GetRuleBaseInput = \{ "id": string \}/);
  assert.match(source, /export type GetRuleTraceInput = \{ "id": string; "query": 'trace'; "direction"\?: string \}/);
  assert.match(source, /getRule\(call: GetRuleTraceInput.*Promise<GetRuleTraceSuccess>/);
  assert.match(source, /validate\.GetRuleTraceSuccess/);
  assert.match(source, /validate\.GetRuleTraceFailure/);
});

test('Rust generation uses a closed query input and decodes the selected result', () => {
  const files = rustClientFiles(document, { wire: 2, state: 1, review_journal: 1, read_derivation: 1 });
  const source = files['operations/get_rule.rs'];
  assert.match(source, /pub enum GetRuleInput<'a>/);
  assert.match(source, /Trace \{ id: &'a str, direction: Option<&'a str> \}/);
  assert.match(source, /pub enum GetRuleOutput/);
  assert.match(source, /runtime::validate\(&value, "GetRuleTraceSuccess"/);
  assert.match(source, /GetRuleOutput::Trace/);
});

test('Effect generation retains selector-specific success and failure types', () => {
  const source = effectClient(document);
  assert.match(source, /getRule\(call: GetRuleTraceInput\): Effect\.Effect<GetRuleTraceSuccess, ClientFailure<GetRuleTraceFailure>>/);
  assert.match(source, /getRule\(call: GetRuleBaseInput\): Effect\.Effect<GetRuleBaseSuccess, ClientFailure<GetRuleBaseFailure>>/);
});
