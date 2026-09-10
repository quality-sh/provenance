import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { execFileSync, spawn } from 'node:child_process';
import { once } from 'node:events';
import { copyFile, mkdtemp, readFile, rm, chmod } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createInterface } from 'node:readline';
import { pathToFileURL } from 'node:url';

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
  const cli = (...args: string[]) => execFileSync(binary, args, { cwd: work, encoding: 'utf8', timeout: 30_000 });
  cli('init', '--path', repository, '--scope', 'default', '--path-prefix', '.');
  for (const [id, parent] of [['req_root', undefined], ['req_child', 'req_root']]) {
    cli('requirements', 'create', '--repo', repository, '--scope', 'default', '--id', id!,
      '--statement', 'The graph is readable.', ...(parent ? ['--refines', parent] : []));
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
  assert.match(await (await get('/')).text(), /src="\.\/host.js"/);
  assert.equal((await get('/review-config')).status, 401);
  assert.equal((await get('/host.js', { Origin: 'https://unrelated.test' })).status, 403);
  const client = await HttpClient.connectWithBearer(config.endpoint, config.bearer);
  const context = { repository: 'fixture', scope: 'default', freshness: 'catch_up' };
  const first = await client.readDocument({ context, request: { id: 'req_root', limit: 1 } });
  assert.equal(first.entries.length, 1);
  assert.ok(first.next_cursor, 'document has a continuation');
  const next = await client.readDocument({ context: { ...context, freshness: 'annotate_only' },
    request: { id: 'req_root', limit: 1, cursor: first.next_cursor } });
  assert.equal(next.entries.length, 1);
  assert.equal(next.next_cursor, null);
  const search = await client.search({ context, request: { text: 'readable', limit: 1 } });
  assert.equal(search.nodes.length, 1);
  assert.ok(search.next_cursor, 'search has a continuation');
  await assert.rejects(client.readDocument({ context: { ...context, repository: 'other' }, request: { id: 'req_root' } }));
  await assert.rejects(client.readDocument({ context: { ...context, scope: 'other' }, request: { id: 'req_root' } }));
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
