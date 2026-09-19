import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { execFileSync, spawn } from 'node:child_process';
import { once } from 'node:events';
import { copyFile, mkdir, mkdtemp, readFile, rm, chmod } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { createInterface } from 'node:readline';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const [binaryArg, assetsArg, sdkArg] = process.argv.slice(2);
if (!binaryArg || !assetsArg || !sdkArg) throw new Error('Usage: node verify-native.ts BINARY COMPOSED_ASSETS SDK_PACKAGE');
const info = JSON.parse(await readFile(join(resolve(assetsArg), 'host-build-info.json'), 'utf8'));
const { HttpClient } = await import(pathToFileURL(join(resolve(sdkArg), 'dist/client.js')).href);
const work = await mkdtemp(join(tmpdir(), 'provenance-native-review-'));
const binary = join(work, process.platform === 'win32' ? 'provenance.exe' : 'provenance');
let host: ReturnType<typeof spawn> | undefined;
try {
  await copyFile(resolve(binaryArg), binary);
  await chmod(binary, 0o755);
  const repository = join(work, 'repository');
  const steAssets = join(work, 'ste-assets');
  await mkdir(steAssets);
  await copyFile(
    join(root, 'packages/provenance/test/fixtures/synthetic-ste-dictionary.pdf'),
    join(steAssets, 'ASD-STE100_ISSUE9.pdf'),
  );
  const fixtureEnv = {
    ...process.env,
    PROVENANCE_STE100_ASSET_DIR: steAssets,
    PROVENANCE_STE100_INDEX_DIR: join(work, 'ste-indexes'),
  };
  const cli = (...args: string[]) => execFileSync(binary, args, {
    cwd: work, encoding: 'utf8', timeout: 180_000, env: fixtureEnv,
  });
  cli('init', '--path', repository, '--scope', 'default', '--path-prefix', '.');
  for (const [id, parent] of [['req_root', undefined], ['req_child', 'req_root']]) {
    execFileSync(binary, [
      'requirements', 'create', '--repo', repository, '--scope', 'default',
      '--idempotency-key', `request_${id}`, '--stdin',
    ], {
      cwd: work, encoding: 'utf8', timeout: 30_000,
      input: JSON.stringify({
        actor: 'review-test', id, statement: 'The record is readable.', status: 'discovery',
        depends_on: [], supersedes: [], ...(parent ? { refines: parent } : {}),
      }),
    });
  }
  host = spawn(binary, ['review', '--repo', repository, '--repository-id', 'fixture', '--scope', 'default'], {
    cwd: work, env: { ...process.env, PATH: work }, stdio: ['ignore', 'pipe', 'inherit'],
  });
  const exited = once(host, 'exit');
  const lines = createInterface({ input: host.stdout! });
  const [line] = await Promise.race([
    once(lines, 'line', { signal: AbortSignal.timeout(30_000) }),
    exited.then(() => { throw new Error('Review host exited before startup'); }),
  ]);
  lines.close();
  const config = JSON.parse(line);
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
  host.kill('SIGTERM');
  await exited;
  console.log(`Verified ${Object.keys(info.files).length} embedded files, SDK cursor reads, search, and access checks: ${binaryArg}`);
} finally {
  if (host && host.exitCode === null && host.signalCode === null) {
    const exited = once(host, 'exit');
    host.kill();
    await exited;
  }
  await rm(work, { recursive: true, force: true });
}
