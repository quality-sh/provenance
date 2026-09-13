import test from 'node:test';
import assert from 'node:assert/strict';
import * as Effect from 'effect/Effect';
import * as Layer from 'effect/Layer';
import { Atom, AtomRegistry } from 'effect/unstable/reactivity';
import { EffectHttpClient, ProvenanceClient, PROTOCOL_VERSION, type ClientFailure } from './effect.js';

for (const outcome of ['stale', 'connection', 'uncertain'] as const) {
  test(`Atom.runtime preserves ${outcome} with the shared SDK service`, async () => {
    let posts = 0;
    const client = await Effect.runPromise(EffectHttpClient.connect({ baseUrl: 'http://localhost', fetch: async (_url, init) => {
      if (init?.method !== 'POST') return Response.json({ engine_version: 'test', protocol_version: PROTOCOL_VERSION });
      posts++;
      if (outcome === 'connection') throw new Error('private transport detail');
      if (outcome === 'uncertain') return new Response('private broken body');
      return Response.json({ protocol_version: PROTOCOL_VERSION, operation: 'get', error: {
        kind: 'stale', serial: 1, digest: 'digest', instance_id: 'instance', moved: [],
      } }, { status: 409 });
    } }));
    const runtime = Atom.runtime(Layer.succeed(ProvenanceClient, client));
    const context = { repository: 'fixture', scope: 'default' };
    const operation = Effect.flatMap(ProvenanceClient,  (sdk): Effect.Effect<unknown, ClientFailure> => outcome === 'uncertain'
      ? sdk.updateRequirement({ context, request: { id: 'req_x', scope_id: 'default', statement: 'The record retains its fields.' } })
      : sdk.get({ context, request: { id: 'req_x', node_type: 'requirement' } }));
    const atom = runtime.fn(() => operation);
    const registry = AtomRegistry.make();
    const release = registry.mount(atom);
    try {
      registry.set(atom, undefined);
      const error = await Effect.runPromise(Effect.flip(AtomRegistry.getResult(registry, atom, { suspendOnWaiting: true })));
      assert.equal(error.name, outcome === 'stale' ? 'OperationError' : outcome === 'connection' ? 'ConnectionError' : 'UncertainWriteError');
      if (error._tag === 'OperationError') assert.equal(error.failure.error.kind, 'stale');
      assert.equal(posts, 1);
      assert.equal(client.unresolvedWrites().length, outcome === 'uncertain' ? 1 : 0);
      assert.doesNotMatch(JSON.stringify(error), /private/);
    } finally { release(); registry.dispose(); }
  });
}

test('Atom.runtime refuses an incompatible host before mutation dispatch', async () => {
  let posts = 0;
  const runtime = Atom.runtime(ProvenanceClient.layer({ baseUrl: 'http://localhost', fetch: async (_url, init) => {
    if (init?.method === 'POST') posts++;
    return Response.json({ engine_version: 'test', protocol_version: 0 });
  } }));
  const atom = runtime.fn(() => Effect.flatMap(ProvenanceClient, client => client.updateRequirement({
    context: { repository: 'fixture', scope: 'default' }, request: { id: 'req_x', scope_id: 'default', statement: 'The record retains its fields.' },
  })));
  const registry = AtomRegistry.make();
  const release = registry.mount(atom);
  try {
    registry.set(atom, undefined);
    const error = await Effect.runPromise(Effect.flip(AtomRegistry.getResult(registry, atom, { suspendOnWaiting: true })));
    assert.equal(error._tag, 'ProtocolMismatchError');
    assert.equal(posts, 0);
  } finally { release(); registry.dispose(); }
});
