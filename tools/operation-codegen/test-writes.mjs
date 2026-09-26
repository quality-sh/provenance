import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';

export async function checkWrites({ HttpClient, OperationError }, fixture) {
  const client = await HttpClient.connectWithBearer(fixture.url, 'fixture-secret', undefined, {
    repository: 'fixture', scope: 'default',
  });
  const document = {
    schema_version: 2, spec: 'typescript', declared_by: 'fixture-ts',
    requirements: [{ key: 'ready', statement: 'The system is prepared.' }],
    rules: [{ key: 'ready', requirement: 'ready', statement: 'The system is prepared.' }],
  };
  const planned = await client.planAuthoring({ data: document });
  assert.equal(planned.data.created, 2);
  assert.deepEqual((await client.listRules({ query: 'search', text: 'ready' })).data.items, []);
  const applied = await client.applyAuthoring({ data: document });
  assert.equal(applied.data.created, 2);
  assert.equal((await client.planAuthoring({ data: document })).data.unchanged, 2);

  const rule = applied.data.resources.find(resource => resource.kind === 'rule').id;
  const run = await client.beginVerification({ data: {
    rule, key: 'ready', method: 'examples', declared_by: 'fixture-ts', file: 'check.rs',
  } });
  assert.equal(run.data.status, 'running');
  const done = await client.completeVerification({
    run_id: run.data.id, data: { status: 'passed' },
  });
  assert.equal(done.data.status, 'passed');

  for (const [status, kind] of [['bogus', 'invalid_completion'], ['passed', 'already_complete']]) {
    await assert.rejects(
      client.completeVerification({ run_id: run.data.id, data: { status } }),
      error => {
        assert.ok(error instanceof OperationError);
        assert.equal(error.failure.error.kind, kind);
        return true;
      },
    );
  }

  const conflicts = structuredClone(document);
  conflicts.declared_by = 'another-owner';
  for (const resource of applied.data.resources) {
    const declarations = resource.kind === 'rule' ? conflicts.rules : conflicts.requirements;
    declarations[0].id = resource.id;
  }
  await assert.rejects(client.applyAuthoring({ data: conflicts }), error => {
    assert.ok(error instanceof OperationError);
    assert.equal(error.failure.error.kind, 'ownership_conflict');
    assert.ok(error.failure.error.conflicts.length > 0);
    return true;
  });

  const persisted = await readFile(
    join(fixture.root, '.provenance/cache/scopes/default/verification-runs.jsonl'), 'utf8',
  );
  assert.ok(persisted.split('\n').filter(Boolean).map(JSON.parse)
    .some(value => value.id === run.data.id && value.status === 'passed'));
}
