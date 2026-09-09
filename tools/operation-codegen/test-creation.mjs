import assert from 'node:assert/strict';

export async function checkCreation({ HttpClient, OperationError }, fixture) {
  const client = await HttpClient.connectWithBearer(fixture.url, 'fixture-secret');
  const context = { repository: 'fixture', scope: 'default' };
  const source = { scope_id: 'default', id: 'source_ts', name: 'Policy', source_type: 'policy', supersedes: [], origin_thread: 'thread_origin', origin_message: 'message_origin' };
  const created = await client.createSource({ context, request: source });
  assert.equal(created.origin_thread, 'thread_origin');
  assert.equal(created.origin_message, 'message_origin');
  const requirement = await client.createRequirement({ context, request: { scope_id: 'default', id: 'req_ts', statement: 'The system is ready.', status: 'discovery', depends_on: [], supersedes: [] } });
  assert.equal(requirement.status, 'discovery');
  const resolution = await client.createResolution({ context, request: { scope_id: 'default', id: 'res_ts', title: 'Decision', requirement_ids: [requirement.id], supersedes: [], position: 'Use the existing record.', rationale: 'The shape is fixed.', status: 'draft', inputs: [] } });
  const rule = await client.createRule({ context, request: { scope_id: 'default', id: 'rule_ts', statement: 'The system is ready.', requirement_ids: [requirement.id], resolution_ids: [resolution.id], status: 'draft', severity: 'high' } });
  assert.deepEqual(rule.requirement_ids, [requirement.id]);
  const linked = await client.addSourceReference({ context, request: { scope_id: 'default', source_id: created.id, requirement_id: requirement.id, clause: '1' } });
  assert.deepEqual(linked.source_refs, [{ source_id: created.id, clause: '1' }]);
  await assert.rejects(client.createSource({ context, request: source }), error => {
    assert.ok(error instanceof OperationError);
    assert.equal(error.failure.error.kind, 'already_exists');
    return true;
  });
  const result = await client.get({ context, request: { node_type: 'rule', id: rule.id } });
  assert.equal(result.node.id, rule.id);
}
