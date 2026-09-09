import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { typescriptClient, rustClientFiles } from './templates.mjs';

test('each method preserves its own failure family in both clients', async () => {
  const document = JSON.parse(await readFile(new URL('../../contracts/operations/openapi.json', import.meta.url), 'utf8'));
  const original = Object.values(document.paths).find(route => route.post).post;
  const second = structuredClone(original);
  second.operationId = 'readGraph';
  second.responses['400'].content['application/json'].schema.$ref = '#/components/schemas/GraphFailure';
  document.paths['/v7/operations/read-graph'] = { post: second };
  const ts = typescriptClient(document);
  const rust = Object.values(rustClientFiles(document)).join('\n');
  assert.match(ts, /export type OperationFailure =[^;]*GraphFailure/s);
  assert.match(rust, /pub enum OperationFailure[\s\S]*ReadGraph\(.*GraphFailure/);
  assert.match(rust, /let failure = OperationFailure::ReadGraph/);
});
