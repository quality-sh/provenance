import assert from 'node:assert/strict';
import { test } from 'node:test';
import { cliWriteArguments, commandVariants, fillArguments, mcpWriteArguments, orderLeaves, verbOperation } from './commands.mjs';

const op = (method, path) => ({ label: `${method.toUpperCase()} ${path}`, method, path, selector: null, write: true });

const operations = [
  op('post', '/topics'),
  op('post', '/requirements'),
  op('patch', '/requirements/{id}'),
  op('post', '/topics/{id}/claim'),
  op('post', '/questions/{id}/claim'),
  op('post', '/rules/{id}/discussions'),
  op('post', '/requirements/{id}/discussions'),
  op('post', '/requirements/{id}/discussions/{discussion_id}/messages'),
];

test('each write verb finds its route, with the preferred collection first', () => {
  const label = verb => verbOperation(verb, operations)?.label ?? null;
  assert.equal(label('create'), 'POST /requirements');
  assert.equal(label('update'), 'PATCH /requirements/{id}');
  assert.equal(label('claim'), 'POST /topics/{id}/claim');
  assert.equal(label('discuss'), 'POST /requirements/{id}/discussions');
  assert.equal(label('reply'), 'POST /requirements/{id}/discussions/{discussion_id}/messages');
  assert.equal(label('get'), null);
  assert.equal(label('discussions'), null);
});

test('a planned create becomes CLI flags and MCP change arguments', () => {
  const request = { method: 'post', path: '/requirements', headers: {}, body: { id: 'req_new', statement: 'The system keeps a record.', depends_on: [], description: null } };
  assert.deepEqual(cliWriteArguments('create', operations[1], request), [
    'req_new', 'create', '--format', 'json', '--type', 'requirement',
    '--statement', 'The system keeps a record.', '--depends-on-json', '[]',
  ]);
  assert.deepEqual(mcpWriteArguments('create', operations[1], request), {
    action: 'create', target: 'req_new', type: 'requirement', statement: 'The system keeps a record.', depends_on: [], description: null,
  });
});

test('an update carries its guard and a reply targets the discussion', () => {
  const update = { method: 'patch', path: '/requirements/req_a', headers: { 'If-Match': 'sha256:1' }, body: { description: 'New.' } };
  assert.deepEqual(cliWriteArguments('update', operations[2], update), ['req_a', 'update', '--format', 'json', '--if-match', 'sha256:1', '--description', 'New.']);
  assert.equal(mcpWriteArguments('update', operations[2], update).if_match, 'sha256:1');
  const reply = { method: 'post', path: '/requirements/req_a/discussions/d1/messages', headers: { 'If-Match': '2' }, body: { body: 'Text.' } };
  assert.deepEqual(cliWriteArguments('reply', operations[7], reply), ['d1', 'reply', '--format', 'json', '--expected-version', '2', '--body', 'Text.']);
  assert.equal(mcpWriteArguments('reply', operations[7], reply).expected_version, 2);
});

test('command arguments take their values from the context', () => {
  assert.deepEqual(fillArguments(['export', '--output', '{scratch}/export.json', '{missing}'], { scratch: '/tmp/a' }), ['export', '--output', '/tmp/a/export.json', '{missing}']);
});

test('a command that reads the file of another command runs after it', () => {
  assert.deepEqual(
    orderLeaves(['import', 'search', 'export', 'report render', 'report build', 'prime']),
    ['search', 'export', 'import', 'report build', 'report render', 'prime'],
  );
  assert.deepEqual(orderLeaves(['report render', 'prime']), ['prime', 'report render']);
});

test('a command that names {artifact} runs once for each artifact kind', () => {
  assert.deepEqual(commandVariants([['validate', '{artifact}', '--input', '{scratch}/{artifact}.json']], ['contribution', 'proposal']), [
    ['validate', 'contribution', '--input', '{scratch}/contribution.json'],
    ['validate', 'proposal', '--input', '{scratch}/proposal.json'],
  ]);
  assert.deepEqual(commandVariants([['prime']], ['contribution']), [['prime']]);
});
