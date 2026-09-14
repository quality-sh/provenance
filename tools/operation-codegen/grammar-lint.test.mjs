import test from 'node:test';
import assert from 'node:assert/strict';
import { documentGrammarErrors, lintFixture, lintRoutes } from './grammar-lint.mjs';

function baseFixture() {
  return {
    catalog_size: 1,
    review_size: 0,
    accounting: { create: 1 },
    collections: ['requirements'],
    compute_collections: ['statement-checks'],
    variables: ['collection', 'id'],
    actions: ['claim'],
    queries: ['search'],
    base_statuses: [400, 401, 403, 404, 500, 503],
    routes: [
      { id: 'post-collection', method: 'POST', path: '/{collection}', mutates: true, statuses: [400, 401, 403, 404, 409, 500, 503], response: 'resource' },
    ],
    operations: [
      { legacy: 'create-requirement', bucket: 'create', source: 'catalog', routes: ['post-collection'] },
    ],
  };
}

function routeOverrides(overrides) {
  const fixture = baseFixture();
  fixture.routes = [{ id: 'under-test', method: 'GET', path: '/requirements/{id}', mutates: false, statuses: [400, 401, 403, 404, 500, 503], response: 'resource' }, ...overrides];
  fixture.operations = [
    { legacy: 'create-requirement', bucket: 'create', source: 'catalog', routes: ['post-collection'] },
    { legacy: 'get-requirement', bucket: 'read', source: 'pr273', routes: ['under-test'] },
  ];
  fixture.accounting = { create: 1, read: 1 };
  fixture.review_size = 1;
  return fixture;
}

function errorsFor(overrides) {
  return lintRoutes(routeOverrides(overrides));
}

test('the unmodified base fixture lints clean', () => {
  assert.deepEqual(lintFixture(baseFixture()), []);
});

test('repository and scope path prefixes are rejected', () => {
  assert.ok(errorsFor([{ id: 'scoped', method: 'GET', path: '/{repository}/{scope}/requirements', mutates: false, statuses: [500] }]).some(e => /repository or scope path prefix/.test(e)));
  assert.ok(errorsFor([{ id: 'scoped', method: 'GET', path: '/repositories/requirements', mutates: false, statuses: [500] }]).some(e => /repository or scope path prefix/.test(e)));
});

test('query subroutes are rejected', () => {
  assert.ok(errorsFor([{ id: 'query-search', method: 'GET', path: '/query/search', mutates: false, statuses: [500] }]).some(e => /\/query subroutes/.test(e)));
});

test('relationship and edge routes are rejected', () => {
  assert.ok(errorsFor([{ id: 'edge', method: 'PATCH', path: '/requirements/{id}/refines', mutates: true, statuses: [400, 401, 403, 404, 409, 500, 503] }]).some(e => /relationship route segment 'refines'/.test(e)));
  assert.ok(errorsFor([{ id: 'edge', method: 'GET', path: '/rules/{id}/requirements', mutates: false, statuses: [500] }]).some(e => /'requirements' as a subresource/.test(e)));
});

test('legacy verb routes are rejected', () => {
  assert.ok(errorsFor([{ id: 'verb', method: 'POST', path: '/create-source', mutates: true, statuses: [400, 401, 403, 404, 409, 500, 503] }]).some(e => /verb-led route 'create-source' is rejected/.test(e)));
  assert.ok(errorsFor([{ id: 'verb', method: 'POST', path: '/create-requirement', mutates: true, statuses: [400, 401, 403, 404, 409, 500, 503] }]).some(e => /'create-requirement' repeats a legacy operation name/.test(e)));
  assert.ok(errorsFor([{ id: 'verb', method: 'POST', path: '/v9/operations/create-source', mutates: true, statuses: [400, 401, 403, 404, 409, 500, 503] }]).some(e => /versioned URL prefix/.test(e)));
  assert.ok(errorsFor([{ id: 'verb', method: 'POST', path: '/operations/set-requirement-refines', mutates: true, statuses: [400, 401, 403, 404, 409, 500, 503] }]).some(e => /legacy verb segment 'operations'/.test(e)));
  assert.ok(errorsFor([{ id: 'verb', method: 'POST', path: '/requirements/{id}/promote-thing', mutates: true, statuses: [400, 401, 403, 404, 409, 500, 503] }]).some(e => /subresource segment 'promote-thing' is not a declared child address or action/.test(e)));
});

test('GET bodies are rejected', () => {
  assert.ok(errorsFor([{ id: 'body', method: 'GET', path: '/requirements/{id}', mutates: false, statuses: [500], body: true }]).some(e => /GET bodies/.test(e)));
});

