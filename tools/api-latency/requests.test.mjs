import assert from 'node:assert/strict';
import { test } from 'node:test';
import { Planner, collectionFor, describe, itemValue, pendingValue } from './requests.mjs';

const op = (method, path, extra = {}) => ({ label: `${method.toUpperCase()} ${path}`, method, path, selector: null, parameters: [], write: method !== 'get', ...extra });

const operations = [
  op('get', '/requirements'),
  op('post', '/requirements'),
  op('get', '/requirements/{id}'),
  op('get', '/requirements/{id}', {
    selector: 'trace',
    label: 'GET /requirements/{id} query=trace',
    parameters: [{ name: 'query', location: 'query', required: true, schema: { const: 'trace' } }],
  }),
  op('get', '/requirements/{id}/discussions'),
  op('post', '/requirements/{id}/discussions'),
  op('get', '/requirements/{id}/discussions/{discussion_id}'),
  op('post', '/requirements/{id}/discussions/{discussion_id}/messages'),
  op('get', '/discussion-containers'),
  op('get', '/messages'),
];

test('a list item gives the value of a parameter, also from a nested object', () => {
  assert.equal(itemValue({ id: 'req_a' }, 'id'), 'req_a');
  assert.equal(itemValue({ kind: 'addressed', discussion: { discussion_id: 'd1', parent: { node_id: 'req_a' } } }, 'discussion_id'), 'd1');
  assert.equal(itemValue({ id: 'msg_1' }, 'message_id'), 'msg_1');
  assert.equal(itemValue(undefined, 'id'), null);
});

test('a parameter names the top-level collection of its records', () => {
  const collections = ['/requirements', '/discussion-containers', '/messages'];
  assert.equal(collectionFor('container_id', collections), '/discussion-containers');
  assert.equal(collectionFor('message_id', collections), '/messages');
  assert.equal(collectionFor('entry_id', collections), null);
});

test('the pending submission of a record gives its proposal id', () => {
  assert.equal(pendingValue({ decision: { pending: { proposal_id: 'p1' } } }, 'proposal_id'), 'p1');
  assert.equal(pendingValue({ review: { pending_proposal_id: 'p2' } }, 'proposal_id'), 'p2');
  assert.equal(pendingValue({ decision: { pending: null } }, 'proposal_id'), undefined);
});

test('a failure answer becomes a short reason', () => {
  assert.equal(describe({ kind: 'invalid_input', reason: 'invalid_value', field: 'request.actor' }), 'invalid_input invalid_value request.actor');
  assert.equal(describe(null), 'no answer');
  assert.equal(describe({ other: 1 }), '{"other":1}');
});

test('the read after a write is the record that the write created or changed', () => {
  const planner = new Planner({ operations, context: {} });
  const read = (request, data = {}) => planner.followRead({ query: {}, headers: {}, ...request }, { data });
  assert.equal(read({ method: 'post', path: '/requirements', body: { id: 'req_new' } }).path, '/requirements/req_new');
  assert.equal(read({ method: 'post', path: '/requirements/req_a/discussions', body: {} }).path, '/requirements/req_a');
  assert.equal(read({ method: 'post', path: '/requirements/req_a/discussions/d1/messages', body: {} }).path, '/requirements/req_a/discussions/d1');
  assert.equal(read({ method: 'post', path: '/statement-checks', body: {} }), null);
});

test('a read finds path values through the list above each parameter', async () => {
  const planner = new Planner({ operations, context: { query: {} } });
  const answers = {
    '/requirements': { items: [{ id: 'req_a' }, { id: 'req_b' }] },
    '/requirements/req_a/discussions': { items: [] },
    '/requirements/req_b/discussions': { items: [{ discussion: { discussion_id: 'd1' } }] },
    '/requirements/req_b/discussions/d1': {},
  };
  const call = async request => (request.path in answers ? { ok: true, data: answers[request.path] } : { ok: false, error: { kind: 'resource_not_found' } });
  const request = await planner.read(call, operations[6]);
  assert.equal(request.path, '/requirements/req_b/discussions/d1');
  const trace = await planner.read(async r => ({ ok: true, data: r.path === '/requirements' ? answers['/requirements'] : {} }), operations[3]);
  assert.deepEqual(trace, { method: 'get', path: '/requirements/req_a', query: { query: 'trace' } });
});
