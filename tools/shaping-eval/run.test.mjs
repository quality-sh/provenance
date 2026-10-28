import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { score } from './score.mjs';
const { prepare, snapshot } = await import('./run.mjs');
// Implementation aid: real public CLI writes verify fixture setup and snapshot capture.
test('real CLI fixture contains the grounded active Rule negative control', async () => {
  assert.equal(typeof prepare, 'function');
  const workspace = await mkdtemp(join(tmpdir(), 'shaping-fixture-'));
  try {
    await prepare(workspace, { variant: 'base' });
    const graph = await snapshot(workspace);
    assert.deepEqual(graph.requirements.find(r => r.id === 'req_health').refines, ['req_robot']);
    assert.equal(graph.rules.find(r => r.id === 'rule_negative_control').status, 'active');
    assert.equal(graph.resolutions.find(r => r.id === 'res_negative_control').status, 'proposed');
    assert.match(graph.rules.find(r => r.id === 'rule_negative_control').source_document, /Owner brief/);
    assert.deepEqual(score(graph).findings, []);
  } finally { await rm(workspace, { recursive: true, force: true }); }
});
// Implementation aid: exit zero cannot hide a structured runner failure.
test('runner completion rejects structured errors and incomplete output', async () => {
  const { completedEvents } = await import('./run.mjs');
  assert.equal(typeof completedEvents, 'function');
  assert.throws(() => completedEvents({ code: 0, stdout: '{"type":"error"}\n' }), /failed/);
  assert.throws(() => completedEvents({ code: 0, stdout: '{"type":"turn.started"}\n' }), /incomplete/);
  assert.deepEqual(completedEvents({ code: 0, stdout: '{"type":"turn.completed"}\n' }), [{ type: 'turn.completed' }]);
});
// Implementation aid: CLI launchers can leave a child holding the output pipe.
test('a runner timeout also stops the child process', async () => {
  const { command } = await import('./run.mjs');
  const start = performance.now();
  const script = "require('node:child_process').spawn(process.execPath, ['-e', 'setTimeout(() => {}, 2000)'], { stdio: 'inherit' }); setTimeout(() => {}, 2000);";
  const result = await command(process.execPath, ['-e', script], tmpdir(), 100);
  assert.equal(result.timedOut, true);
  assert.ok(performance.now() - start < 1500, 'The child kept the output pipe open after the timeout.');
});
