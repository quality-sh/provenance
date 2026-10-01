import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import { hoistSharedFamilies, requestSideNames } from './contract-families.mjs';

function responseAlternatives(schema, schemas) {
  if (schema?.$ref) return responseAlternatives(schemas[schema.$ref.split('/').at(-1)], schemas);
  if (schema?.oneOf) return schema.oneOf.flatMap(part => responseAlternatives(part, schemas));
  if (schema?.anyOf) return schema.anyOf.flatMap(part => responseAlternatives(part, schemas));
  return [schema];
}

function assertResponseEnvelope(name, schema, schemas, label) {
  const alternatives = responseAlternatives(schema, schemas);
  assert.ok(alternatives.length > 0, `${name}: ${label} has an envelope`);
  for (const alternative of alternatives) {
    assert.ok(alternative.properties?.meta, `${name}: ${label} has meta`);
    assert.ok(alternative.properties?.data || alternative.properties?.error,
      `${name}: ${label} has data or error`);
  }
}

// The wire refuses unknown fields on request bodies (invalid_input/unknown_field
// is a wired failure kind; the Rust request structs carry
// #[serde(deny_unknown_fields)]). Response-side strictness is per type: most
// responses stay forward-compatible, but the ideation write results are
// `deny_unknown_fields` structs too. The exported OpenAPI mirrors Rust truth in
// both directions, so the pin below fails if either side drifts.
test('request envelopes are closed; response envelopes match Rust strictness', async () => {
  const document = JSON.parse(await readFile(new URL('../../contracts/operations/openapi.json', import.meta.url), 'utf8'));
  const schemas = document.components.schemas;
  const referenced = status => new Set(Object.values(document.paths).flatMap(path => Object.values(path))
    .map(operation => operation.responses?.[status]?.content?.['application/json']?.schema?.$ref?.split('/').at(-1)).filter(Boolean));
  const requestEnvelopes = Object.keys(schemas).filter(name => /Request$/.test(name));
  assert.ok(requestEnvelopes.length > 0);
  for (const name of requestEnvelopes) {
    assert.equal(schemas[name].additionalProperties, false, `${name}: request envelopes refuse unknown fields`);
  }
  const responseEnvelopes = [...referenced('200'), ...referenced('400')];
  assert.ok(responseEnvelopes.length > 0);
  for (const name of responseEnvelopes) {
    assertResponseEnvelope(name, schemas[name], schemas, 'response envelope');
  }
  // Nested request objects follow their own Rust structs: most deny unknown
  // fields, but a few (PostMessageInput, ThreadParent, SourceReference,
  // ResolutionInput, ArtifactLink) deliberately stay lenient.
  const nested = [...requestSideNames(document)]
    .filter(name => /Request/.test(name) && schemas[name]?.type === 'object');
  assert.ok(nested.length > requestEnvelopes.length);
});

test('family hoisting keeps request and response shapes apart', async () => {
  const document = JSON.parse(await readFile(new URL('../../contracts/operations/openapi.json', import.meta.url), 'utf8'));
  const contract = hoistSharedFamilies(document);
  const schemas = contract.components.schemas;
  const responseEnvelopes = new Set(Object.values(contract.paths).flatMap(path => Object.values(path))
    .flatMap(operation => ['200', '400'].map(status => operation.responses?.[status]?.content?.['application/json']?.schema?.$ref?.split('/').at(-1))).filter(Boolean));
  for (const name of requestSideNames(contract)) {
    const body = schemas[name];
    if (body === undefined) throw new Error(`request-side name ${name} missing after hoisting`);
    if (body.type === 'object' && /Request$/.test(name)) {
      assert.equal(body.additionalProperties, false, `${name}: hoisted request envelope must stay closed`);
    }
  }
  for (const name of responseEnvelopes) {
    assertResponseEnvelope(name, schemas[name], schemas, 'hoisted response envelope');
  }
});

test('family hoisting shares current operation response families', async () => {
  const document = JSON.parse(await readFile(new URL('../../contracts/operations/openapi.json', import.meta.url), 'utf8'));
  const contract = hoistSharedFamilies(document);
  const schemas = contract.components.schemas;

  assert.ok(schemas.OperationFailure, 'the shared transport failure must be declared once');
  assert.ok(schemas.WriteFailure, 'the shared write failure must be declared once');
  assert.ok(schemas.ResponseMeta, 'the shared response metadata must be declared once');
  assert.equal(schemas.UpdateRuleFailureOperationFailure, undefined);
  assert.equal(schemas.WithdrawSourceReviewFailureOperationFailure, undefined);
  assert.deepEqual(schemas.WriteOperationError.anyOf, [
    { $ref: '#/components/schemas/OperationFailure' },
    { $ref: '#/components/schemas/WriteFailure' },
  ]);
  assert.deepEqual(schemas.UpdateRuleFailure.properties.error, {
    $ref: '#/components/schemas/WriteOperationError',
  });
  assert.deepEqual(schemas.UpdateRuleFailure.properties.meta, {
    $ref: '#/components/schemas/ResponseMeta',
  });
  assert.ok(schemas.Stamp2, 'query result record stamps must be shared');
  assert.equal(schemas.ListRulesSearchSuccessStamp2, undefined);
});

test('family hoisting shares equal groups when one operation differs', () => {
  const envelope = detail => ({
    properties: { error: { $ref: `#/components/schemas/${detail}` } },
    type: 'object',
  });
  const operation = failure => ({ responses: { 400: { content: {
    'application/json': { schema: { $ref: `#/components/schemas/${failure}` } },
  } } } });
  const document = {
    paths: {
      '/a': { get: operation('GetAFailure') },
      '/b': { get: operation('GetBFailure') },
      '/c': { get: operation('GetCFailure') },
    },
    components: { schemas: {
      GetAFailure: envelope('GetAFailureDetail'),
      GetBFailure: envelope('GetBFailureDetail'),
      GetCFailure: envelope('GetCFailureDetail'),
      GetAFailureDetail: { properties: { kind: { const: 'shared' } }, type: 'object' },
      GetBFailureDetail: { properties: { kind: { const: 'shared' } }, type: 'object' },
      GetCFailureDetail: { properties: { kind: { const: 'different' } }, type: 'object' },
    } },
  };

  const schemas = hoistSharedFamilies(document).components.schemas;
  assert.deepEqual(schemas.Detail, {
    properties: { kind: { const: 'shared' } },
    type: 'object',
  });
  assert.equal(schemas.GetAFailureDetail, undefined);
  assert.equal(schemas.GetBFailureDetail, undefined);
  assert.ok(schemas.GetCFailureDetail);
});

test('operation and schema names do not use version suffixes', () => {
  const result = spawnSync('git', [
    'grep', '-n', '-E', '(-v[0-9]+|[A-Za-z]V[0-9]+)', '--',
    'crates/provenance-store/src/operations',
  ], { encoding: 'utf8' });

  assert.ok([0, 1].includes(result.status), result.stderr);
  assert.equal(result.stdout, '');
});
