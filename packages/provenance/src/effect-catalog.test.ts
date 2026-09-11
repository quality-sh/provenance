import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import * as Effect from 'effect/Effect';
import * as Schema from 'effect/Schema';
import * as sdk from './effect.js';
import { verifies } from './rules.js';

type Node = {
  $ref?: string; const?: unknown; enum?: unknown[]; anyOf?: Node[]; oneOf?: Node[];
  allOf?: Node[]; type?: string | string[]; properties?: Record<string, Node>;
  required?: string[]; items?: Node; minItems?: number; minimum?: number;
  format?: string; default?: unknown;
};
type Operation = { operationId: string; requestBody: { content: { 'application/json': { schema: Node } } };
  responses: Record<string, { content: { 'application/json': { schema: Node } } }> };
const document = JSON.parse(readFileSync(new URL('../../../contracts/operations/openapi.json', import.meta.url), 'utf8')) as {
  paths: Record<string, { post?: Operation }>;
  components: { schemas: Record<string, Node> };
};
function value(node: Node): unknown {
  if (node.$ref) return value(document.components.schemas[node.$ref.split('/').at(-1)!]);
  if ('const' in node) return node.const;
  if (node.enum) return node.enum[0];
  if (node.anyOf || node.oneOf) return value((node.anyOf ?? node.oneOf)![0]);
  if (node.allOf) return Object.assign({}, ...node.allOf.map(value));
  const type = Array.isArray(node.type) ? node.type[0] : node.type;
  switch (type) {
    case 'null': return null;
    case 'boolean': return false;
    case 'number': case 'integer': return node.minimum ?? 0;
    case 'array': return Array.from({ length: node.minItems ?? 0 }, () => value(node.items!));
    case 'object': return Object.fromEntries((node.required ?? []).map(key => [key, value(node.properties![key])]));
    case 'string': return node.format === 'date-time' ? '2026-09-11T00:00:00Z' : node.format === 'uuid' ? '00000000-0000-4000-8000-000000000000' : 'fixture';
    default: throw new Error(`Unsupported fixture node: ${JSON.stringify(node)}`);
  }
}

test('every catalog method is lazy and preserves its generated request, success, and failure family', async () => {
  verifies('rule_sdk_bindings_derive_from_openapi', 'conformance');
  const schemas = sdk as unknown as Record<string, Schema.Codec<unknown>>;
  for (const [path, route] of Object.entries(document.paths)) {
    if (!route.post) continue;
    const op = route.post;
    const input = value(op.requestBody.content['application/json'].schema);
    const success = value(op.responses['200'].content['application/json'].schema);
    const failure = value(op.responses['400'].content['application/json'].schema);
    let posts = 0;
    const client = await Effect.runPromise(sdk.EffectHttpClient.connect({ baseUrl: 'https://example.test', fetch: async (url, init) => {
      if (init?.method !== 'POST') return Response.json({ engine_version: 'test', protocol_version: sdk.PROTOCOL_VERSION });
      posts++;
      assert.equal(url, `https://example.test${path}`);
      assert.deepEqual(JSON.parse(String(init.body)), input);
      return Response.json(posts === 1 ? success : failure, { status: posts === 1 ? 200 : 400 });
    } }));
    const method = (client as unknown as Record<string, (call: unknown) => Effect.Effect<unknown, sdk.ClientFailure>>)[op.operationId].bind(client);
    const effect = method(input);
    assert.equal(posts, 0, op.operationId);
    assert.deepEqual(await Effect.runPromise(effect), success, op.operationId);
    const error = await Effect.runPromise(Effect.flip(method(input)));
    assert.equal(error._tag, 'OperationError', op.operationId);
    if (error._tag === 'OperationError') assert.deepEqual(error.failure, failure);
    assert.equal(posts, 2);
    assert.deepEqual(client.unresolvedWrites(), []);
    for (const [schemaNode, fixture] of [[op.requestBody.content['application/json'].schema, input], [op.responses['200'].content['application/json'].schema, success], [op.responses['400'].content['application/json'].schema, failure]] as const) {
      const schema = schemas[schemaNode.$ref!.split('/').at(-1)!];
      assert.deepEqual(Schema.decodeUnknownSync(schema)(fixture), fixture, op.operationId);
    }
  }
});
