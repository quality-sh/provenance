import assert from 'node:assert/strict';

export async function checkDiscussions({ HttpClient, OperationError }, fixture) {
  const client = await HttpClient.connectWithBearer(fixture.url, 'fixture-secret', undefined, {
    repository: 'fixture', scope: 'default',
  });
  const question = await client.createQuestion({ data: {
    id: 'question_ts', topic_id: null, question: 'Which path is correct?',
    resolution_method: 'human', status: 'open', links: [],
  } });
  assert.equal(question.data.id, 'question_ts');

  const first = await client.questionCreateDiscussion({
    id: question.data.id,
    idempotency_key: 'discussion_first',
    data: { actor: 'fixture', role: 'system', body: ' TypeScript text ' },
  });
  assert.equal(first.data.status, 'open');
  assert.ok(first.data.discussion_id);

  const second = await client.questionCreateDiscussionMessage({
    id: question.data.id,
    discussion_id: first.data.discussion_id,
    idempotency_key: 'discussion_second',
    if_match: String(first.data.version),
    data: { actor: 'fixture', role: 'system', body: 'Second' },
  });
  assert.equal(second.data.version, first.data.version + 1);

  const discussions = await client.questionListDiscussions({ id: question.data.id });
  assert.equal(discussions.data.items.length, 1);
  const messages = await client.questionListDiscussionMessages({
    id: question.data.id, discussion_id: first.data.discussion_id,
  });
  assert.equal(messages.data.items.length, 2);

  await assert.rejects(client.questionCreateDiscussionMessage({
    id: question.data.id,
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
