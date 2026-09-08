import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import * as templates from './templates.mjs';

test('each Rust operation owns its method file as the catalog grows', async () => {
  assert.equal(typeof templates.rustClientFiles, 'function');
  const doc = JSON.parse(await readFile(new URL('../../contracts/operations/openapi.json', import.meta.url)));
  const route = structuredClone(Object.values(doc.paths).find(route => route.post));
  doc.paths = Object.fromEntries(Array.from({ length: 32 }, (_, i) => {
    const entry = structuredClone(route);
    entry.post.operationId = `operation${i}`;
    return [`/v7/operations/operation-${i}`, entry];
  }));
  const files = templates.rustClientFiles(doc);
  for (let i = 0; i < 32; i++) {
    assert.match(files[`operations/operation${i}.rs`], new RegExp(`pub async fn operation${i}\\(`));
  }
  assert.ok(!files['client.rs'].includes('pub async fn operation'));
  for (const [path, source] of Object.entries(files)) assert.ok(source.split('\n').length <= 500, path);
});
