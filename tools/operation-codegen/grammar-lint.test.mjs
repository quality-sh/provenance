import test from 'node:test';
import assert from 'node:assert/strict';
import { documentGrammarErrors, routeGrammarErrors } from './grammar-lint.mjs';

const successEnvelope = data => ({
  type: 'object',
  additionalProperties: false,
  required: ['data', 'meta'],
  properties: { data: data ?? { type: 'object' }, meta: { type: 'object' } },
});
const failureEnvelope = {
  type: 'object',
  additionalProperties: false,
  required: ['error', 'meta'],
  properties: { error: { type: 'object' }, meta: { type: 'object' } },
};
const requestEnvelope = data => ({
  type: 'object',
  additionalProperties: false,
  required: ['data'],
  properties: { data: data ?? { type: 'object', properties: {} } },
});

function operation(path, method, overrides = {}) {
  const mutates = overrides['x-operation-mutates'] ?? method !== 'get';
  const statuses = overrides.statuses
    ?? [400, 401, 403, 404, ...(mutates ? [409] : []), 500, 503];
  const parameters = path.match(/\{([a-z0-9_]+)\}/g)?.map(part => ({
    name: part.slice(1, -1), in: 'path', required: true, schema: { type: 'string' },
  })) ?? [];
  const responses = Object.fromEntries([
    ['200', { content: { 'application/json': { schema: successEnvelope(overrides.dataSchema) } } }],
    ...statuses.map(status => [String(status), {
      content: { 'application/json': { schema: failureEnvelope } },
    }]),
  ]);
  const value = {
    operationId: overrides.operationId ?? `${method}${path.replace(/[^a-z0-9]+/gi, '_')}`,
    description: 'Perform one specific resource operation in the bound scope.',
    'x-operation-mutates': mutates,
    parameters,
    responses,
  };
  if (method !== 'get') {
    value.requestBody = {
      required: true,
      content: { 'application/json': { schema: requestEnvelope(overrides.requestData) } },
    };
  }
  return Object.assign(value, overrides.operation ?? {});
}

function document(routes) {
  const paths = {};
  for (const route of routes) {
    paths[route.path] ??= {};
    paths[route.path][route.method] = operation(route.path, route.method, route);
  }
  return { paths, components: { schemas: {} } };
}

function errorsFor(path, method = 'get', overrides = {}) {
  return routeGrammarErrors(document([{ path, method, ...overrides }]));
}

test('the live resource document lints clean', () => {
  assert.deepEqual(routeGrammarErrors(document([
    { path: '/requirements', method: 'post' },
    { path: '/requirements/{id}', method: 'get' },
    { path: '/requirements/{id}', method: 'patch' },
    { path: '/requirements/{id}/submit', method: 'post' },
  ])), []);
});

test('repository and scope path prefixes are rejected', () => {
  assert.ok(errorsFor('/{repository}/{scope}/requirements').some(error => /repository or scope path prefix/.test(error)));
  assert.ok(errorsFor('/repositories/requirements').some(error => /repository or scope path prefix/.test(error)));
});

test('query subroutes are rejected', () => {
  assert.ok(errorsFor('/query/search').some(error => /\/query subroutes/.test(error)));
});

test('relationship and edge routes are rejected', () => {
  assert.ok(errorsFor('/requirements/{id}/refines', 'patch').some(error => /relationship route segment 'refines'/.test(error)));
  assert.ok(errorsFor('/rules/{id}/requirements').some(error => /'requirements' as a subresource/.test(error)));
});

test('legacy verb routes are rejected', () => {
  assert.ok(errorsFor('/create-source', 'post').some(error => /verb-led route 'create-source'/.test(error)));
  assert.ok(errorsFor('/v9/operations/create-source', 'post').some(error => /versioned URL prefix/.test(error)));
  assert.ok(errorsFor('/operations/set-requirement-refines', 'post').some(error => /legacy verb segment 'operations'/.test(error)));
  assert.ok(errorsFor('/requirements/{id}/promote-thing', 'post').some(error => /not a declared child address or action/.test(error)));
});

test('GET bodies are rejected', () => {
  const doc = document([{ path: '/requirements/{id}', method: 'get' }]);
  doc.paths['/requirements/{id}'].get.requestBody = {
    required: true,
    content: { 'application/json': { schema: requestEnvelope() } },
  };
  assert.ok(routeGrammarErrors(doc).some(error => /GET bodies/.test(error)));
});

test('required-null and untyped request slots are rejected', () => {
  const doc = document([{ path: '/requirements', method: 'post' }]);
  doc.paths['/requirements'].post.requestBody = {
    required: false,
    content: { 'application/json': { schema: null } },
  };
  assert.ok(routeGrammarErrors(doc).some(error => /required typed \{data\} envelope/.test(error)));
});

test('raw, flattened, and MCP-only response envelopes are rejected', () => {
  const raw = document([{ path: '/requirements', method: 'get' }]);
  raw.paths['/requirements'].get.responses['200'].content['application/json'].schema = {
    type: 'array', items: { type: 'object' },
  };
  assert.ok(routeGrammarErrors(raw).some(error => /flattened or non-standard envelope/.test(error)));

  const mcp = { tools: [{
    name: 'list-requirements',
    description: 'List Requirements in the bound scope.',
    outputSchema: { type: 'object', required: ['result'], properties: { result: { type: 'array' } } },
  }] };
  assert.ok(documentGrammarErrors(document([]), mcp).some(error => /MCP tool.*non-standard envelope/.test(error)));
});

