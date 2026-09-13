import assert from 'node:assert/strict';

function verifies() {}

export async function checkReview({ HttpClient, OperationError }, fixture) {
  verifies('rule_review_uses_generated_http_client', 'conformance');
  const client = await HttpClient.connectWithBearer(fixture.url, 'fixture-secret');
  const context = { repository: 'fixture', scope: 'default' };
  const create = { request_id: 'create_review', actor: 'worker', create: {
    scope_id: 'default', id: 'req_review', statement: 'The review client saves changes.',
    description: null, status: 'discovery', domain_id: null, refines: null,
    depends_on: [], supersedes: [], spawned_by: null, origin_thread: null, origin_message: null,
  }, origin: null };
  const created = await client.createReviewRequirement({ context, request: create });
  assert.equal(created.outcome, 'created');
  assert.deepEqual(await client.requirementCreationReceipt({ context, request: create }), created);

  const state = await client.requirementEditState({ context, request: { requirement_id: 'req_review' } });
  const save = { request_id: 'save_review', actor: 'worker', expected_etag: state.etag,
    update: { scope_id: 'default', id: 'req_review', description: 'The generated client keeps the result.' }, relationships: null };
  const saved = await client.saveRequirement({ context, request: save });
  assert.equal(saved.outcome, 'changed');
  assert.deepEqual(await client.requirementSaveReceipt({ context, request: {
    requirement_id: 'req_review', request_id: 'save_review', actor: 'worker', declared_by: null,
  } }), saved);

  const history = await client.reviewHistory({ context, request: { requirement_id: 'req_review', limit: 1, cursor: null } });
  assert.equal(history.entries.length, 1);
  assert.equal(typeof history.next_cursor, 'string');
  const evidence = await client.reviewEvidence({ context, request: {
    requirement_id: 'req_review', entry_id: saved.id, before: false, field: 'description', offset: 0,
  } });
  assert.equal(evidence.field, 'description');
  assert.equal(typeof evidence.json_text, 'string');

  const discussion = { scope_id: 'default', parent: { node_type: 'requirement', node_id: 'req_review' },
    request_id: 'discussion_review', actor: 'reviewer', declared_by: null,
    action: { kind: 'start', role: 'user', body: 'Please explain this change.' } };
  const discussed = await client.writeDiscussion({ context, request: discussion });
  assert.deepEqual(await client.discussionReceipt({ context, request: discussion }), discussed);
  const groups = await client.reviewDiscussions({ context, request: { parent: discussion.parent, limit: 50, cursor: null } });
  assert.equal(groups.entries.length, 1);
  const messages = await client.reviewDiscussionMessages({ context, request: { parent: discussion.parent,
    selector: { kind: 'discussion', discussion_id: discussed.discussion_id }, limit: 50, cursor: null } });
  assert.equal(messages.entries.length, 1);

  const submit = { scope_id: 'default', request_id: 'submit_review', actor: 'worker', requirement_id: 'req_review',
    declared_by: null, proposal_id: 'proposal_review', proposal_key: 'review', title: 'Review Requirement',
    summary: 'Review the current revision.', confidence: null, source_ids: [], evidence_references: [],
    builds_on: [], expected_revision: saved.revision, revises: null };
  assert.equal(await client.requirementReviewReceipt({ context, request: { kind: 'submit', request: submit } }), null);
  const submitted = await client.submitRequirementReview({ context, request: submit });
  assert.deepEqual(await client.requirementReviewReceipt({ context, request: { kind: 'submit', request: submit } }), submitted);
  let decisions = await client.requirementDecisionState({ context, request: { requirement_id: 'req_review' } });
  assert.equal(decisions.pending.proposal_id, 'proposal_review');

  const withdraw = { scope_id: 'default', request_id: 'withdraw_review', actor: 'worker',
    proposal_id: 'proposal_review', declared_by: null, reason: 'A new candidate will replace this one.' };
  const withdrawn = await client.withdrawRequirementReview({ context, request: withdraw });
  assert.deepEqual(await client.requirementReviewReceipt({ context, request: { kind: 'withdraw', request: withdraw } }), withdrawn);
  decisions = await client.requirementDecisionState({ context, request: { requirement_id: 'req_review' } });
  assert.equal(decisions.pending, null);
  assert.deepEqual(decisions.withdrawn, ['proposal_review']);

  const replacement = { ...submit, request_id: 'submit_replacement', proposal_id: 'proposal_replacement',
    proposal_key: 'replacement', title: 'Replacement review' };
  await client.submitRequirementReview({ context, request: replacement });
  const decide = { scope_id: 'default', request_id: 'decide_review',
    actor: { identity_type: 'human', id: 'reviewer' }, proposal_id: 'proposal_replacement',
    disposition_id: 'disposition_review', decision: 'rejected', rationale: 'Revise this Requirement.',
    canonical_artifact: null, feedback: null, declared_by: null };
  const decided = await client.decideRequirementReview({ context, request: decide });
  assert.deepEqual(await client.requirementReviewReceipt({ context, request: { kind: 'decide', request: decide } }), decided);
  decisions = await client.requirementDecisionState({ context, request: { requirement_id: 'req_review' } });
  assert.equal(decisions.pending, null);
  assert.equal(decisions.decisions[0].disposition.decision, 'rejected');

  await assert.rejects(client.saveRequirement({ context, request: { ...save,
    request_id: 'wrong_scope', update: { ...save.update, scope_id: 'other' } } }), error => {
    assert.ok(error instanceof OperationError);
    assert.equal(error.failure.error.kind, 'scope_mismatch');
    return true;
  });
}
