import assert from 'node:assert/strict';

export async function checkDiscussions({ HttpClient, OperationError }, fixture) {
  const client = await HttpClient.connectWithBearer(fixture.url, 'fixture-secret', undefined, {
    repository: 'fixture', scope: 'default',
  });
  const source = await client.createSource({ data: {
    id: 'source_ts', name: 'Discussion source', source_type: 'policy', supersedes: [],
  } });
  assert.equal(source.data.id, 'source_ts');

  const first = await client.sourceCreateDiscussion({
    id: source.data.id,
    idempotency_key: 'discussion_first',
    data: { actor: 'fixture', role: 'system', body: ' TypeScript text ' },
  });
  assert.equal(first.data.status, 'active');
  assert.ok(first.data.discussion_id);

  const second = await client.sourceCreateDiscussionMessage({
    id: source.data.id,
    discussion_id: first.data.discussion_id,
    idempotency_key: 'discussion_second',
    if_match: String(first.data.version),
    data: { actor: 'fixture', role: 'system', body: 'Second' },
  });
  assert.equal(second.data.version, first.data.version + 1);

  const discussions = await client.sourceListDiscussions({ id: source.data.id });
  assert.equal(discussions.data.items.length, 1);
  const messages = await client.sourceListDiscussionMessages({
    id: source.data.id, discussion_id: first.data.discussion_id,
  });
  assert.equal(messages.data.items.length, 2);

  await assert.rejects(client.sourceCreateDiscussionMessage({
    id: source.data.id,
    discussion_id: first.data.discussion_id,
    idempotency_key: 'discussion_empty',
    if_match: String(second.data.version),
    data: { actor: 'fixture', role: 'system', body: ' ' },
  }), error => {
    assert.ok(error instanceof OperationError);
    assert.equal(error.failure.error.kind, 'empty_message_body');
    return true;
  });
}
