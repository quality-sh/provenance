import assert from 'node:assert/strict';

export async function checkEvidence({ HttpClient, OperationError }, fixture) {
  const identity = { repository: fixture.targets.first, scope: 'default' };
  const client = await HttpClient.connectWithBearer(fixture.url, fixture.bearer, undefined, identity);
  const evidence = fixture.evidence;
  assert.ok(evidence, 'Fixture must seed evidence and real Git commits');

  const current = await client.getRuleEvidence({ id: evidence.rule_id });
  assert.equal(current.data.stale, null);
  assert.ok(current.data.verification_runs.length > 1);
  assert.ok(current.meta.stamp);

  const based = await client.getRuleEvidence({ id: evidence.rule_id, base: evidence.base });
  assert.equal(based.data.stale.base, evidence.base);
  assert.ok(based.data.stale.sites.length > 0);

  const impact = await client.getRule({ id: evidence.rule_id, query: 'impact' });
  assert.ok(impact.data.affected_rules.length > 0);
  assert.equal(typeof impact.data.scan_cut, 'boolean');

  const symbol = await client.listRules({ query: 'resolve-symbol', file: evidence.file });
  assert.ok(symbol.data.items.length > 0);
  const stale = await client.listRules({ query: 'stale', base: evidence.base });
  assert.ok(stale.data.items.length > 0);

  for (const method of ['listVerificationRuns', 'listVerificationBindings']) {
    const result = await client[method]({});
    assert.ok(Array.isArray(result.data.items));
    assert.ok(result.data.items.length > 1);
  }

  await assert.rejects(
    client.listRules({ query: 'resolve-symbol', file: '../outside.rs' }),
    error => {
      assert.ok(error instanceof OperationError);
      assert.equal(error.failure.error.kind, 'file_access_denied');
      return true;
    },
  );
}