test('required-null and untyped request slots are rejected', () => {
  for (const request of [null, 'null', 'empty', 'anything-goes']) {
    assert.ok(errorsFor([{ id: 'null-body', method: 'GET', path: '/requirements', mutates: false, statuses: [500], request }]).some(e => /required-null or empty request slots/.test(e)), `request ${JSON.stringify(request)}`);
  }
});

test('raw array and flattened envelopes are rejected', () => {
  assert.ok(errorsFor([{ id: 'raw', method: 'GET', path: '/requirements', mutates: false, statuses: [500], response: 'array' }]).some(e => /response kind 'array'/.test(e)));
  assert.ok(errorsFor([{ id: 'flat', method: 'GET', path: '/requirements', mutates: false, statuses: [500], response: 'resource', envelope: 'flattened' }]).some(e => /flattened or non-standard envelopes/.test(e)));
});

test('MCP-only wrappers are rejected', () => {
  assert.ok(errorsFor([{ id: 'mcp', method: 'GET', path: '/requirements', mutates: false, statuses: [500], response: 'items', mcp_envelope: 'wrapped' }]).some(e => /MCP-only wrappers/.test(e)));
  assert.ok(errorsFor([{ id: 'mcp', method: 'GET', path: '/requirements', mutates: false, statuses: [500], response: 'items', mcp_only: true }]).some(e => /MCP-only wrappers/.test(e)));
});

test('payload identity repetition and empty tool descriptions are rejected', () => {
  const document = { paths: { '/requirements/{id}': { patch: {
    description: 'short', parameters: [],
    requestBody: { content: { 'application/json': { schema: { properties: { data: {
      properties: { id: { type: 'string' }, scope_id: { type: 'string' } },
    } } } } } },
  } } } };
  const errors = documentGrammarErrors(document, { tools: [{ name: 'update-requirement', description: 'Invoke the shared operation.' }] });
  assert.ok(errors.some(error => error.includes("path field 'id'")), errors.join('; '));
  assert.ok(errors.some(error => error.includes("connection field 'scope_id'")), errors.join('; '));
  assert.ok(errors.some(error => error.includes('tool-description-usefulness')), errors.join('; '));
});

test('an immutable child id does not repeat its Proposal parent id', () => {
  const document = { paths: { '/proposals/{id}/assertions': { post: {
    description: 'Add one immutable assertion to a Proposal.', parameters: [],
    requestBody: { content: { 'application/json': { schema: { properties: { data: {
      properties: { id: { type: 'string' } },
    } } } } } },
  } } } };
  assert.deepEqual(documentGrammarErrors(document), []);
});

test('undeclared actions and queries are rejected', () => {
  assert.ok(errorsFor([{ id: 'act', method: 'POST', path: '/requirements/{id}/promote', action: 'promote', mutates: true, statuses: [400, 401, 403, 404, 409, 500, 503] }]).some(e => /action 'promote' is not declared/.test(e)));
  assert.ok(errorsFor([{ id: 'q', method: 'GET', path: '/requirements', query: 'rank', mutates: false, statuses: [500], response: 'items' }]).some(e => /query 'rank' is not declared/.test(e)));
  assert.ok(errorsFor([{ id: 'q', method: 'POST', path: '/requirements', query: 'search', mutates: false, statuses: [500], response: 'items' }]).some(e => /queries are GETs/.test(e)));
});

test('mutating GETs and undeclared read POSTs are rejected', () => {
  assert.ok(errorsFor([{ id: 'mget', method: 'GET', path: '/requirements', mutates: true, statuses: [500] }]).some(e => /mutating GETs/.test(e)));
  assert.ok(errorsFor([{ id: 'rpost', method: 'POST', path: '/requirements', statuses: [500], response: 'result' }]).some(e => /declare mutates explicitly/.test(e)));
  assert.ok(errorsFor([{ id: 'ppatch', method: 'PATCH', path: '/requirements/{id}', mutates: false, statuses: [400, 401, 403, 404, 500, 503] }]).some(e => /PATCH is a write/.test(e)));
});

test('missing status declarations are rejected', () => {
  assert.ok(errorsFor([{ id: 'nostatus', method: 'GET', path: '/requirements/{id}', mutates: false }]).some(e => /statuses must be a non-empty array/.test(e)));
  assert.ok(errorsFor([{ id: 'nobase', method: 'GET', path: '/requirements/{id}', mutates: false, statuses: [400, 500] }]).some(e => /base status 401 is not declared/.test(e)));
  assert.ok(errorsFor([{ id: 'noconflict', method: 'POST', path: '/requirements', mutates: true, statuses: [400, 401, 403, 404, 500, 503], response: 'resource' }]).some(e => /must declare 409/.test(e)));
  assert.ok(errorsFor([{ id: 'unsorted', method: 'GET', path: '/requirements/{id}', mutates: false, statuses: [500, 400, 401, 403, 404, 503] }]).some(e => /sorted and unique/.test(e)));
});

