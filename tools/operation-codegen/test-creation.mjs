import assert from 'node:assert/strict';

export async function checkCreation({ HttpClient, OperationError }, fixture) {
  const client = await HttpClient.connectWithBearer(fixture.url, 'fixture-secret', undefined, {
    repository: 'fixture', scope: 'default',
  });
  const source = {
    id: 'source_ts', name: 'Policy', source_type: 'policy', supersedes: [],
    origin_thread: 'thread_origin', origin_message: 'message_origin',
  };
  const created = await client.createSource({ data: source });
  assert.equal(created.data.origin_thread, 'thread_origin');
  assert.equal(created.data.origin_message, 'message_origin');
  assert.equal(created.data.scope_id, 'default');

  await assert.rejects(client.createSource({ data: source }), error => {
    assert.ok(error instanceof OperationError);
    assert.equal(error.failure.error.kind, 'already_exists');
    return true;
  });

  const updated = await client.updateSource({ id: source.id, data: {
    url: 'https://example.test/new', reference: 'section 2', commit_pin: 'abcdef0123456789',
  } });
  assert.equal(updated.data.origin_thread, 'thread_origin');
  assert.equal(updated.data.url, 'https://example.test/new');

  const cleared = await client.updateSource({ id: source.id, data: {
    clear_fields: ['reference', 'commit_pin'],
  } });
  assert.equal(cleared.data.url, 'https://example.test/new');
  assert.equal(cleared.data.reference, undefined);
  assert.equal(cleared.data.commit_pin, undefined);

  await assert.rejects(client.updateSource({ id: source.id, data: {
    url: 'conflict', clear_fields: ['url'],
  } }), error => {
    assert.ok(error instanceof OperationError);
    assert.equal(error.failure.error.kind, 'invalid_update');
    return true;
  });
  assert.equal((await client.getSource({ id: source.id })).data.id, source.id);
}
