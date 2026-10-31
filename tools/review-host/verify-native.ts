// Release-check aid: this flow checks the native bundle and SDK against a running host.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { withFixtureHost } from './fixture-host.ts';

const [binaryArg, assetsArg, sdkArg] = process.argv.slice(2);
if (!binaryArg || !assetsArg || !sdkArg) throw new Error('Usage: node verify-native.ts BINARY COMPOSED_ASSETS SDK_PACKAGE');
const info = JSON.parse(await readFile(join(resolve(assetsArg), 'host-build-info.json'), 'utf8'));
const { HttpClient } = await import(pathToFileURL(join(resolve(sdkArg), 'dist/client.js')).href);
await withFixtureHost(binaryArg, repository => {
  for (const [id, parent] of [['req_root', undefined], ['req_child', 'req_root']]) {
    repository.create('requirements', {
      actor: 'review-test', id, statement: 'The record is readable.', status: 'discovery',
      depends_on: [], supersedes: [], ...(parent ? { refines: [parent] } : {}),
    });
  }
}, async config => {
  const get = (path: string, headers = {}) => fetch(`${config.endpoint}${path}`, { headers, signal: AbortSignal.timeout(10_000) });
  for (const [file, expected] of Object.entries(info.files)) {
    const response = await get(`/${file}`);
    assert.equal(response.status, 200, file);
    const bytes = new Uint8Array(await response.arrayBuffer());
    assert.equal(createHash('sha256').update(bytes).digest('hex'), expected, file);
  }
  assert.deepEqual(await (await get('/host-build-info.json')).json(), info);
  assert.equal(await (await get('/')).text(), await readFile(join(resolve(assetsArg), 'index.html'), 'utf8'));
  assert.equal((await get('/review-config')).status, 401);
  assert.equal((await get('/host.js', { Origin: 'https://unrelated.test' })).status, 403);
  const client = await HttpClient.connectWithBearer(
    config.endpoint, config.bearer, globalThis.fetch,
    { repository: 'fixture', scope: 'default' },
  );
  const first = await client.getRequirementDocument({ id: 'req_root', limit: 1 });
  assert.equal(first.data.entries.length, 1);
  assert.ok(first.meta.next_cursor, 'document has a continuation');
  const next = await client.getRequirementDocument({
    id: 'req_root', limit: 1, cursor: first.meta.next_cursor,
  });
  assert.equal(next.data.entries.length, 1);
  assert.equal(next.meta.next_cursor, null);
  const search = await client.listRequirements({ query: 'search', text: 'readable', limit: 1 });
  assert.equal(search.data.items.length, 1);
  assert.ok(search.meta.next_cursor, 'search has a continuation');
  await assert.rejects(HttpClient.connectWithBearer(
    config.endpoint, config.bearer, globalThis.fetch, { repository: 'other', scope: 'default' },
  ));
  await assert.rejects(HttpClient.connectWithBearer(
    config.endpoint, config.bearer, globalThis.fetch, { repository: 'fixture', scope: 'other' },
  ));
});
console.log(`Verified ${Object.keys(info.files).length} embedded files, SDK cursor reads, search, and access checks: ${binaryArg}`);