test('a connection-scoped metadata read is exempt from the 404 base requirement', () => {
  const fixture = routeOverrides([]);
  fixture.routes[0] = { id: 'get-metadata', method: 'GET', path: '/metadata', mutates: false, statuses: [400, 401, 403, 500, 503], response: 'result' };
  assert.deepEqual(lintRoutes(fixture), []);
  assert.ok(errorsFor([{ id: 'no404', method: 'GET', path: '/requirements/{id}', mutates: false, statuses: [400, 401, 403, 500, 503], response: 'resource' }]).some(e => /base status 404 is not declared/.test(e)));
});

test('a single-message read never ships the items envelope', () => {
  const fixture = routeOverrides([
    { id: 'single', method: 'GET', path: '/requirements/{id}/discussions/{discussion_id}/messages/{message_id}', mutates: false, statuses: [400, 401, 403, 404, 500, 503], response: 'items' },
  ]);
  fixture.variables.push('discussion_id', 'message_id');
  const errors = lintRoutes(fixture);
  assert.ok(errors.some(e => /single-message read must not ship the items envelope/.test(e)), errors.join('; '));
});

test('name collisions and duplicate bindings are rejected', () => {
  const collided = routeOverrides([
    { id: 'first', method: 'GET', path: '/requirements/{id}', mutates: false, statuses: [500] },
    { id: 'second', method: 'GET', path: '/requirements/{id}', mutates: false, statuses: [500] },
  ]);
  const errors = lintRoutes(collided);
  assert.ok(errors.some(e => /generated name '.*' collides/.test(e)), errors.join('; '));
  const duplicated = baseFixture();
  duplicated.routes.push({ id: 'again', method: 'POST', path: '/{collection}', mutates: true, statuses: [400, 401, 403, 404, 409, 500, 503], response: 'resource' });
  assert.ok(lintRoutes(duplicated).some(e => /duplicate binding/.test(e)));
});

test('unresolved path parameters are rejected', () => {
  assert.ok(errorsFor([{ id: 'loose', method: 'GET', path: '/requirements/{bogus_id}', mutates: false, statuses: [500] }]).some(e => /\{bogus_id\} is not declared/.test(e)));
});

test('undeclared root segments are rejected', () => {
  assert.ok(errorsFor([{ id: 'stray', method: 'GET', path: '/gadgets', mutates: false, statuses: [500], response: 'items' }]).some(e => /root segment 'gadgets' is not a declared collection/.test(e)));
});

test('coverage failures: double-mapped, unmapped, internal routes, counts', () => {
  const fixture = baseFixture();
  fixture.operations.push({ legacy: 'create-requirement', bucket: 'create', source: 'catalog', routes: ['post-collection'] });
  fixture.accounting = { create: 2 };
  let errors = lintFixture(fixture);
  assert.ok(errors.some(e => /double-mapped/.test(e)), errors.join('; '));
  assert.ok(errors.some(e => /2 operations mapped, expected 1/.test(e)));

  const unmapped = baseFixture();
  unmapped.operations.push({ legacy: 'orphan-op', bucket: 'create', source: 'pr273', routes: [] });
  unmapped.review_size = 1;
  errors = lintFixture(unmapped);
  assert.ok(errors.some(e => /orphan-op: unmapped/.test(e)), errors.join('; '));

  const internal = baseFixture();
  internal.operations[0].internal = true;
  errors = lintFixture(internal);
  assert.ok(errors.some(e => /internal operations declare no routes/.test(e)), errors.join('; '));

  const wrongBucket = baseFixture();
  wrongBucket.accounting = { create: 5 };
  assert.ok(lintFixture(wrongBucket).some(e => /bucket 'create' has 1 operations, expected 5/.test(e)));
});

test('catalog drift fails in both directions', () => {
  const fixture = baseFixture();
  assert.deepEqual(lintFixture(fixture, { catalogNames: ['create-requirement'] }), []);
  const missing = lintFixture(fixture, { catalogNames: ['create-requirement', 'create-rule'] });
  assert.ok(missing.some(e => /'create-rule' is unmapped/.test(e)), missing.join('; '));
  const stale = lintFixture(fixture, { catalogNames: [] });
  assert.ok(stale.some(e => /'create-requirement' is not in the live catalog/.test(e)), stale.join('; '));
});
