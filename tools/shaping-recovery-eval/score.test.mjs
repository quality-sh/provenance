import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import test from 'node:test';

const binary = process.env.SHAPING_EVAL_CLI;
const here = dirname(fileURLToPath(import.meta.url));
function score(turns, scenario) {
  const result = spawnSync(process.execPath, [join(here, 'score.mjs'), '--stdin'], { input: JSON.stringify({ turns, scenario }), encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
  return JSON.parse(result.stdout);
}
assert.ok(binary, 'SHAPING_EVAL_CLI must name a real CLI artifact');
function cli(repo, args, body) {
  const result = spawnSync(binary, [...args, '--repo', repo, '--scope', 'default', '--format', 'json'],
    { input: body && JSON.stringify(body), encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
  return JSON.parse(result.stdout).data;
}
function fixture() {
  const repo = mkdtempSync(join(tmpdir(), 'shaping-score-'));
  const init = spawnSync(binary, ['init', '--path', repo, '--scope', 'default'], { encoding: 'utf8' });
  assert.equal(init.status, 0, init.stderr);
  cli(repo, ['requirements', 'create', '--stdin'], { id: 'req_pay', statement: 'The system calculates overtime pay.' });
  cli(repo, ['topics', 'create', '--stdin'], { id: 'topic_pay', requirement_id: 'req_pay', title: 'Overtime pay' });
  for (const id of ['question_threshold', 'question_rate']) {
    cli(repo, ['questions', 'create', '--stdin'], { id, topic_id: 'topic_pay', question: 'Which value applies?', resolution_method: 'grill' });
  }
  return repo;
}
function snapshot(repo) {
  return ['question_threshold', 'question_rate'].map(id => cli(repo, ['questions', id, 'get']));
}

// Implementation aid: evaluate prerequisite order from real public CLI writes.
test('a saved dependent answer before its prerequisite is a violation', async () => {
  const repo = fixture();
  try {
    const before = snapshot(repo);
    cli(repo, ['questions', 'question_rate', 'answer', '--answer', 'The rate is 30 dollars.']);
    const after = snapshot(repo);
    const result = score([{ before, after }], {
      dependencies: { question_rate: ['question_threshold'] },
      accepted: ['question_rate', 'question_threshold'],
    });
    assert.deepEqual(result.prerequisite_violations, [{ turn: 0, question: 'question_rate', prerequisite: 'question_threshold' }]);
    assert.deepEqual(result.unanswered_accepted_questions, ['question_threshold']);
  } finally { rmSync(repo, { recursive: true, force: true }); }
});

// Implementation aid: compare separate real CLI publications with the same oracle.
test('a prerequisite answered in an earlier turn permits the dependent answer', async () => {
  const repo = fixture();
  try {
    const before = snapshot(repo);
    cli(repo, ['questions', 'question_threshold', 'answer', '--answer', 'The threshold is 38 hours.']);
    const middle = snapshot(repo);
    cli(repo, ['questions', 'question_rate', 'answer', '--answer', 'The rate is 30 dollars.']);
    const after = snapshot(repo);
    const result = score([{ before, after: middle }, { before: middle, after }], {
      dependencies: { question_rate: ['question_threshold'] },
      accepted: ['question_rate', 'question_threshold'],
    });
    assert.deepEqual(result.prerequisite_violations, []);
    assert.deepEqual(result.unanswered_accepted_questions, []);
  } finally { rmSync(repo, { recursive: true, force: true }); }
});
