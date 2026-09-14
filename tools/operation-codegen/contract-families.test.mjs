import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { hoistSharedFamilies, requestSideNames } from './contract-families.mjs';

function responseAlternatives(schema, schemas) {
  if (schema?.$ref) return responseAlternatives(schemas[schema.$ref.split('/').at(-1)], schemas);
  if (schema?.oneOf) return schema.oneOf.flatMap(part => responseAlternatives(part, schemas));
  if (schema?.anyOf) return schema.anyOf.flatMap(part => responseAlternatives(part, schemas));
  return [schema];
}

function assertV2Envelope(name, schema, schemas, label) {
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
    assertV2Envelope(name, schemas[name], schemas, 'v2 envelope');
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
    assertV2Envelope(name, schemas[name], schemas, 'hoisted v2 envelope');
  }
});
