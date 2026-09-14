import assert from 'node:assert/strict';

export async function checkStatements({ HttpClient, OperationError }, fixture) {
  const client = await HttpClient.connect(fixture.url);
  for (const statement of ['Install the cover.', 'Stop; wait.', 'Café; stop.']) {
    const call = { data: { statement } };
    const raw = await fetch(`${fixture.url}/statement-checks`, {
      method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(call),
    });
    assert.equal(raw.status, 200);
    assert.deepEqual(await client.checkStatement(call), await raw.json());
  }
  await assert.rejects(client.checkStatement({ data: {} }), error => {
    assert.ok(error instanceof OperationError);
    assert.equal(error.status, 400);
    assert.equal(error.failure.error.kind, 'invalid_input');
    assert.deepEqual(error.failure.meta, {});
    return true;
  });
}
