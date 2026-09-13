import assert from 'node:assert/strict';

export async function checkRecords({ HttpClient, OperationError, PROTOCOL_VERSION }, fixture) {
  const client = await HttpClient.connectWithBearer(fixture.url, fixture.bearer);
  const context = { repository: fixture.targets.first, scope: 'default' };
  await checkCursorReads(client, context);
  const raw = async (operation, call) => {
    const response = await fetch(`${fixture.url}/v${PROTOCOL_VERSION}/operations/${operation}`, {
      method: 'POST', headers: { 'content-type': 'application/json', authorization: `Bearer ${fixture.bearer}` }, body: JSON.stringify(call),
    });
    return { status: response.status, value: await response.json() };
  };
  const compare = async (operation, call, invoke) => {
    const expected = await raw(operation, call);
    assert.equal(expected.status, 200);
    const actual = await invoke(call);
    assert.deepEqual(actual, expected.value, `${operation} retains every field and full stamp`);
    return actual;
  };
  const info = await compare('info', { context: { repository: context.repository }, request: {} }, call => client.info(call));
  assert.equal(info.repository, context.repository);
  assert.equal(info.protocol_version, PROTOCOL_VERSION);
  assert.equal(typeof info.state_schema_version, 'number');
  assert.equal(Object.keys(fixture.nodes).length, 8);
  for (const [node_type, id] of Object.entries(fixture.nodes)) {
    const result = await compare('get', { context, request: { node_type, id } }, call => client.get(call));
    assert.equal(result.found, true);
    assert.equal(result.node.node_type, node_type);
    assert.equal(result.node.id, id);
    assert.equal(typeof result.stamp.serial, 'number');
    assert.equal(typeof result.stamp.instance_id, 'string');
  }
  const selected = (repository, scope) => ({ context: { repository, scope }, request: { node_type: 'rule', id: fixture.shared_rule } });
  assert.equal((await client.get(selected(fixture.targets.first, 'default'))).node.statement, fixture.expected.first);
  assert.equal((await client.get(selected(fixture.targets.second, 'default'))).node.statement, fixture.expected.second);
  assert.equal((await client.get(selected(fixture.targets.first, 'other'))).node.statement, fixture.expected.other);
  const missing = await client.get({ context, request: { node_type: 'rule', id: 'rule_missing' } });
  assert.equal(missing.found, false);
  assert.equal(Object.hasOwn(missing, 'node'), false);
  const nullable = await client.get({ ...selected(context.repository, context.scope), context: { ...context, freshness: null } });
  assert.equal(nullable.node.statement, fixture.expected.first);
  const search = await compare('search', { context, request: { text: 'shared', limit: 1 } }, call => client.search(call));
  assert.equal(search.limit, 1);
  assert.equal(search.has_more, true);
  assert.equal(search.nodes.length, 1);
  assert.equal((await client.search({ context, request: { text: 'shared' } })).limit, 50);
  const neighbors = await compare('neighbors', { context, request: { node_type: 'requirement', id: fixture.nodes.requirement, limit: 1 } }, call => client.neighbors(call));
  assert.equal(neighbors.has_more, true);
  assert.equal(neighbors.neighbors.length, 1);
  const trace = await compare('trace', { context, request: { node_type: 'requirement', id: fixture.nodes.requirement, limit: 1 } }, call => client.trace(call));
  assert.equal(trace.has_more, true);
  assert.equal(trace.nodes.length, 1);
  const refuses = async (operation, call, invoke, status, kind) => {
    const expected = await raw(operation, call);
    assert.equal(expected.status, status);
    await assert.rejects(invoke(call), error => {
      assert.ok(error instanceof OperationError);
      assert.equal(error.status, status);
      assert.equal(error.failure.error.kind, kind);
      assert.deepEqual(error.failure, expected.value);
      return true;
    });
    return expected.value;
  };
  for (const limit of [0, 201]) await refuses('search', { context, request: { text: 'shared', limit } }, call => client.search(call), 400, 'invalid_input');
  await refuses('get', selected('unknown', 'default'), call => client.get(call), 404, 'unknown_target');
  await refuses('get', selected(fixture.denied_target, 'default'), call => client.get(call), 403, 'access_denied');
  await refuses('get', selected(context.repository, 'missing'), call => client.get(call), 404, 'unknown_scope');
  const staleCall = { ...selected(fixture.stale_target, 'default'), context: { repository: fixture.stale_target, scope: 'default', freshness: 'refuse_stale' } };
  const stale = await refuses('get', staleCall, call => client.get(call), 409, 'stale');
  assert.equal(typeof stale.error.serial, 'number');
  assert.equal(typeof stale.error.digest, 'string');
  assert.equal(typeof stale.error.instance_id, 'string');
  assert.ok(stale.error.moved.length > 0);
  await refuses('get', { ...selected(fixture.unmaterialized_target, 'default'), context: { repository: fixture.unmaterialized_target, scope: 'default', freshness: 'annotate_only' } }, call => client.get(call), 409, 'no_projection');
}

export async function checkCursorReads(client, context) {
  const seen = new Set();
  let cursor = null;
  do {
    const page = await client.readDocument({ context, request: { id: 'req_shared', limit: 2, cursor } });
    assert.equal(page.operation, 'read-document');
    assert.ok(page.entries.length <= 2);
    for (const entry of page.entries) {
      const record = entry.node ?? entry.thread ?? entry.message;
      const key = `${entry.kind}:${record.node_type ?? ''}:${record.id}`;
      assert.equal(seen.has(key), false, 'pages do not repeat identities');
      seen.add(key);
    }
    cursor = page.next_cursor;
    assert.equal(page.has_more, cursor !== null);
  } while (cursor !== null);
  assert.ok(seen.has('member:requirement:req_shared'));
  const first = await client.search({ context, request: { text: 'shared', limit: 1 } });
  const next = await client.search({ context, request: { text: 'shared', limit: 1, cursor: first.next_cursor } });
  assert.notEqual(first.nodes[0].id, next.nodes[0].id);
  await assert.rejects(client.search({ context, request: { text: 'other', limit: 1, cursor: first.next_cursor } }),
    error => error.failure.error.kind === 'cursor_invalid');
}
