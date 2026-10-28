// Implementation aid: the pinned renderer ships its own SDK build, so this flow checks the
// host's read responses against the response validators that the renderer bundles.
import { execFileSync, spawn } from 'node:child_process';
import { once } from 'node:events';
import { chmod, copyFile, mkdir, mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { createInterface } from 'node:readline';
import { fileURLToPath, pathToFileURL } from 'node:url';

type Validator = ((body: unknown) => boolean) & { errors?: { instancePath: string; keyword: string; message?: string }[] | null };

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const [binaryArg, assetsArg, sdkArg] = process.argv.slice(2);
if (!binaryArg || !assetsArg || !sdkArg) throw new Error('Usage: node verify-renderer-reads.ts BINARY COMPOSED_ASSETS SDK_PACKAGE');
const assets = resolve(assetsArg);
const review = await readFile(join(assets, 'review.js'), 'utf8');
const { HttpClient } = await import(pathToFileURL(join(resolve(sdkArg), 'dist/client.js')).href);

// The renderer client loads the validators of each operation from a chunk with a hashed name.
// The chunk is found from the client method of the operation, so a new renderer build needs no edit.
async function rendererValidator(operation: string, name: string): Promise<Validator> {
  const start = review.indexOf(`async ${operation}(`);
  const chunk = start < 0 ? null : /import\(`(\.\/assets\/[^`]+\.js)`\)/.exec(review.slice(start, start + 400));
  if (!chunk) throw new Error(`The renderer client has no ${operation} read`);
  const validator = (await import(pathToFileURL(join(assets, chunk[1])).href))[name];
  if (typeof validator !== 'function') throw new Error(`The renderer ${operation} chunk has no ${name} validator`);
  return validator;
}

// The error names the nearest record id on the path to the field that failed.
function recordAt(body: unknown, path: string): string {
  let node = body as Record<string, unknown> | undefined;
  let id = '(no record)';
  for (const part of path.split('/').slice(1)) {
    if (node && typeof node === 'object' && typeof node.id === 'string') id = node.id;
    node = node?.[part.replaceAll('~1', '/').replaceAll('~0', '~')] as Record<string, unknown> | undefined;
  }
  return node && typeof node === 'object' && typeof node.id === 'string' ? node.id : id;
}

const work = await mkdtemp(join(tmpdir(), 'provenance-renderer-reads-'));
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
  execFileSync(binary, ['init', '--path', repository, '--scope', 'default', '--path-prefix', '.'], {
    cwd: work, encoding: 'utf8', timeout: 180_000, env: {
      ...process.env,
      PROVENANCE_STE100_ASSET_DIR: steAssets,
      PROVENANCE_STE100_INDEX_DIR: join(work, 'ste-indexes'),
    },
  });
  const records: [string, string[]][] = [
    ['req_root', []], ['req_child', ['req_root']], ['req_joint', ['req_root', 'req_child']],
  ];
  for (const [id, refines] of records) {
    execFileSync(binary, ['requirements', 'create', '--repo', repository, '--scope', 'default', '--stdin'], {
      cwd: work, encoding: 'utf8', timeout: 30_000,
      input: JSON.stringify({
        actor: 'review-test', id, statement: 'The record is readable.', status: 'discovery',
        depends_on: [], supersedes: [], ...(refines.length ? { refines } : {}),
      }),
    });
  }
  host = spawn(binary, ['review', '--repo', repository, '--repository-id', 'fixture', '--scope', 'default', '--no-open'], {
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
  let body: unknown;
  const recording: typeof fetch = async (input, init) => {
    const response = await fetch(input, init);
    body = await response.clone().json().catch(() => undefined);
    return response;
  };
  const client = await HttpClient.connectWithBearer(
    config.endpoint, config.bearer, recording, { repository: 'fixture', scope: 'default' },
  );
  const failures: string[] = [];
  const counts: Record<string, number> = {};
  // Reads every page of one read and checks each response body with the renderer validator.
  async function check(operation: string, name: string, label: string, read: (cursor?: string) => Promise<unknown>) {
    const validator = await rendererValidator(operation, name);
    let cursor: string | undefined;
    let page = 0;
    do {
      page += 1;
      await read(cursor);
      if (!validator(body)) {
        // A union reports an error for each branch that failed. Only the deepest fields are reported,
        // so the branch that matched the record type shows its field and the other branches add less noise.
        const messages = new Map<string, Set<string>>();
        for (const error of validator.errors ?? []) {
          if (error.keyword === 'anyOf' || error.keyword === 'oneOf') continue;
          messages.set(error.instancePath, (messages.get(error.instancePath) ?? new Set()).add(error.message ?? error.keyword));
        }
        for (const [path, texts] of messages) {
          if ([...messages.keys()].some(other => other.startsWith(`${path}/`))) continue;
          failures.push(`${operation} ${label} page ${page}: record ${recordAt(body, path)} field ${path}: ${[...texts].join('; ')}`);
        }
      }
      counts[operation] = (counts[operation] ?? 0) + 1;
      cursor = (body as { meta?: { next_cursor?: string | null } }).meta?.next_cursor ?? undefined;
    } while (cursor);
  }
  for (const [id] of records) {
    await check('getRequirementDocument', 'GetRequirementDocumentSuccess', id,
      cursor => client.getRequirementDocument({ id, limit: 1, ...(cursor ? { cursor } : {}) }));
    await check('getRequirement', 'GetRequirementBaseSuccess', id, () => client.getRequirement({ id }));
    await check('listRequirementHistory', 'ListRequirementHistorySuccess', id,
      cursor => client.listRequirementHistory({ id, limit: 1, ...(cursor ? { cursor } : {}) }));
  }
  await check('listRequirements', 'ListRequirementsBaseSuccess', 'list',
    cursor => client.listRequirements({ limit: 1, ...(cursor ? { cursor } : {}) }));
  await check('listRequirements', 'ListRequirementsSearchSuccess', 'search',
    cursor => client.listRequirements({ query: 'search', text: 'readable', limit: 1, ...(cursor ? { cursor } : {}) }));
  host.kill('SIGTERM');
  await exited;
  const summary = Object.entries(counts).map(([operation, count]) => `${operation} ${count}`).join(', ');
  if (failures.length) {
    for (const failure of failures) console.error(failure);
    throw new Error(`The renderer validators refused ${failures.length} host response fields (${summary} pages)`);
  }
  console.log(`The renderer validators accepted every host response (${summary} pages): ${binaryArg}`);
} finally {
  if (host && host.exitCode === null && host.signalCode === null) {
    const exited = once(host, 'exit');
    host.kill();
    await exited;
  }
  await rm(work, { recursive: true, force: true });
}
