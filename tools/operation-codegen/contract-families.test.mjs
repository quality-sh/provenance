import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { hoistSharedFamilies, requestSideNames } from './contract-families.mjs';

// The wire refuses unknown fields on request bodies (invalid_input/unknown_field
// is a wired failure kind; the Rust request structs carry
// #[serde(deny_unknown_fields)]). Response-side strictness is per type: most
// responses stay forward-compatible, but the ideation write results are
// `deny_unknown_fields` structs too. The exported OpenAPI mirrors Rust truth in
// both directions, so the pin below fails if either side drifts.
const CLOSED_RESPONSE_ENVELOPES = new Set([
  'CreateAssertionSuccessOutput',
  'CreateContributionSuccessOutput',
  'CreateDispositionSuccessOutput',
  'CreateProposalSuccessOutput',
  'CreateSynthesisPacketSuccessOutput',
  'UpsertContributionSuccessOutput',
  'UpsertSynthesisPacketSuccessOutput',
]);

test('request envelopes are closed; response envelopes match Rust strictness', async () => {
  const document = JSON.parse(await readFile(new URL('../../contracts/operations/openapi.json', import.meta.url), 'utf8'));
  const schemas = document.components.schemas;
  const requestEnvelopes = Object.keys(schemas).filter(name => /RequestInput$/.test(name));
  assert.ok(requestEnvelopes.length > 0);
  for (const name of requestEnvelopes) {
    assert.equal(schemas[name].additionalProperties, false, `${name}: request envelopes refuse unknown fields`);
  }
  const responseEnvelopes = Object.keys(schemas).filter(name => /(SuccessOutput|FailureOutput)$/.test(name));
  assert.ok(responseEnvelopes.length > 0);
  for (const name of responseEnvelopes) {
    const expected = CLOSED_RESPONSE_ENVELOPES.has(name) ? false : undefined;
    assert.equal(schemas[name].additionalProperties, expected,
      `${name}: response envelope strictness changed; update CLOSED_RESPONSE_ENVELOPES with the Rust structs`);
  }
  // Nested request objects follow their own Rust structs: most deny unknown
  // fields, but a few (PostMessageInput, ThreadParent, SourceReference,
  // ResolutionInput, ArtifactLink) deliberately stay lenient.
  const lenientNested = [...requestSideNames(document)]
    .filter(name => /RequestInput/.test(name) && schemas[name]?.type === 'object' && schemas[name].additionalProperties !== false);
  assert.ok(lenientNested.includes('PostThreadMessageRequestInputThreadParent'));
  assert.ok(lenientNested.includes('CreateResolutionRequestInputResolutionInput'));
});

test('family hoisting keeps request and response shapes apart', async () => {
  const document = JSON.parse(await readFile(new URL('../../contracts/operations/openapi.json', import.meta.url), 'utf8'));
  const contract = hoistSharedFamilies(document);
  const schemas = contract.components.schemas;
  for (const name of requestSideNames(contract)) {
    const body = schemas[name];
    if (body === undefined) throw new Error(`request-side name ${name} missing after hoisting`);
    if (body.type === 'object' && /RequestInput$/.test(name)) {
      assert.equal(body.additionalProperties, false, `${name}: hoisted request envelope must stay closed`);
    }
  }
  for (const name of Object.keys(schemas).filter(name => /(SuccessOutput|FailureOutput)$/.test(name))) {
    const expected = CLOSED_RESPONSE_ENVELOPES.has(name) ? false : undefined;
    assert.equal(schemas[name].additionalProperties, expected, `${name}: hoisted response envelope strictness changed`);
  }
});
