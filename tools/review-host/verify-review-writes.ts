// Release-check aid: the generated client writes reviews and Discussions through a
// real review host, and each step uses a new host process on the same repository.
import assert from 'node:assert/strict';
import { execFileSync, spawn } from 'node:child_process';
import { once } from 'node:events';
import { chmod, copyFile, mkdir, mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { createInterface } from 'node:readline';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const [binaryArg, sdkArg] = process.argv.slice(2);
if (!binaryArg || !sdkArg) throw new Error('Usage: node verify-review-writes.ts BINARY SDK_PACKAGE');
const { HttpClient } = await import(pathToFileURL(join(resolve(sdkArg), 'dist/client.js')).href);
const work = await mkdtemp(join(tmpdir(), 'provenance-review-writes-'));
const binary = join(work, process.platform === 'win32' ? 'provenance.exe' : 'provenance');
const repository = join(work, 'repository');
const steAssets = join(work, 'ste-assets');
const env = {
  ...process.env,
  PROVENANCE_STE100_ASSET_DIR: steAssets,
  PROVENANCE_STE100_INDEX_DIR: join(work, 'ste-indexes'),
};
const reviewer = { identity_type: 'human', id: 'reviewer' };

/** Starts one review host, gives its client to `step`, and stops the host. */
async function withHost<T>(step: (client: any) => Promise<T>, fetcher: typeof fetch = globalThis.fetch): Promise<T> {
  const host = spawn(binary, [
    'review', '--repo', repository, '--repository-id', 'fixture', '--scope', 'default', '--no-open',
  ], { cwd: work, env: { ...env, PATH: work }, stdio: ['ignore', 'pipe', 'inherit'] });
  const exited = once(host, 'exit');
  try {
    const lines = createInterface({ input: host.stdout! });
    const [line] = await Promise.race([
      once(lines, 'line', { signal: AbortSignal.timeout(30_000) }),
      exited.then(() => { throw new Error('Review host exited before startup'); }),
    ]);
    lines.close();
    const config = JSON.parse(line);
    const client = await HttpClient.connectWithBearer(
      config.endpoint, config.bearer, fetcher, { repository: 'fixture', scope: 'default' },
    );
    return await step(client);
  } finally {
    if (host.exitCode === null && host.signalCode === null) host.kill('SIGTERM');
    await exited;
  }
}

async function createRequirement(client: any, id: string) {
  const created = await client.createRequirement({ data: {
    actor: 'agent', id, statement: 'The record is readable.', status: 'active',
    depends_on: [], supersedes: [],
  } });
  return created.data;
}

// @provenance verification: examples
// @provenance rule: rule_reply_threads_resolve_independently
async function twoDiscussionsKeepTheirRepliesAndStatus(): Promise<void> {
  const id = 'req_discussed';
  const [a, b] = await withHost(async client => {
    await createRequirement(client, id);
    const started = [];
    for (const name of ['A', 'B']) {
      const discussion = await client.requirementCreateDiscussion({
        id, data: { actor: 'reviewer', role: 'user', body: `Concern ${name}.` },
      });
      started.push(discussion.data.discussion_id);
    }
    return started;
  });
  await withHost(async client => {
    for (const [discussion_id, name] of [[a, 'A'], [b, 'B']]) {
      await client.requirementCreateDiscussionMessage({
        id, discussion_id, if_match: '1',
        data: { actor: 'agent', role: 'assistant', body: `Reply ${name}.` },
      });
    }
  });
  await withHost(client => client.requirementUpdateDiscussion({
    id, discussion_id: a, if_match: '2', data: { actor: 'reviewer', status: 'resolved' },
  }));
  await withHost(async client => {
    const listed = await client.requirementListDiscussions({ id });
    const statuses = Object.fromEntries(listed.data.items.map(
      (item: any) => [item.discussion.discussion_id, item.discussion.status],
    ));
    assert.deepEqual(statuses, { [a]: 'resolved', [b]: 'active' });
    for (const [discussion_id, name, status] of [[a, 'A', 'resolved'], [b, 'B', 'active']]) {
      const read = await client.requirementGetDiscussion({ id, discussion_id });
      assert.equal(read.data.discussion.discussion_id, discussion_id);
      assert.equal(read.data.discussion.status, status);
      const messages = await client.requirementListDiscussionMessages({ id, discussion_id });
      assert.deepEqual(
        messages.data.items.map((message: any) => message.body),
        [`Concern ${name}.`, `Reply ${name}.`],
      );
    }
  });
}

// @provenance verification: examples
// @provenance rule: rule_revised_item_requires_new_review
// @provenance rule: rule_porcelain_review_returns_current_state
async function revisionAfterRejectionKeepsFeedbackAndWaitsForDecision(): Promise<void> {
  const id = 'req_reviewed';
  const first = await withHost(async client => (await createRequirement(client, id)).decision.pending.proposal_id);
  const feedback = await withHost(async client => {
    const decided = await client.decideRequirementReview({ id, proposal_id: first, data: {
      actor: reviewer, decision: 'rejected', rationale: 'Name the scope.',
      feedback: { role: 'user', body: 'Name the scope.' },
    } });
    return decided.data.feedback_message_id;
  });
  assert.ok(feedback, 'the rejection publishes its feedback');
  const second = await withHost(async client => {
    const read = await client.getRequirement({ id });
    const revised = await client.updateRequirement({
      id, if_match: read.data.edit.etag,
      data: { actor: 'agent', description: 'The scope is the review host.' },
    });
    return revised.data.decision.pending.proposal_id;
  });
  assert.notEqual(second, first);
  await withHost(async client => {
    const { data } = await client.getRequirement({ id });
    assert.equal(data.decision.pending.proposal_id, second);
    assert.equal(data.decision.pending.revision, data.decision.current_revision);
    assert.equal(data.decision.current_acceptance, null);
    assert.equal(data.decision.decisions.length, 1);
    assert.equal(data.decision.decisions[0].disposition.proposal_id, first);
    assert.equal(data.decision.decisions[0].disposition.decision, 'rejected');
    assert.equal(data.decision.decisions[0].feedback_message_id, feedback);
    const listed = await client.requirementListDiscussions({ id });
    assert.equal(listed.data.items.length, 1);
    const messages = await client.requirementListDiscussionMessages({
      id, discussion_id: listed.data.items[0].discussion.discussion_id,
    });
    assert.deepEqual(
      messages.data.items.map((message: any) => [message.id, message.body]),
      [[feedback, 'Name the scope.']],
    );
  });
}

// Implementation aid: no Rule states the generated Promise client's behaviour when a
// write response is lost. The Effect SDK Rule rule_effect_sdk_retains_unknown_writes
// does not cover this client. This check keeps the client from sending a write again.
async function lostWriteResponseIsReportedAndNotResent(): Promise<void> {
  const id = 'req_lost';
  await withHost(client => createRequirement(client, id));
  const target = `/requirements/${id}/discussions`;
  const sent: string[] = [];
  const losing: typeof fetch = async (input, init) => {
    const url = new URL(input instanceof Request ? input.url : input);
    if (init?.method === 'POST' && url.pathname === target) {
      sent.push(url.pathname);
      const response = await globalThis.fetch(input, init);
      // The host has written the Discussion; close the connection before the body arrives.
      await response.body?.cancel();
      throw new TypeError('The connection closed before the response arrived.');
    }
    return globalThis.fetch(input, init);
  };
  await withHost(async client => {
    await assert.rejects(client.requirementCreateDiscussion({
      id, data: { actor: 'reviewer', role: 'user', body: 'One concern.' },
    }), (error: any) => {
      assert.equal(error._tag, 'ConnectionError');
      return true;
    });
  }, losing);
  assert.deepEqual(sent, [target]);
  await withHost(async client => {
    const listed = await client.requirementListDiscussions({ id });
    assert.equal(listed.data.items.length, 1);
    const messages = await client.requirementListDiscussionMessages({
      id, discussion_id: listed.data.items[0].discussion.discussion_id,
    });
    assert.deepEqual(messages.data.items.map((message: any) => message.body), ['One concern.']);
  });
}

try {
  await copyFile(resolve(binaryArg), binary);
  await chmod(binary, 0o755);
  await mkdir(steAssets);
  await copyFile(
    join(root, 'packages/provenance/test/fixtures/synthetic-ste-dictionary.pdf'),
    join(steAssets, 'ASD-STE100_ISSUE9.pdf'),
  );
  execFileSync(binary, [
    'init', '--path', repository, '--scope', 'default', '--path-prefix', '.',
    '--disposition-actor-id', 'reviewer',
  ], { cwd: work, encoding: 'utf8', timeout: 180_000, env });
  await twoDiscussionsKeepTheirRepliesAndStatus();
  await revisionAfterRejectionKeepsFeedbackAndWaitsForDecision();
  await lostWriteResponseIsReportedAndNotResent();
  console.log(`Verified review and Discussion writes across host restarts: ${binaryArg}`);
} finally {
  await rm(work, { recursive: true, force: true });
}
