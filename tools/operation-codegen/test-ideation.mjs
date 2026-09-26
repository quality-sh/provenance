import assert from 'node:assert/strict';
import { join } from 'node:path';

const ideationDir = root => join(root, '.provenance/state/scopes/default/ideation');

async function seedEvidence(root) {
  const contribution = {
    schema_version: 2, scope_id: 'default', id: 'contribution_ts',
    target: { artifact_type: 'requirement', artifact_id: 'req_overtime' },
    participant_slot: 'reviewer', stance: 'support', strongest_finding: 'Observed',
    evidence_references: [{ reference_id: 'evidence_ts', evidence_type: 'source', summary: 'Pinned' }],
    material_claims: [{ claim_id: 'claim_ts', statement: 'Observed', evidence_type: 'source', evidence_reference_ids: ['evidence_ts'] }],
    risks: [], objections: [], challenges: [], suggested_artifact_changes: [],
    unsupported_recommendations: [], uncertainty: { level: 'low', rationale: 'Direct' },
    open_questions: [],
  };
  const packet = {
    schema_version: 2, scope_id: 'default', id: 'synthesis_ts',
    target: { artifact_type: 'requirement', artifact_id: 'req_overtime' },
    summary: 'Adjudicated', consensus: [], contested_claims: [], minority_objections: [],
    evidence_gaps: [], unsupported_speculation: [], open_questions: [],
    suggested_artifacts: [{ proposal_id: 'proposal_ts', proposal_key: 'overtime', proposal_type: 'requirement_candidate', summary: 'Candidate', origin_participant_slots: ['reviewer'] }],
    required_human_decisions: [],
  };
  const directory = ideationDir(root);
  const { mkdir, appendFile } = await import('node:fs/promises');
  await mkdir(directory, { recursive: true });
  for (const [name, record] of [['contributions.jsonl', contribution], ['synthesis_packets.jsonl', packet]]) {
    await appendFile(join(directory, name), `${JSON.stringify(record)}\n`);
  }
}

async function seedLargeProposals(root) {
  const summary = 'x'.repeat(850_000);
  const records = Array.from({ length: 20 }, (_, index) => ({
    schema_version: 2, scope_id: 'default', id: `proposal_large_${String(index).padStart(2, '0')}`,
    proposal_key: `large-${index}`, proposal_type: 'requirement_candidate',
    title: `Large proposal ${index}`, summary,
    traceability: {
      target: { artifact_type: 'requirement', artifact_id: 'req_overtime' },
      source_ids: [], evidence_references: [], supporting_claim_ids: [],
    },
    builds_on: [], promotion_state: 'proposed',
  }));
  const { appendFile } = await import('node:fs/promises');
  await appendFile(join(ideationDir(root), 'proposal_cards.jsonl'), `${records.map(JSON.stringify).join('\n')}\n`);
  return records.map(record => record.id);
}

export async function checkIdeation({ HttpClient, OperationError }, fixture) {
  const client = await HttpClient.connectWithBearer(fixture.url, 'fixture-secret', undefined, {
    repository: 'fixture', scope: 'default',
  });
  await client.createRequirement({
    idempotency_key: 'create-ideation-target',
    data: {
      actor: 'fixture', id: 'req_overtime', statement: 'The system defines overtime.',
      status: 'active', depends_on: [], supersedes: [],
    },
  });
  const proposal = {
    id: 'proposal_ts', proposal_key: 'overtime', proposal_type: 'requirement_candidate',
    title: 'Overtime', summary: 'Clarify overtime.',
    traceability: {
      target: { artifact_type: 'requirement', artifact_id: 'req_overtime' },
      source_ids: [], evidence_references: [], supporting_claim_ids: ['claim_ts'],
    },
    builds_on: [], promotion_state: 'proposed',
  };
  const created = await client.createProposal({ data: proposal });
  assert.equal(created.data.id, proposal.id);
  await assert.rejects(client.createProposal({ data: proposal }), error => {
    assert.ok(error instanceof OperationError);
    assert.equal(error.failure.error.kind, 'write_failed');
    return true;
  });

  await seedEvidence(fixture.root);
  const assertion = await client.createProposalAssertion({
    id: proposal.id,
    data: { id: 'assertion_ts', synthesis_packet_id: 'synthesis_ts', supporting_claim_ids: ['claim_ts'] },
  });
  assert.equal(assertion.data.proposal_id, proposal.id);
  assert.equal((await client.getProposalAssertion({ id: proposal.id, fact_id: assertion.data.id })).data.id, assertion.data.id);
  assert.equal((await client.listProposalAssertions({ id: proposal.id })).data.items.length, 1);
  assert.equal((await client.listAssertions({})).data.items.length, 1);

  const disposition = await client.createProposalDisposition({
    id: proposal.id,
    data: {
      id: 'disposition_ts', decision: 'rejected', rationale: 'Reviewed',
      actor: { identity_type: 'human', id: 'reviewer' },
    },
  });
  assert.equal(disposition.data.proposal_id, proposal.id);
  assert.equal((await client.getProposalDisposition({ id: proposal.id, fact_id: disposition.data.id })).data.id, disposition.data.id);
  assert.equal((await client.listDispositions({})).data.items.length, 1);
  assert.equal((await client.listProposals({})).data.items[0].promotion_state, 'rejected');

  const largeIds = await seedLargeProposals(fixture.root);
  const seen = new Set();
  let cursor;
  do {
    const page = await client.listProposals({ limit: 200, ...(cursor === undefined ? {} : { cursor }) });
    assert.equal(page.meta.limit, 200);
    assert.ok(page.data.items.length > 0);
    assert.ok(page.data.items.length < largeIds.length);
    for (const item of page.data.items) {
      assert.ok(!seen.has(item.id), `duplicate proposal ${item.id}`);
      seen.add(item.id);
    }
    cursor = page.meta.next_cursor ?? undefined;
    assert.equal(page.meta.has_more, cursor !== undefined);
  } while (cursor !== undefined);
  for (const id of largeIds) assert.ok(seen.has(id), `missing proposal ${id}`);

  await assert.rejects(client.createProposalDisposition({
    id: proposal.id,
    data: {
      id: 'disposition_second', decision: 'rejected', rationale: 'Reviewed',
      actor: { identity_type: 'human', id: 'reviewer' },
    },
  }), error => {
    assert.ok(error instanceof OperationError);
    assert.equal(error.failure.error.kind, 'write_failed');
    return true;
  });
}
