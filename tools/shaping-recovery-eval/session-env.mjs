import { spawnSync } from 'node:child_process';
import { chmodSync, existsSync, mkdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

// The CLI gives BROWSER the path of a local redirect page; the stub records the page it names.
const browserStub = `#!/bin/sh
url=$(grep -o 'http://127\\.0\\.0\\.1:[0-9]*/[^"<> ]*' "$1" | head -n 1)
printf '%s\\t%s\\n' "$(date +%s%3N)" "$url" >> "$EVAL_BROWSER_LOG"
`;

/** Gives one trial its own user cache and a browser that records each page the CLI opens. */
export function sessionEnv(dir) {
  const cache = join(dir, 'cache'), bin = join(dir, 'bin'), log = join(dir, 'browser.log');
  mkdirSync(cache, { recursive: true });
  mkdirSync(bin, { recursive: true });
  writeFileSync(join(bin, 'eval-browser'), browserStub);
  chmodSync(join(bin, 'eval-browser'), 0o755);
  if (!existsSync(log)) writeFileSync(log, '');
  const env = { ...process.env, XDG_CACHE_HOME: cache, BROWSER: join(bin, 'eval-browser'), EVAL_BROWSER_LOG: log,
    DISPLAY: process.env.DISPLAY || ':0' };
  for (const name of ['SSH_CONNECTION', 'SSH_CLIENT', 'SSH_TTY']) delete env[name];
  return { env, cache, log };
}

/** Pages opened after `offset` lines of the browser log, with the root and focus they show. */
export function openedPages(log, offset = 0) {
  const lines = readFileSync(log, 'utf8').split('\n').filter(Boolean);
  return lines.slice(offset).map(line => {
    const [at, raw] = line.split('\t');
    const url = new URL(raw.replaceAll('&amp;', '&'));
    url.hash = '';
    return { at: Number(at), url: url.toString(), root: url.searchParams.get('root'), focus: url.searchParams.get('focus') };
  });
}

export function browserLines(log) {
  return readFileSync(log, 'utf8').split('\n').filter(Boolean).length;
}

function hostPids(repo) {
  const result = spawnSync('pgrep', ['-f', `review --repo ${realpathSync(repo)}`], { encoding: 'utf8' });
  return result.stdout.split('\n').filter(Boolean).map(Number);
}

export function hostRunning(repo) {
  return hostPids(repo).length > 0;
}

/** Stops the review hosts that a trial started, so no trial leaves a listener behind. */
export function stopHosts(repo) {
  for (const pid of hostPids(repo)) {
    try { process.kill(pid, 'SIGTERM'); } catch { /* already stopped */ }
  }
}
