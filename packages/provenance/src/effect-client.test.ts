import test from 'node:test';
import assert from 'node:assert/strict';
import * as Effect from 'effect/Effect';
import * as Fiber from 'effect/Fiber';
import * as Exit from 'effect/Exit';
import { EffectHttpClient, PROTOCOL_VERSION, ProvenanceClient } from './effect.js';
import { verifies } from './rules.js';

const metadata = () => Response.json({ engine_version: 'test', protocol_version: PROTOCOL_VERSION });
const call = { request: { statement: 'Stop.' } };
const write = { context: { repository: 'fixture', scope: 'default' }, request: { id: 'req_x', scope_id: 'default', statement: 'The record retains its fields.' } };

test('interrupting a read waits for body cancellation and releases the lock', async () => {
  verifies('rule_sdk_read_interruption_releases_resources', 'examples');
  let cancelled = false;
  let body!: ReadableStream<Uint8Array>;
  let started!: () => void;
  const reading = new Promise<void>(resolve => { started = resolve; });
  const client = await Effect.runPromise(EffectHttpClient.connect({ baseUrl: 'http://localhost', fetch: async (_url, init) => {
    if (init?.method !== 'POST') return metadata();
    body = new ReadableStream({ pull() { started(); }, cancel() { cancelled = true; } });
    return new Response(body);
  } }));
  const fiber = Effect.runFork(client.checkStatement(call));
  await reading;
  await Effect.runPromise(Fiber.interrupt(fiber));
  assert.equal(cancelled, true);
  assert.equal(body.locked, false);
  assert.deepEqual(client.unresolvedWrites(), []);
});

test('an interrupted mutation retains uncertainty without replay', async () => {
  verifies('rule_effect_sdk_retains_unknown_writes', 'examples');
  let started!: () => void;
  const dispatched = new Promise<void>(resolve => { started = resolve; });
  let posts = 0;
  const client = await Effect.runPromise(EffectHttpClient.connect({ baseUrl: 'http://localhost', fetch: async (_url, init) => {
    if (init?.method !== 'POST') return metadata();
    posts++; started();
    return new Response(new ReadableStream());
  } }));
  const fiber = Effect.runFork(client.updateRequirement(write));
  await dispatched;
  assert.equal(client.unresolvedWrites()[0].state, 'pending');
  await Effect.runPromise(Fiber.interrupt(fiber));
  assert.equal(posts, 1);
  const [outcome] = client.unresolvedWrites();
  assert.equal(outcome.operation, 'updateRequirement');
  assert.equal(outcome.state, 'uncertain');
  assert.ok(!JSON.stringify(outcome).includes('statement'));
  client.resolveWrite(outcome.id);
  assert.deepEqual(client.unresolvedWrites(), []);
});

test('protocol mismatch prevents operations through the service layer', async () => {
  let posts = 0;
  const effect = Effect.flatMap(ProvenanceClient, client => client.checkStatement(call)).pipe(Effect.provide(ProvenanceClient.layer({ baseUrl: 'http://localhost', fetch: async (_url, init) => {
    if (init?.method === 'POST') posts++;
    return Response.json({ engine_version: 'test', protocol_version: 0 });
  } })));
  const error = await Effect.runPromise(Effect.flip(effect));
  assert.equal(error.name, 'ProtocolMismatchError');
  assert.equal(posts, 0);
});

test('interruption waits until asynchronous response cancellation completes', async () => {
  let reading!: () => void;
  let cancelling!: () => void;
  let release!: () => void;
  const started = new Promise<void>(resolve => { reading = resolve; });
  const cancelled = new Promise<void>(resolve => { cancelling = resolve; });
  const cleanup = new Promise<void>(resolve => { release = resolve; });
  let body!: ReadableStream<Uint8Array>;
  const client = await Effect.runPromise(EffectHttpClient.connect({ baseUrl: 'http://localhost', fetch: async (_url, init) => {
    if (init?.method !== 'POST') return metadata();
    body = new ReadableStream({ pull() { reading(); }, cancel() { cancelling(); return cleanup; } });
    return new Response(body);
  } }));
  const fiber = Effect.runFork(client.checkStatement(call));
  await started;
  while (!body.locked) await Promise.resolve();
  let finished = false;
  const interrupt = Effect.runPromise(Fiber.interrupt(fiber)).then(() => { finished = true; });
  await cancelled;
  await new Promise(resolve => setTimeout(resolve, 10));
  try { assert.equal(finished, false); assert.equal(body.locked, true); }
  finally { release(); await interrupt; }
  assert.equal(body.locked, false);
});

test('concurrent writes stop at the retention limit without losing an unresolved outcome', async () => {
  let posts = 0;
  const client = await Effect.runPromise(EffectHttpClient.connect({ baseUrl: 'http://localhost', fetch: async (_url, init) => {
    if (init?.method !== 'POST') return metadata();
    posts++; throw new Error('Response lost');
  } }));
  const outcomes = await Effect.runPromise(Effect.all(Array.from({ length: 129 }, () => Effect.flip(client.updateRequirement(write))), { concurrency: 'unbounded' }));
  assert.equal(posts, 128);
  assert.equal(outcomes.filter(error => error._tag === 'WriteCapacityError').length, 1);
  assert.equal(client.unresolvedWrites().length, 128);
});


test('abandoning a validated mutation result retains uncertainty until the Effect receives it', async () => {
  verifies('rule_effect_sdk_retains_unknown_writes', 'examples');
  let interruptions = 0;
  let successes = 0;
  for (let delay = 0; delay < 24; delay++) {
    let dispatched!: () => void;
    const dispatch = new Promise<void>(resolve => { dispatched = resolve; });
    const client = await Effect.runPromise(EffectHttpClient.connect({ baseUrl: 'http://localhost', fetch: async (_url, init) => {
      if (init?.method !== 'POST') return metadata();
      dispatched();
      return Response.json({ schema_version: 2, scope_id: 'default', id: 'req_x', statement: 'The record retains its fields.', status: 'active' });
    } }));
    const fiber = Effect.runFork(client.updateRequirement(write));
    await dispatch;
    for (let step = 0; step < delay; step++) await Promise.resolve();
    await Effect.runPromise(Fiber.interrupt(fiber));
    const exit = await Effect.runPromise(Fiber.await(fiber));
    if (Exit.hasInterrupts(exit)) {
      interruptions++;
      assert.equal(client.unresolvedWrites().length, 1, `interrupted at microtask ${delay}`);
      assert.equal(client.unresolvedWrites()[0].state, 'uncertain');
    } else {
      successes++;
      assert.ok(Exit.isSuccess(exit));
      assert.deepEqual(client.unresolvedWrites(), []);
    }
  }
  assert.ok(interruptions > 0 && successes > 0);
});
