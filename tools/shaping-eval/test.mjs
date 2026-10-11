import test from 'node:test';
import assert from 'node:assert/strict';
import * as scoring from './score.mjs';
const { compare } = scoring;
const authored = (...args) => { assert.equal(typeof scoring.authored, 'function'); return scoring.authored(...args); };
const assess = (...args) => { assert.equal(typeof scoring.assess, 'function', 'human label assessment missing'); return scoring.assess(...args); };
// Implementation aids: these examples check evaluation arithmetic, not writing quality.
test('each runner must improve contamination without losing grounding', () => {
  const baseline = { codex: { total: 10, contaminated: 2, rationales: 2, grounding: [2, 2, 2, 2] } };
  assert.equal(compare(baseline, { codex: { total: 20, contaminated: 2, rationales: 2, grounding: [2, 2, 2, 2] } }, ['codex']).passed, true);
  assert.equal(compare(baseline, { codex: { total: 20, contaminated: 2, rationales: 2, grounding: [2, 1, 2, 2] } }).passed, false);
  assert.equal(compare(baseline, {}).passed, false);
  assert.equal(compare({ codex: { total: 10, contaminated: 0, rationales: 2, grounding: [2, 2, 2, 2] } }, baseline).passed, false);
});
test('seed records do not enter the authored denominator', () => {
  const seed = { requirements: [{ id: 'req_seed', statement: 'Seed.' }] };
  const graph = { requirements: [...seed.requirements, { id: 'req_new', statement: 'New.' }] };
  assert.deepEqual(authored(seed, graph).map(r => r.id), ['req_new']);
});
test('missing human labels cannot pass', () => {
  const records = [{ id: 'res_new', rationale: 'Reason.' }];
  assert.throws(() => assess(records, []), /label/);
  assert.throws(() => assess(records, [{ id: 'res_new', contaminated: false }]), /grounding/);
  assert.deepEqual(assess(records, [{ id: 'res_new', contaminated: false, grounding: { position: true, locator: true, reason: true, alternative: true } }]), { total: 1, contaminated: 0, rationales: 1, grounding: [1, 1, 1, 1] });
});

// Implementation aid: omitted configured evidence must not approve release.
test('comparison requires both approved runners by default', () => {
  const base = { codex: { total: 10, contaminated: 2, rationales: 1, grounding: [1, 1, 1, 1] } };
  const candidate = { codex: { total: 10, contaminated: 1, rationales: 1, grounding: [1, 1, 1, 1] } };
  assert.equal(compare(base, candidate).passed, false);
});
