// Release-check aid: this flow runs the real page bootstrap against a running host, with
// Node's fetch and WebSocket in place of the browser.
import assert from 'node:assert/strict';
import { execFileSync, spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createInterface } from 'node:readline';
import { bootstrapReviewPage, EVENT_PROTOCOL, SESSION_KEY, type BrowserElement } from './bootstrap.ts';
import type { EventSocket } from './events.ts';
import { CONNECTION_REFUSED, type ReviewMountOptions } from './session.ts';

const [binaryArg] = process.argv.slice(2);
if (!binaryArg) throw new Error('Usage: node verify-page-session.ts BINARY');
const binary = resolve(binaryArg);
const work = await mkdtemp(join(tmpdir(), 'provenance-page-session-'));
const sockets: WebSocket[] = [];
let host: ReturnType<typeof spawn> | undefined;

async function until(condition: () => boolean, what: string) {
  const deadline = Date.now() + 15_000;
  while (!condition()) {
    if (Date.now() > deadline) throw new Error(`Timed out: ${what}`);
    await new Promise(resolve => setTimeout(resolve, 25));
  }
}

function fakeStorage() {
  const items = new Map<string, string>();
  return {
    items,
    getItem: (key: string) => items.get(key) ?? null,
    setItem: (key: string, value: string) => { items.set(key, value); },
    removeItem: (key: string) => { items.delete(key); },
  };
}

/** Runs the page once on a fake document; the network is real. */
function page(endpoint: string, hash: string, storage: ReturnType<typeof fakeStorage>) {
  const elements = new Map(['root', 'selection', 'requirement', 'status'].map(id => [id, {
    hidden: false, textContent: null, value: '', addEventListener() {}, focus() {},
  } as BrowserElement]));
  const mounted: ReviewMountOptions[] = [];
  const frames: { type: string }[] = [];
  const offered: string[][] = [];
  bootstrapReviewPage({
    document: { getElementById: id => elements.get(id) },
    location: { origin: endpoint, pathname: '/', search: '?root=req_root', hash },
    history: { replaceState() {} },
    fetch: (path, init) => fetch(`${endpoint}${path}`, { ...init, signal: AbortSignal.timeout(10_000) }),
    storage,
    mount(_root, options) {
      mounted.push(options);
      return { dispose() {}, refresh() {}, showRecord: async () => {} };
    },
    openEvents: session => {
      const protocols = [EVENT_PROTOCOL, session];
      offered.push(protocols);
      // A browser always sends its Origin; Node's WebSocket needs it named.
      const socket = new WebSocket(`${endpoint.replace(/^http/, 'ws')}/review-events`,
        { protocols, headers: { Origin: endpoint } } as unknown as string[]);
      sockets.push(socket);
      const events: EventSocket = {
        onmessage: null, onclose: null,
        send: text => socket.send(text),
        close: () => socket.close(),
      };
      socket.onmessage = event => {
        frames.push(JSON.parse(String(event.data)));
        events.onmessage?.(event);
      };
      socket.onclose = () => events.onclose?.();
      return events;
    },
    onWake() {},
  });
  return { mounted, frames, offered, status: elements.get('status')!, protocol: () => sockets.at(-1)?.protocol };
}

try {
  const repository = join(work, 'repository');
  const env = { ...process.env, XDG_CACHE_HOME: join(work, 'cache') };
  execFileSync(binary, ['init', '--path', repository, '--scope', 'default', '--path-prefix', '.'], {
    cwd: work, encoding: 'utf8', timeout: 180_000, env,
  });
  host = spawn(binary, ['review', '--repo', repository, '--repository-id', 'fixture', '--scope', 'default', '--no-open'], {
    cwd: work, env, stdio: ['ignore', 'pipe', 'inherit'],
  });
  const exited = once(host, 'exit');
  const lines = createInterface({ input: host.stdout! });
  const [line] = await Promise.race([
    once(lines, 'line', { signal: AbortSignal.timeout(30_000) }),
    exited.then(() => { throw new Error('Review host exited before startup'); }),
  ]);
  lines.close();
  const config = JSON.parse(line);
  const launchKey = await readFile(
    join(env.XDG_CACHE_HOME, 'provenance', 'review-launch', `${config.instanceNonce}.key`), 'utf8');
  const issued = await fetch(`${config.endpoint}/review-launch/code`, {
    method: 'POST', body: JSON.stringify({ launchKey }), signal: AbortSignal.timeout(10_000),
  });
  assert.equal(issued.status, 200);
  const { code } = await issued.json() as { code: string };
  const storage = fakeStorage();

  const linked = page(config.endpoint, `#launch=${code}`, storage);
  await until(() => linked.frames.some(frame => frame.type === 'ready'), 'ready frame after a launch link');
  const session = storage.items.get(SESSION_KEY);
  assert.ok(session?.startsWith(`${config.instanceNonce}.`), 'the page stores its session');
  assert.notEqual(session, config.bearer);
  assert.equal(linked.mounted.length, 1);
  assert.equal(linked.mounted[0].bearer, session);
  assert.equal(linked.mounted[0].rootId, 'req_root');
  assert.deepEqual(linked.offered, [[EVENT_PROTOCOL, session]]);
  assert.equal(linked.protocol(), EVENT_PROTOCOL);

  const reloaded = page(config.endpoint, '', storage);
  await until(() => reloaded.frames.some(frame => frame.type === 'ready'), 'ready frame after a reload');
  assert.equal(reloaded.mounted.length, 1);
  assert.equal(reloaded.mounted[0].bearer, session);
  assert.equal(storage.items.get(SESSION_KEY), session);

  const changed = `${session!.slice(0, -1)}${session!.endsWith('0') ? '1' : '0'}`;
  storage.setItem(SESSION_KEY, changed);
  const refused = page(config.endpoint, '', storage);
  await until(() => refused.status.textContent === CONNECTION_REFUSED, 'refusal of a changed session');
  assert.equal(refused.mounted.length, 0);
  assert.equal(refused.offered.length, 0);
  assert.equal(storage.items.has(SESSION_KEY), false);

  for (const socket of sockets) socket.close();
  host.kill('SIGTERM');
  await exited;
  console.log(`Verified the page session through a launch link, a reload, and a refusal: ${binaryArg}`);
} finally {
  for (const socket of sockets) socket.close();
  if (host && host.exitCode === null && host.signalCode === null) {
    const exited = once(host, 'exit');
    host.kill();
    await exited;
  }
  await rm(work, { recursive: true, force: true });
}
