import assert from 'node:assert/strict';
import test from 'node:test';
import * as Effect from 'effect/Effect';
import * as Cause from 'effect/Cause';
import * as Exit from 'effect/Exit';
import * as Fiber from 'effect/Fiber';
import { createServer, type RequestListener } from 'node:http';
import type { AddressInfo } from 'node:net';
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

test('an interrupted mutation is sent once and ends as an interruption', async () => {
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
  assert.ok(Exit.isFailure(exit) && Cause.hasInterruptsOnly(exit.cause));
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

test('metadata refusal remains a typed Effect error with status and payload', async () => {
  const failure = { error: { kind: 'unauthenticated' as const }, meta: {} };
  const error = await Effect.runPromise(Effect.flip(EffectHttpClient.connect({
    baseUrl: 'http://localhost',
    bearer: 'wrong-secret',
    fetch: async () => Response.json(failure, { status: 401 }),
  })));
  assert.equal(error._tag, 'OperationError');
  if (error._tag === 'OperationError') {
    assert.equal(error.status, 401);
    assert.deepEqual(error.failure, failure);
  }
});

test('malformed metadata refusal JSON remains a malformed Effect response', async () => {
  const error = await Effect.runPromise(Effect.flip(EffectHttpClient.connect({
    baseUrl: 'http://localhost',
    fetch: async () => new Response('{', { status: 401 }),
  })));
  assert.equal(error._tag, 'MalformedResponseError');
});

test('schema-invalid metadata refusal remains a malformed Effect response', async () => {
  const error = await Effect.runPromise(Effect.flip(EffectHttpClient.connect({
    baseUrl: 'http://localhost',
    fetch: async () => Response.json({ error: { kind: 'not_declared' }, meta: {} }, { status: 401 }),
  })));
  assert.equal(error._tag, 'MalformedResponseError');
});

test('metadata transport loss remains an Effect connection error', async () => {
  const error = await Effect.runPromise(Effect.flip(EffectHttpClient.connect({
    baseUrl: 'http://localhost',
    fetch: async () => { throw new TypeError('fixture unavailable'); },
  })));
  assert.equal(error._tag, 'ConnectionError');
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

async function listen(handler: RequestListener) {
  const server = createServer(handler);
  await new Promise<void>(resolve => server.listen(0, '127.0.0.1', resolve));
  const close = async () => {
    server.closeAllConnections();
    await new Promise<void>(resolve => server.close(() => resolve()));
  };
  return { baseUrl: `http://127.0.0.1:${(server.address() as AddressInfo).port}`, close };
}

async function hostWithLostResponses() {
  const received = { reads: 0, writes: 0 };
  const host = await listen((request, response) => {
    if (request.method === 'GET') {
      response.setHeader('content-type', 'application/json');
      response.end(JSON.stringify({ data: { compatibility: COMPATIBILITY, package: { name: 'provenance', version: 'test' }, repository: 'fixture', scope: 'default' }, meta: {} }));
      return;
    }
    if (request.url?.startsWith('/statement-checks')) received.reads++;
    else received.writes++;
    request.resume();
    request.on('end', () => request.socket.destroy());
  });
  const client = await Effect.runPromise(EffectHttpClient.connect({ baseUrl: host.baseUrl }));
  return { client, received, close: host.close };
}

test('a write whose response is lost fails with an outcome-unknown error and is sent once', async function lostWriteResponse() {
  verifies('rule_effect_sdk_retains_unknown_writes', 'examples');
  const host = await hostWithLostResponses();
  try {
    const error = await Effect.runPromise(Effect.flip(host.client.answerQuestion({ id: 'question_x', data: { answer: 'Done.' } })));
    assert.match(error.message, /Read the current state before you send the write again/);
    assert.equal(error._tag, 'OutcomeUnknownError');
    assert.equal(host.received.writes, 1);
  } finally { await host.close(); }
});

test('a write that cannot connect fails with a connection error and is not sent', async function unsentWrite() {
  verifies('rule_effect_sdk_retains_unknown_writes', 'examples');
  const host = await hostWithLostResponses();
  await host.close();
  const error = await Effect.runPromise(Effect.flip(host.client.answerQuestion({ id: 'question_x', data: { answer: 'Done.' } })));
  assert.equal(error._tag, 'ConnectionError');
  assert.equal(host.received.writes, 0);
});

test('a read whose response is lost fails with a connection error', async function lostReadResponse() {
  verifies('rule_effect_sdk_retains_unknown_writes', 'examples');
  const host = await hostWithLostResponses();
  try {
    const error = await Effect.runPromise(Effect.flip(host.client.checkStatement({ data: { statement: 'Stop.' } })));
    assert.equal(error._tag, 'ConnectionError');
    assert.equal(host.received.reads, 1);
  } finally { await host.close(); }
});
