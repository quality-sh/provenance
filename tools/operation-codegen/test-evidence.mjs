import assert from 'node:assert/strict';
import { appendFile, readFile } from 'node:fs/promises';
import { join } from 'node:path';

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

  const seenRuns = new Set();
  let runCursor;
  do {
    const page = await client.listVerificationRuns({ limit: 1, cursor: runCursor });
    assert.ok(page.data.items.length <= 1);
    for (const run of page.data.items) {
      assert.equal(seenRuns.has(run.id), false, `duplicate verification run ${run.id}`);
      seenRuns.add(run.id);
    }
    runCursor = page.meta.next_cursor ?? undefined;
    assert.equal(page.meta.has_more, runCursor !== undefined);
  } while (runCursor !== undefined);
  assert.ok(seenRuns.size > 1);

  const firstPage = await client.listVerificationRuns({ limit: 1 });
  const staleCursor = firstPage.meta.next_cursor;
  assert.equal(typeof staleCursor, 'string');
  const runsPath = join(
    fixture.repository_root, '.provenance/cache/scopes/default/verification-runs.jsonl',
  );
  const existing = (await readFile(runsPath, 'utf8')).split('\n').filter(Boolean).map(JSON.parse);
  const added = { ...existing.at(-1), id: 'verification_live_revision' };
  await appendFile(runsPath, `${JSON.stringify(added)}\n`);
  await assert.rejects(
    client.listVerificationRuns({ limit: 1, cursor: staleCursor }),
    error => {
      assert.ok(error instanceof OperationError);
      assert.equal(error.status, 409);
      assert.equal(error.failure.error.kind, 'cursor_revision_changed');
      return true;
    },
  );

  await assert.rejects(
    client.listRules({ query: 'resolve-symbol', file: '../outside.rs' }),
    error => {
      assert.ok(error instanceof OperationError);
      assert.equal(error.failure.error.kind, 'file_access_denied');
      return true;
    },
  );
}
