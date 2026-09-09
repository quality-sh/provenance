import assert from 'node:assert/strict';
import { join } from 'node:path';

const ideationDir = root => join(root, '.provenance/state/scopes/default/ideation');

async function seedEvidence(root, id, claim, proposalId) {
  const contribution = {
    schema_version: 2,
    scope_id: 'default',
    id: `contribution_${id}`,
    target: { artifact_type: 'requirement', artifact_id: 'req_overtime' },
    participant_slot: 'reviewer',
    stance: 'support',
    strongest_finding: 'Observed',
    evidence_references: [
      { reference_id: `evidence_${id}`, evidence_type: 'source', summary: 'Pinned' },
    ],
    material_claims: [
      { claim_id: claim, statement: 'Observed', evidence_type: 'source', evidence_reference_ids: [`evidence_${id}`] },
    ],
    risks: [],
    objections: [],
    challenges: [],
    suggested_artifact_changes: [],
    unsupported_recommendations: [],
    uncertainty: { level: 'low', rationale: 'Direct' },
    open_questions: [],
  };
  const packet = {
    schema_version: 2,
    scope_id: 'default',
    id: `synthesis_${id}`,
    target: { artifact_type: 'requirement', artifact_id: 'req_overtime' },
    summary: 'Adjudicated',
    consensus: [],
    contested_claims: [],
    minority_objections: [],
    evidence_gaps: [],
    unsupported_speculation: [],
    open_questions: [],
    suggested_artifacts: [
      { proposal_id: proposalId, proposal_key: 'overtime', proposal_type: 'requirement_candidate', summary: 'Candidate', origin_participant_slots: ['reviewer'] },
    ],
    required_human_decisions: [],
  };
  const directory = ideationDir(root);
  const { mkdir, appendFile } = await import('node:fs/promises');
  await mkdir(directory, { recursive: true });
  for (const [name, record] of [['contributions.jsonl', contribution], ['synthesis_packets.jsonl', packet]]) {
    await appendFile(join(directory, name), `${JSON.stringify(record)}\n`);
  }
}

export async function checkIdeation({ HttpClient, OperationError, UncertainWriteError }, fixture) {
  const client = await HttpClient.connectWithBearer(fixture.url, 'fixture-secret');
  const context = { repository: 'fixture', scope: 'default' };
  const proposal = {
    scope_id: 'default',
    id: 'proposal_ts',
    proposal_key: 'overtime',
    proposal_type: 'requirement_candidate',
    title: 'Overtime',
    summary: 'Clarify overtime.',
    traceability: {
      target: { artifact_type: 'requirement', artifact_id: 'req_overtime' },
      source_ids: [],
      evidence_references: [],
      supporting_claim_ids: ['claim_ts'],
    },
    builds_on: [],
    promotion_state: 'proposed',
  };
  const created = await client.createProposal({ context, request: proposal });
  assert.equal(created.id, 'proposal_ts');
  assert.equal(created.promotion_state, 'proposed');
  assert.equal(created.schema_version, 2);
  // A duplicate proposal is an unclassified refusal before publication, so
  // the client reports the write outcome as uncertain instead of guessing.
  await assert.rejects(client.createProposal({ context, request: proposal }), error => {
    assert.ok(error instanceof UncertainWriteError);
    assert.equal(error.failure.error.kind, 'write_failed');
    return true;
  });
  // The evidence a supported assertion rests on.
  await seedEvidence(fixture.root, 'ts', 'claim_ts', 'proposal_ts');
  const assertion = await client.createAssertion({
    context,
    request: {
      scope_id: 'default',
      id: 'assertion_ts',
      proposal_id: 'proposal_ts',
      synthesis_packet_id: 'synthesis_ts',
      supporting_claim_ids: ['claim_ts'],
    },
  });
  assert.equal(assertion.proposal_id, 'proposal_ts');
  const asserted = await client.listProposals({ context, request: null });
  assert.deepEqual(asserted, [{ ...created, promotion_state: 'asserted' }]);
  assert.deepEqual(await client.listAssertions({ context, request: null }), [assertion]);
  assert.deepEqual(await client.listDispositions({ context, request: null }), []);
  const rejected = {
    scope_id: 'default',
    id: 'disposition_ts',
    proposal_id: 'proposal_ts',
    decision: 'rejected',
    rationale: 'Reviewed',
    actor: { identity_type: 'human', id: 'reviewer' },
  };
  const disposition = await client.createDisposition({ context, request: rejected });
  assert.deepEqual(disposition, { ...rejected, schema_version: 2 });
  const after = await client.listDispositions({ context, request: null });
  assert.deepEqual(after, [disposition]);
  const settled = await client.listProposals({ context, request: null });
  assert.deepEqual(settled, [{ ...created, promotion_state: 'rejected' }]);
  // The stored definition keeps the state its author wrote.
  const rows = await import('node:fs/promises').then(fs =>
    fs.readFile(join(ideationDir(fixture.root), 'proposal_cards.jsonl'), 'utf8'),
  );
  assert.equal(JSON.parse(rows.trim()).promotion_state, 'proposed');
  // The projection is the effective state, not the raw definition list.
  const effective = await client.listProposals({ context, request: null });
  assert.equal(effective[0].promotion_state, 'rejected');
  for (const [request, kind, uncertain] of [
    [{ ...rejected, id: 'disposition_second' }, 'write_failed', true],
    [{ ...rejected, scope_id: 'other' }, 'scope_mismatch', false],
    [{ ...rejected, actor: { identity_type: 'human', id: 'forged-reviewer' } }, 'write_failed', true],
    [{ ...rejected, rationale: '   ' }, 'write_failed', true],
  ]) {
    await assert.rejects(client.createDisposition({ context, request }), error => {
      assert.ok(error instanceof (uncertain ? UncertainWriteError : OperationError));
      assert.equal(error.failure.error.kind, kind);
      return true;
    });
  }
  assert.deepEqual(await client.listDispositions({ context, request: null }), [disposition]);
}