test('payload identity repetition and empty tool descriptions are rejected', () => {
  const doc = document([{
    path: '/requirements/{id}',
    method: 'patch',
    requestData: { properties: { id: { type: 'string' }, scope_id: { type: 'string' } } },
  }]);
  const errors = documentGrammarErrors(doc, { tools: [{
    name: 'update-requirement',
    description: 'Invoke the shared operation.',
    outputSchema: successEnvelope(),
  }] });
  assert.ok(errors.some(error => error.includes("path field 'id'")), errors.join('; '));
  assert.ok(errors.some(error => error.includes("connection field 'scope_id'")), errors.join('; '));
  assert.ok(errors.some(error => error.includes('tool-description-usefulness')), errors.join('; '));
});

test('an immutable child id does not repeat its Proposal parent id', () => {
  const doc = document([{
    path: '/proposals/{id}/assertions',
    method: 'post',
    requestData: { properties: { id: { type: 'string' } } },
  }]);
  assert.deepEqual(documentGrammarErrors(doc), []);
});

test('undeclared actions and queries are rejected', () => {
  assert.ok(errorsFor('/requirements/{id}/promote', 'post').some(error => /not a declared child address or action/.test(error)));
  const doc = document([{ path: '/requirements', method: 'get' }]);
  doc.paths['/requirements'].get.parameters.push({
    name: 'query', in: 'query', schema: { type: 'string', enum: ['rank'] },
  });
  assert.ok(routeGrammarErrors(doc).some(error => /query 'rank' is not declared/.test(error)));
});

test('mutating GETs, implicit POSTs, and read PATCHes are rejected', () => {
  assert.ok(errorsFor('/requirements', 'get', { operation: { 'x-operation-mutates': true } }).some(error => /mutating GETs/.test(error)));
  assert.ok(errorsFor('/requirements', 'post', { operation: { 'x-operation-mutates': undefined } }).some(error => /must be declared explicitly/.test(error)));
  assert.ok(errorsFor('/requirements/{id}', 'patch', { operation: { 'x-operation-mutates': false } }).some(error => /PATCH is a write/.test(error)));
});

test('missing failure statuses are rejected', () => {
  assert.ok(errorsFor('/requirements/{id}', 'get', { statuses: [400, 500] }).some(error => /base status 401 is not declared/.test(error)));
  assert.ok(errorsFor('/requirements', 'post', { statuses: [400, 401, 403, 404, 500, 503] }).some(error => /must declare 409/.test(error)));
});

test('declared statuses cover every live failure variant status', () => {
  const doc = document([{ path: '/requirements/{id}', method: 'get' }]);
  doc.paths['/requirements/{id}'].get.responses['400'].content['application/json'].schema = {
    properties: { error: { $ref: '#/components/schemas/ReadFailure' } },
    required: ['error', 'meta'],
  };
  doc.components.schemas.ReadFailure = {
    oneOf: [
      { properties: { kind: { const: 'resource_not_found' } } },
      { properties: { kind: { const: 'file_unavailable' } } },
    ],
  };
  delete doc.paths['/requirements/{id}'].get.responses['503'];
  assert.ok(documentGrammarErrors(doc).some(error => /file_unavailable.*503|503.*file_unavailable/.test(error)));
});

test('a single-message read never ships the items envelope', () => {
  const errors = errorsFor(
    '/requirements/{id}/discussions/{discussion_id}/messages/{message_id}',
    'get',
    { dataSchema: { type: 'object', required: ['items'], properties: { items: { type: 'array' } } } },
  );
  assert.ok(errors.some(error => /single-message read must not ship the items envelope/.test(error)));
});

test('operation name collisions and duplicate bindings are rejected', () => {
  const collided = document([
    { path: '/requirements', method: 'get', operationId: 'sameName' },
    { path: '/rules', method: 'get', operationId: 'sameName' },
  ]);
  assert.ok(routeGrammarErrors(collided).some(error => /operationId 'sameName' collides/.test(error)));

  const duplicated = document([
    { path: '/requirements/{id}', method: 'get' },
    { path: '/requirements/{requirement_id}', method: 'get' },
  ]);
  assert.ok(routeGrammarErrors(duplicated).some(error => /duplicate binding/.test(error)));
});

test('unresolved path parameters are rejected', () => {
  const doc = document([{ path: '/requirements/{id}', method: 'get' }]);
  doc.paths['/requirements/{id}'].get.parameters = [];
  assert.ok(routeGrammarErrors(doc).some(error => /\{id\} is not declared/.test(error)));
});

test('undeclared root segments and methods are rejected', () => {
  assert.ok(errorsFor('/gadgets').some(error => /root segment 'gadgets' is not a declared collection/.test(error)));
  const doc = { paths: { '/requirements/{id}': { delete: {} } } };
  assert.ok(routeGrammarErrors(doc).some(error => /method is not GET, POST, or PATCH/.test(error)));
});
