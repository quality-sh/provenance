import assert from 'node:assert/strict';
import test from 'node:test';
import * as Effect from 'effect/Effect';
import * as Fiber from 'effect/Fiber';
import { COMPATIBILITY, EffectHttpClient } from './effect.js';
import { verifies } from './rules.js';

const metadata = (repository = 'fixture', scope = 'default') => Response.json({
  data: {
    compatibility: COMPATIBILITY,
    package: { name: 'provenance', version: 'test' },
    repository, scope,
  },
  meta: {},
});

test('interrupting a read cancels the response body and releases its lock', async () => {
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
  const fiber = Effect.runFork(client.checkStatement({ data: { statement: 'Stop.' } }));
  await reading;
  await Effect.runPromise(Fiber.interrupt(fiber));
  assert.equal(cancelled, true);
  assert.equal(body.locked, false);
});

test('an interrupted mutation is sent once and ends as a connection error', async () => {
  let started!: () => void;
  const dispatched = new Promise<void>(resolve => { started = resolve; });
  let posts = 0;
  const client = await Effect.runPromise(EffectHttpClient.connect({ baseUrl: 'http://localhost', fetch: async (_url, init) => {
    if (init?.method !== 'POST') return metadata();
    posts++;
    started();
    return new Response(new ReadableStream());
  } }));
  const fiber = Effect.runFork(client.answerQuestion({ id: 'question_x', data: { answer: 'Done.' } }));
  await dispatched;
  await Effect.runPromise(Fiber.interrupt(fiber));
  const exit = await Effect.runPromise(Fiber.await(fiber));
  assert.equal(posts, 1);
  assert.equal(exit._tag, 'Failure');
});

test('connection pins the complete compatibility tuple and bound identity', async () => {
  const incompatible = { ...COMPATIBILITY, read_derivation: COMPATIBILITY.read_derivation + 1 };
  const tupleError = await Effect.runPromise(Effect.flip(EffectHttpClient.connect({
    baseUrl: 'http://localhost',
    fetch: async () => Response.json({ ...await metadata().json(), data: { ...await metadata().json().then(value => value.data), compatibility: incompatible } }),
  })));
  assert.equal(tupleError._tag, 'ProtocolMismatchError');

  const identityError = await Effect.runPromise(Effect.flip(EffectHttpClient.connect({
    baseUrl: 'http://localhost', repository: 'requested', scope: 'default', fetch: async () => metadata('authorized'),
  })));
  assert.equal(identityError._tag, 'IdentityMismatchError');
});

test('a declared failure remains a typed operation error', async () => {
  const client = await Effect.runPromise(EffectHttpClient.connect({ baseUrl: 'http://localhost', fetch: async (_url, init) => {
    if (init?.method !== 'POST') return metadata();
    return Response.json({ error: { kind: 'access_denied' }, meta: {} }, { status: 403 });
  } }));
  const error = await Effect.runPromise(Effect.flip(client.checkStatement({ data: { statement: 'Stop.' } })));
  assert.equal(error._tag, 'OperationError');
  if (error._tag === 'OperationError') assert.equal(error.status, 403);
});
