import test from 'node:test';
import assert from 'node:assert/strict';
const { score, compare } = await import('./score.mjs');
// Implementation aid: these examples check the evaluation rubric, not product behaviour.
test('linked repeated text needs a meaning assessment; unrelated text does not', () => {
  assert.equal(typeof score, 'function');
  const graph = { requirements: [{ id: 'parent', statement: 'Same' }, { id: 'child', statement: 'Same', refines: ['parent'] }, { id: 'other', statement: 'Same' }], rules: [], resolutions: [] };
  assert.deepEqual(score(graph).findings.map(f => f.recordIds), [['parent', 'child']]);
  assert.equal(score(graph).findings[0].reviewerAssessment, null);
});
test('active lifecycle with a proposed producer is not an acceptance finding', () => {
  assert.equal(typeof score, 'function');
  const result = score({ requirements: [], resolutions: [{ id: 'res', status: 'proposed', position: 'Choose daily', rationale: 'A daily limit bounds loss.' }], rules: [{ id: 'rule', statement: 'Loss is at most ten.', status: 'active', resolution_ids: ['res'], source_document: 'Owner brief, section 2' }] });
  assert.deepEqual(result.findings, []);
});
test('every category must improve separately for every runner', () => {
  assert.equal(typeof compare, 'function');
  const base = { a: [2, 1, 0, 3], b: [0, 0, 1, 0] };
  assert.equal(compare(base, { a: [1, 0, 0, 2], b: [0, 0, 0, 0] }).passed, true);
  assert.equal(compare(base, { a: [2, 0, 0, 2], b: [0, 0, 0, 0] }).passed, false);
  assert.equal(compare(base, { a: [0, 0, 1, 0], b: [0, 0, 0, 0] }).passed, false);
  assert.equal(compare(base, { a: [1, 0, 0, 2] }).passed, false);
});
// Implementation aid: incomplete human review must not produce a zero score.
test('assessment totals require completed categories and evidenced findings', async () => {
  const { totals, categories } = await import('./score.mjs');
  assert.throws(() => totals({ completedCategories: [], findings: [] }), /incomplete/);
  assert.deepEqual(totals({ completedCategories: categories, findings: [{ category: 'placement', recordIds: ['rule'], evidenceLocator: 'graph.json:rule', reviewerAssessment: { isError: true, reason: 'The Rule belongs under health.' } }] }), [0, 1, 0, 0]);
  assert.throws(() => totals({ completedCategories: categories, findings: [{ category: 'state', recordIds: ['rule'] }] }), /evidence/);
});
