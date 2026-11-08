import assert from 'node:assert/strict';
import { test } from 'node:test';
import { fillPath, helpCommands, mcpToolActions, operationsFromCatalog, pathParameters, possibleValues } from './catalog.mjs';

const selector = value => ({ name: 'query', location: 'query', required: true, schema: { const: value, type: 'string' } });

const catalog = {
  bounds: { matching: 3, returned: 3, total: 3, truncated: false },
  routes: [
    { method: 'get', path: '/rules', request_schema: null, queries: [{ parameters: [] }, { parameters: [selector('search')] }, { parameters: [selector('stale')] }] },
    { method: 'patch', path: '/rules/{id}', request_schema: { type: 'object' }, queries: [{ parameters: [{ name: 'id', location: 'path', required: true }] }] },
    { method: 'post', path: '/statement-checks', request_schema: { type: 'object' }, queries: [{ parameters: [] }] },
  ],
};

test('each query variant of each route is one operation', () => {
  const operations = operationsFromCatalog(catalog, { readOnlyPosts: ['/statement-checks'] });
  assert.deepEqual(operations.map(op => [op.label, op.write]), [
    ['GET /rules', false],
    ['GET /rules query=search', false],
    ['GET /rules query=stale', false],
    ['PATCH /rules/{id}', true],
    ['POST /statement-checks', false],
  ]);
  assert.equal(operations[3].parameters[0].name, 'id');
  assert.deepEqual(operations[3].requestSchema, { type: 'object' });
});

test('a truncated catalog listing is refused', () => {
  assert.throws(
    () => operationsFromCatalog({ ...catalog, bounds: { returned: 200, total: 201, truncated: true } }),
    /truncated at 200 of 201 routes/,
  );
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
