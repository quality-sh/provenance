import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';

export async function checkWrites({ HttpClient, OperationError }, fixture) {
  const client = await HttpClient.connectWithBearer(fixture.url, 'fixture-secret');
  const context = { repository: 'fixture', scope: 'default' };
  const request = { schema_version: 2, spec: 'typescript', declared_by: 'fixture-ts',
    requirements: [{ key: 'ready', statement: 'The system is ready.' }],
    rules: [{ key: 'ready', requirement: 'ready', statement: 'The system is ready.' }] };
  const call = { context, request };
  const planned = await client.plan(call);
  assert.equal(planned.created, 2);
  assert.deepEqual((await client.search({ context, request: { text: 'ready' } })).nodes, []);
  const applied = await client.apply(call);
  assert.equal(applied.created, 2);
  assert.equal((await client.plan(call)).unchanged, 2);
  const rule = applied.resources.find(resource => resource.kind === 'rule').id;
  const run = await client.beginVerification({ context, request: { rule, key: 'ready', method: 'examples', declared_by: 'fixture-ts', file: 'check.rs' } });
  assert.equal(run.status, 'running');
  const done = await client.completeVerification({ context, request: { run: run.id, status: 'passed' } });
  assert.equal(done.status, 'passed');
  for (const [status, kind] of [['bogus', 'invalid_completion'], ['passed', 'already_complete']]) {
    await assert.rejects(client.completeVerification({ context, request: { run: run.id, status } }), error => {
      assert.ok(error instanceof OperationError);
      assert.equal(error.failure.error.kind, kind);
      return true;
    });
  }
  const conflicts = structuredClone(request);
  conflicts.declared_by = 'another-owner';
  for (const resource of applied.resources) {
    const declarations = resource.kind === 'rule' ? conflicts.rules : conflicts.requirements;
    declarations[0].id = resource.id;
  }
  await assert.rejects(client.apply({ context, request: conflicts }), error => {
    assert.ok(error instanceof OperationError);
    assert.equal(error.failure.error.kind, 'ownership_conflict');
    assert.ok(error.failure.error.conflicts.length > 0);
    return true;
  });
  const persisted = await readFile(join(fixture.root, '.provenance/cache/scopes/default/verification-runs.jsonl'), 'utf8');
  assert.ok(persisted.split('\n').filter(Boolean).map(JSON.parse).some(value => value.id === run.id && value.status === 'passed'));
}
