import assert from 'node:assert/strict';
import { test } from 'node:test';
import { readFileSync } from 'node:fs';
import { requestBody } from './body.mjs';
import { catalogWithSchemas, fillPath, helpCommands, mcpToolActions, operationsFromCatalog, pathParameters, possibleValues, routeFilter } from './catalog.mjs';

// Answers of a provenance binary, pinned by fixtures/capture.mjs: a listing of three
// routes without schemas, and the answer to the filter of each route's method and path.
const { listing, answers } = JSON.parse(readFileSync(new URL('fixtures/catalog.json', import.meta.url), 'utf8'));
const readRoute = async filter => answers[filter];
const READ_ONLY_POSTS = ['/statement-checks'];

test('each query form of each route is one operation, with its write body', async () => {
  const operations = operationsFromCatalog(await catalogWithSchemas(listing, readRoute), { readOnlyPosts: READ_ONLY_POSTS });
  assert.deepEqual(operations.map(op => [op.label, op.write]), [
    ['GET /sources', false],
    ['GET /sources query=search', false],
    ['POST /sources', true],
    ['POST /statement-checks', false],
  ]);
  const search = operations[1];
  assert.equal(search.parameters.find(item => item.name === 'query').schema.const, 'search');
  for (const op of operations.filter(item => item.method === 'post')) {
    assert.equal(op.requestSchema.type, 'object', op.label);
  }
  const body = requestBody(operations[2].requestSchema, null, { text: 'text', idText: 'source_a', fields: { id: 'source_a' } });
  assert.equal(body.id, 'source_a');
  assert.ok(Object.keys(body).length > 1, JSON.stringify(body));
});

test('a listing without schemas is refused, so no query form or write body is lost', () => {
  assert.equal(listing.routes[0].queries[1].parameters[0].schema, undefined);
  assert.throws(() => operationsFromCatalog(listing), /the catalog gives no schemas for GET \/sources/);
});

test('each route is read with the filter of its method and path', async () => {
  const filters = [];
  await catalogWithSchemas(listing, async filter => (filters.push(filter), answers[filter]));
  assert.deepEqual(filters.sort(), ['GET /sources', 'POST /sources', 'POST /statement-checks']);
  assert.equal(routeFilter({ method: 'patch', path: '/rules/{id}' }), 'PATCH /rules/{id}');
});

test('an answer with more routes for the filter gives the route of that method and path', async () => {
  // A binary whose filter matches text gives more routes for "GET /sources".
  const wide = { ...answers['GET /sources'], routes: [answers['POST /sources'].routes[0], answers['GET /sources'].routes[0]] };
  const one = { ...listing, routes: [listing.routes[0]] };
  const catalog = await catalogWithSchemas(one, async () => wide);
  assert.equal(catalog.routes[0], answers['GET /sources'].routes[0]);
  await assert.rejects(catalogWithSchemas(one, async () => answers['POST /sources']), /gives no route GET \/sources/);
});

test('a truncated catalog listing is refused', async () => {
  const truncated = { ...listing, bounds: { returned: 200, total: 201, truncated: true } };
  assert.throws(() => operationsFromCatalog(truncated), /truncated at 200 of 201 routes/);
  await assert.rejects(catalogWithSchemas(truncated, readRoute), /truncated at 200 of 201 routes/);
});

test('path parameters are read and filled in order', () => {
  const path = '/rules/{id}/discussions/{discussion_id}';
  assert.deepEqual(pathParameters(path), ['id', 'discussion_id']);
  assert.equal(fillPath(path, { id: 'rule_a', discussion_id: 'a/b' }), '/rules/rule_a/discussions/a%2Fb');
  assert.throws(() => fillPath(path, { id: 'rule_a' }), /no value for path parameter discussion_id/);
});

const help = `Usage: provenance [OPTIONS] <COMMAND>

Commands:
  search           Find records.
  check            Check graph validity.
  help             Print this message or the help of the given subcommand(s)

Options:
      --quiet    Drop the advisory notes
`;

test('help text gives the subcommands without help', () => {
  assert.deepEqual(helpCommands(help), ['search', 'check']);
  assert.deepEqual(helpCommands('Usage: provenance prime [OPTIONS]\n\nOptions:\n  -h, --help\n'), []);
});

test('help text gives the possible values of one argument', () => {
  const text = [
    '  [ACTION]  [possible values: get, trace, create]',
    '      --view <VIEW>',
    '          [possible values: record, children]',
    '      --view <VIEW>  [possible values: record, children]',
  ].join('\n');
  assert.deepEqual(possibleValues(text, '[ACTION]'), ['get', 'trace', 'create']);
  assert.deepEqual(possibleValues(text, '--view'), ['record', 'children']);
  assert.deepEqual(possibleValues(text, '--missing'), []);
  assert.deepEqual(possibleValues('      --depth <DEPTH>\n      --kind <KIND>\n          [possible values: a, b]', '--depth'), []);
});

test('a tool with an action enum gives one call for each action', () => {
  const tools = [
    { name: 'get', annotations: { readOnlyHint: true }, inputSchema: { properties: { target: {} } } },
    { name: 'change', annotations: { readOnlyHint: false }, inputSchema: { properties: { action: { enum: ['create', 'claim'] } } } },
  ];
  assert.deepEqual(mcpToolActions(tools), [
    { label: 'tool get', tool: 'get', action: null, readOnly: true },
    { label: 'tool change action=create', tool: 'change', action: 'create', readOnly: false },
    { label: 'tool change action=claim', tool: 'change', action: 'claim', readOnly: false },
  ]);
});
