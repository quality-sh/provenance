import assert from 'node:assert/strict';

export async function checkRecords({ HttpClient, OperationError }, fixture) {
  const identity = { repository: fixture.targets.first, scope: 'default' };
  const client = await HttpClient.connectWithBearer(fixture.url, fixture.bearer, undefined, identity);
  const metadata = await fetch(`${fixture.url}/metadata`, {
    headers: { authorization: `Bearer ${fixture.bearer}` },
  }).then(response => response.json());
  assert.deepEqual(
    { repository: metadata.data.repository, scope: metadata.data.scope },
    identity,
  );

  const methods = {
    domain: 'getDomain', boundary: 'getBoundary', requirement: 'getRequirement', rule: 'getRule',
    source: 'getSource', resolution: 'getResolution', topic: 'getTopic', question: 'getQuestion',
  };
  assert.equal(Object.keys(fixture.nodes).length, 8);
  for (const [nodeType, id] of Object.entries(fixture.nodes)) {
    const result = await client[methods[nodeType]]({ id });
    assert.equal(result.data.id, id);
    assert.equal(result.data.scope_id, 'default');
    if (nodeType === 'requirement') {
      assert.deepEqual(result.meta, {});
    } else {
      assert.ok(result.meta.stamp);
      assert.equal(result.meta.freshness_error, null);
    }
  }

  const search = await client.listRules({ query: 'search', text: 'shared', limit: 1 });
  assert.equal(search.meta.limit, 1);
  assert.equal(search.meta.has_more, false);
  assert.equal(search.data.items.length, 1);

  const neighbors = await client.getRequirement({
    id: fixture.nodes.requirement, query: 'neighbors', limit: 1,
  });
  assert.equal(neighbors.meta.has_more, true);
  assert.equal(neighbors.data.neighbors.length, 1);

  const trace = await client.getRequirement({
    id: fixture.nodes.requirement, query: 'trace', limit: 1,
  });
  assert.equal(trace.meta.has_more, true);
  assert.equal(trace.data.nodes.length, 1);

  for (const query of ['neighbors', 'trace']) {
    const filtered = await client.getRequirement({
      id: fixture.nodes.requirement, query, relations: ['domain_id'], limit: 20,
    });
    const items = query === 'neighbors' ? filtered.data.neighbors : filtered.data.nodes;
    assert.ok(items.length > 0);
    assert.ok(query === 'neighbors'
      ? items.every(item => item.relation === 'domain_id')
      : items.every(item => item.node.id === fixture.nodes.domain));
  }
  await checkCursorReads(client);

  await assert.rejects(client.listRules({ query: 'search', text: 'shared', limit: 0 }), error => {
    assert.ok(error instanceof OperationError);
    assert.equal(error.status, 400);
    assert.equal(error.failure.error.kind, 'invalid_input');
    return true;
  });
}

export async function checkCursorReads(client) {
  const seen = new Set();
  let cursor;
  do {
    const page = await client.getRequirementDocument({ id: 'req_shared', limit: 2, cursor });
    assert.ok(page.data.entries.length <= 2);
    for (const entry of page.data.entries) {
      const record = entry.node ?? entry.thread ?? entry.message;
      const key = `${entry.kind}:${record.node_type ?? ''}:${record.id}`;
      assert.equal(seen.has(key), false, 'pages do not repeat identities');
      seen.add(key);
    }
    cursor = page.meta.next_cursor ?? undefined;
    assert.equal(page.meta.has_more, cursor !== undefined);
  } while (cursor !== undefined);
  assert.ok(seen.has('member:requirement:req_shared'));
}
