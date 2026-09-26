import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { typescriptClient, rustClientFiles } from './templates.mjs';

test('each method preserves its own failure family in both clients', async () => {
  const document = JSON.parse(await readFile(new URL('../../contracts/operations/openapi.json', import.meta.url), 'utf8'));
  const compatibility = JSON.parse(await readFile(new URL('../../contracts/operations/compatibility.json', import.meta.url), 'utf8'));
  const original = Object.values(document.paths).find(route => route.post).post;
  const second = structuredClone(original);
  second.operationId = 'readGraph';
  second.responses['400'].content['application/json'].schema.$ref = '#/components/schemas/CheckStatementFailure';
  document.paths['/fixtures/read-graph'] = { post: second };
  const ts = typescriptClient(document);
  const rust = Object.values(rustClientFiles(document, compatibility)).join('\n');
  assert.match(ts, /export type OperationFailure =[^;]*CheckStatementFailure/s);
  assert.match(rust, /pub enum OperationFailure[\s\S]*ReadGraph\(.*CheckStatementFailure/);
  assert.match(rust, /return Err\(Error::Operation \{[^}]*OperationFailure::ReadGraph/s);
});

test('metadata uses the declared failure contract in both clients', async () => {
  const document = JSON.parse(await readFile(new URL('../../contracts/operations/openapi.json', import.meta.url), 'utf8'));
  const compatibility = JSON.parse(await readFile(new URL('../../contracts/operations/compatibility.json', import.meta.url), 'utf8'));
  const ts = typescriptClient(document, compatibility);
  const rust = rustClientFiles(document, compatibility)['client.rs'];
  assert.match(ts, /export type OperationFailure =[^;]*MetadataFailure/s);
  assert.match(ts, /checked\(value, validate\.MetadataFailure, 'metadata', false\);[\s\S]*throw new OperationError<components\['schemas'\]\['MetadataFailure'\]>/);
  assert.match(rust, /Metadata\(Box<MetadataFailure>\)/);
  assert.match(rust, /runtime::validate\(&value, "MetadataFailure", "metadata", false\)\?;[\s\S]*OperationFailure::Metadata/);
});
