import assert from 'node:assert/strict';
import test from 'node:test';
import { scorePacket } from './cases.mjs';

// Implementation aid: each case passes on a good session and names the failure that the
// 2026-10-31 shaping session showed.
const question = (id, status = 'open', claimed_by = null) => ({ id, status, claimed_by });
const shell = command => ({ type: 'shell', command });
const graph = (overrides = {}) => ({
  topics: [], sources: [], boundaries: [],
  questions: [question('question_format'), question('question_schedule'), question('question_delivery')],
  requirements: [
    { id: 'req_payroll', statement: 'Payroll.', status: 'active' },
    { id: 'req_pay', statement: 'Overtime pay.', refines: ['req_payroll'], status: 'discovery' },
    { id: 'req_format_table', statement: 'The report shows a table.', refines: ['req_pay'], spawned_by: 'res_format', status: 'discovery' },
  ],
  resolutions: [{ id: 'res_format', position: 'The report shows a table.', status: 'proposed' }],
  rules: [{ id: 'rule_rows', statement: 'Each row shows the pay.', requirement_ids: ['req_format_table'], resolution_ids: ['res_format'], status: 'review' }],
  ...overrides,
});
const review = [
  { kind: 'requirements', id: 'req_format_table', pending: true, decisions: 0 },
  { kind: 'resolutions', id: 'res_format', pending: true, decisions: 0, impact_rules: ['rule_rows'], spawned: ['req_format_table'] },
  { kind: 'rules', id: 'rule_rows', pending: true, decisions: 0 },
];
function packet({ pages = [{ root: 'req_payroll', url: 'http://127.0.0.1:1/?root=req_payroll' }], answer = 'View: http://127.0.0.1:1/?root=req_payroll',
  final = graph(), finalReview = review, events = [shell('provenance topics topic_pay get'), shell('provenance questions question_format get'),
    shell('provenance questions question_format answer --answer table')] } = {}) {
  return {
    packet: 'test', anchor: 'req_pay', anchor_parent: 'req_payroll', fixture_records: ['req_payroll', 'req_pay'],
    dependencies: { question_delivery: ['question_schedule'] },
    expected_decisions: [{ id: 'req_format_table', terms: ['table'], choice: true }],
    refused_terms: [['bonus']],
    turns: [
      { index: 0, opened_pages: pages, answer, tool_events: [shell('provenance topics topic_pay get')], before: graph(), after: graph() },
      { index: 1, opened_pages: [], answer: '', tool_events: events, before: graph(), after: final, review: finalReview },
    ],
  };
}
const failing = result => Object.entries(result.cases).filter(([, findings]) => findings.length).map(([name]) => name);

test('a session that follows the skill passes every case', () => {
  const result = scorePacket(packet());
  assert.deepEqual(failing(result), []);
  assert.equal(result.pass, true);
});

test('failure 1: no page before the first question', () => {
  assert.deepEqual(failing(scorePacket(packet({ pages: [], answer: 'Which rate applies?' }))), ['view_before_first_question', 'anchor_inside_parent_tree']);
});

test('failure 2: a Rule directly under the anchor and a choice with no Resolution', () => {
  const final = graph();
  final.rules = [{ id: 'rule_flat', statement: 'Rows show pay.', requirement_ids: ['req_pay'], status: 'review' }];
  final.requirements[2] = { ...final.requirements[2], spawned_by: undefined };
  const result = scorePacket(packet({ final, finalReview: review.filter(entry => entry.kind !== 'resolutions') }));
  assert.deepEqual(failing(result), ['tree_branches_by_decision']);
  assert.equal(result.cases.tree_branches_by_decision.length, 2);
});

test('failure 3: a proposed Resolution with an empty impact view', () => {
  const finalReview = review.map(entry => entry.kind === 'resolutions' ? { ...entry, impact_rules: [] } : entry);
  assert.deepEqual(failing(scorePacket(packet({ finalReview }))), ['resolution_shows_downstream']);
});

test('failure 4: only the Requirement has a review submission', () => {
  const finalReview = review.map(entry => entry.kind === 'requirements' ? entry : { ...entry, pending: false });
  const result = scorePacket(packet({ finalReview }));
  assert.deepEqual(failing(result), ['every_record_submitted']);
  assert.deepEqual(result.cases.every_record_submitted.map(entry => entry.id), ['res_format', 'rule_rows']);
});

test('failure 5: the page shows the anchor as a root', () => {
  const pages = [{ root: 'req_pay', url: 'http://127.0.0.1:1/?root=req_pay' }];
  assert.deepEqual(failing(scorePacket(packet({ pages }))), ['anchor_inside_parent_tree']);
});

test('a claim while a stored prerequisite stays open is a prerequisite violation', () => {
  const final = graph();
  final.questions = [question('question_format', 'answered'), question('question_schedule'), question('question_delivery', 'open', 'eval-writer')];
  const events = [shell('provenance topics topic_pay get'), shell('provenance questions question_delivery get'),
    shell('provenance questions question_format get'), shell('provenance questions question_delivery claim --actor eval-writer'),
    shell('provenance questions question_format answer --answer table')];
  const result = scorePacket(packet({ final, events }));
  assert.deepEqual(result.cases.prerequisite_order, [{ turn: 1, question: 'question_delivery', action: 'claimed', open_prerequisite: 'question_schedule' }]);
});

test('a recovery read through a filter or after the write does not count', () => {
  const final = graph();
  final.questions = [question('question_format', 'answered'), question('question_schedule'), question('question_delivery')];
  const events = [shell('provenance topics topic_pay get | jq .data'), shell('provenance questions question_format answer --answer table'),
    shell('provenance questions question_format get')];
  assert.deepEqual(scorePacket(packet({ final, events })).cases.fresh_reads, [{ turn: 1, missing: 'topic_pay' }, { turn: 1, missing: 'question_format' }]);
});
