import assert from 'node:assert/strict';

export async function checkEvidence({ HttpClient, OperationError, PROTOCOL_VERSION }, fixture) {
  const client = await HttpClient.connectWithBearer(fixture.url, fixture.bearer);
  const context = { repository: fixture.targets.first, scope: 'default' };
  const evidence = fixture.evidence;
  assert.ok(evidence, 'Fixture must seed evidence and real Git commits');
  const raw = async (operation, call) => {
    const response = await fetch(`${fixture.url}/v${PROTOCOL_VERSION}/operations/${operation}`, {
      method: 'POST', headers: { 'content-type': 'application/json', authorization: `Bearer ${fixture.bearer}` }, body: JSON.stringify(call),
    });
    return { status: response.status, value: await response.json() };
  };
  const compare = async (operation, call, invoke) => {
    const expected = await raw(operation, call);
    assert.equal(expected.status, 200, JSON.stringify(expected.value));
    const actual = await invoke(call);
    assert.deepEqual(actual, expected.value, `${operation} retains its complete response`);
    return actual;
  };
  const noBase = await compare('evidence', { context, request: { rule: evidence.rule_id } }, call => client.evidence(call));
  assert.equal(noBase.stale, null);
  assert.ok(noBase.verification_runs.length > 1);
  const noGitContext = { repository: fixture.targets.second, scope: 'default' };
  const noGit = await client.evidence({ context: noGitContext, request: { rule: evidence.rule_id, head: 'unused-without-base' } });
  assert.equal(noGit.stale, null);
  assert.equal(Object.hasOwn(noGit, 'latest_verification_run'), false);
  const cut = await compare('evidence', { context, request: { rule: evidence.rule_id, limit: 1 } }, call => client.evidence(call));
  for (const field of ['implementation_bindings', 'verification_bindings', 'verification_runs', 'reviews']) {
    assert.equal(cut[field].length, 1, field);
    assert.equal(cut[`${field}_has_more`], true, field);
  }
  assert.equal(cut.has_more, true);
  const based = await compare('evidence', { context, request: { rule: evidence.rule_id, base: evidence.base } }, call => client.evidence(call));
  assert.equal(based.stale.base, evidence.base);
  assert.ok(based.stale.sites.length > 0);
  const impact = await compare('impact', { context, request: { id: evidence.rule_id } }, call => client.impact(call));
  assert.ok(impact.affected_rules.length > 0);
  assert.equal(typeof impact.scan_cut, 'boolean');
  const symbol = await compare('resolve-symbol', { context, request: { file: evidence.file } }, call => client.resolveSymbol(call));
  assert.ok(symbol.rules.length > 0);
  assert.equal(Object.hasOwn(symbol, 'symbol'), false);
  const stale = await compare('stale', { context, request: { base: evidence.base, rules: [evidence.rule_id] } }, call => client.stale(call));
  assert.ok(stale.sites.length > 0);
  for (const [operation, method] of [['verification-runs', 'verificationRuns'], ['verification-bindings', 'verificationBindings']]) {
    const result = await compare(operation, { context, request: {} }, call => client[method](call));
    assert.ok(Array.isArray(result));
    assert.ok(result.length > 1, 'Lists retain complete arrays');
    const filtered = await compare(operation, { context, request: { rule: evidence.rule_id } }, call => client[method](call));
    assert.ok(filtered.length > 1);
    const other = await client[method]({ context: { ...context, scope: 'other' }, request: {} });
    assert.deepEqual(other, []);
  }
  const refuses = async (operation, call, invoke, kind) => {
    const expected = await raw(operation, call);
    assert.notEqual(expected.status, 200);
    await assert.rejects(invoke(call), error => {
      assert.ok(error instanceof OperationError);
      assert.equal(error.failure.error.kind, kind);
      assert.deepEqual(error.failure, expected.value);
      return true;
    });
  };
  await refuses('resolve-symbol', { context, request: { file: '../outside.rs' } }, call => client.resolveSymbol(call), 'file_access_denied');
  await refuses('evidence', { context: noGitContext, request: { rule: evidence.rule_id, base: 'HEAD' } }, call => client.evidence(call), 'git_unavailable');
  await refuses('stale', { context, request: { base: 'missing-revision' } }, call => client.stale(call), 'git_revision_not_found');
}
