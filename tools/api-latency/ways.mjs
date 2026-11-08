// The three ways in. Each way sends one request and gives the result and the
// wall-clock time from the send to the complete answer.
//
// A request is { method, path, query, headers, body }. `path` is a concrete
// path. A result is { ok, status, data, error, ms }.
import { spawn } from 'node:child_process';
import { mkdtemp, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createInterface } from 'node:readline';

const now = () => Number(process.hrtime.bigint()) / 1e6;

function parseJson(text) {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

function envelopeResult(envelope, ok, status, ms) {
  if (envelope && 'data' in envelope && ok) {
    return { ok: true, status, data: envelope.data, meta: envelope.meta, ms };
  }
  return { ok: false, status, error: envelope?.error ?? envelope, ms };
}

/** Run a child process to the end. Gives exit code, stdout, stderr and time. */
export function run(command, args, { cwd, input, env } = {}) {
  return new Promise(resolve => {
    const start = now();
    const child = spawn(command, args, { cwd, env: env ?? process.env, stdio: [input === undefined ? 'ignore' : 'pipe', 'pipe', 'pipe'] });
    const out = [];
    const err = [];
    child.stdout.on('data', chunk => out.push(chunk));
    child.stderr.on('data', chunk => err.push(chunk));
    child.on('error', error => resolve({ code: -1, stdout: '', stderr: String(error), ms: now() - start }));
    child.on('close', code => {
      resolve({ code, stdout: Buffer.concat(out).toString(), stderr: Buffer.concat(err).toString(), ms: now() - start });
    });
    if (child.stdin) {
      // A child that exits before it reads its input closes the pipe; that is not a failure of the run.
      child.stdin.on('error', () => {});
      child.stdin.end(input);
    }
  });
}

/** Wait for the first line on a child's stdout. */
function firstLine(child, timeoutMs) {
  return new Promise((resolve, reject) => {
    const lines = createInterface({ input: child.stdout });
    const timer = setTimeout(() => reject(new Error('no startup line before the time limit')), timeoutMs);
    lines.once('line', line => {
      clearTimeout(timer);
      resolve(line);
    });
    child.once('exit', code => {
      clearTimeout(timer);
      reject(new Error(`the process stopped with code ${code} before its startup line`));
    });
  });
}

function stopChild(child) {
  return new Promise(resolve => {
    if (!child || child.exitCode !== null) {
      resolve();
      return;
    }
    const timer = setTimeout(() => child.kill('SIGKILL'), 10_000);
    child.once('exit', () => {
      clearTimeout(timer);
      resolve();
    });
    child.kill('SIGTERM');
  });
}

/** HTTP to one long-running `provenance review` host. */
export class HttpWay {
  name = 'HTTP';

  constructor({ bin, repo }) {
    this.bin = bin;
    this.repo = repo;
  }

  async start() {
    const startedAt = now();
    this.child = spawn(this.bin, ['review', '--repo', this.repo, '--repository-id', 'bench', '--scope', 'default', '--no-open'], {
      cwd: this.repo,
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    this.stderr = [];
    this.child.stderr.on('data', chunk => this.stderr.push(chunk));
    const line = await firstLine(this.child, 60_000).catch(error => {
      throw new Error(`${error.message}: ${Buffer.concat(this.stderr).toString().slice(-2000)}`);
    });
    const startup = JSON.parse(line);
    this.endpoint = startup.endpoint;
    this.bearer = startup.bearer;
    return now() - startedAt;
  }

  async call({ method, path, query = {}, headers = {}, body = null }) {
    const url = new URL(path, this.endpoint);
    for (const [name, value] of Object.entries(query)) {
      url.searchParams.append(name, String(value));
    }
    const init = { method: method.toUpperCase(), headers: { Authorization: `Bearer ${this.bearer}`, ...headers } };
    if (body !== null) {
      init.headers['Content-Type'] = 'application/json';
      init.body = JSON.stringify({ data: body });
    }
    const start = now();
    let response;
    let text;
    try {
      response = await fetch(url, init);
      text = await response.text();
    } catch (error) {
      return { ok: false, status: 0, error: { kind: 'transport', message: String(error.cause ?? error) }, ms: now() - start };
    }
    const ms = now() - start;
    const result = envelopeResult(parseJson(text), response.ok, response.status, ms);
    result.etag = response.headers.get('etag') ?? undefined;
    return result;
  }

  async stop() {
    await stopChild(this.child);
  }
}

/** One `provenance` process for each call. */
export class CliWay {
  name = 'CLI';

  constructor({ bin, repo }) {
    this.bin = bin;
    this.repo = repo;
  }

  async start() {
    this.scratch = await mkdtemp(join(tmpdir(), 'api-latency-cli-'));
    this.count = 0;
    return 0;
  }

  /** The argument list of `provenance api` for one request. */
  static apiArguments({ method, path, query = {}, headers = {} }, inputFile) {
    const args = ['api', path, '--format', 'json', '--method', method.toUpperCase()];
    for (const [name, value] of Object.entries(query)) {
      args.push('--query', `${name}=${value}`);
    }
    for (const [name, value] of Object.entries(headers)) {
      args.push('--header', `${name}: ${value}`);
    }
    if (inputFile) {
      args.push('--input', inputFile);
    }
    return args;
  }

  async call(request) {
    let inputFile = null;
    if (request.body !== null && request.body !== undefined) {
      inputFile = join(this.scratch, `body-${this.count++}.json`);
      await writeFile(inputFile, JSON.stringify(request.body));
    }
    return this.command(CliWay.apiArguments(request, inputFile));
  }

  /** Run one CLI command in the repository. Output is JSON or plain text. */
  async command(args) {
    const result = await run(this.bin, args, { cwd: this.repo });
    const stdout = parseJson(result.stdout);
    if (result.code === 0) {
      return { ok: true, status: 0, data: stdout?.data ?? stdout ?? result.stdout, ms: result.ms };
    }
    const error = parseJson(result.stderr.replace(/^Error: /, '')) ?? { message: result.stderr.trim().slice(0, 400) };
    return { ok: false, status: result.code, error: error.error ?? error, ms: result.ms };
  }

  async stop() {}
}

/** MCP tool calls over stdio to one running `provenance mcp` process. */
export class McpWay {
  name = 'MCP';

  constructor({ bin, repo }) {
    this.bin = bin;
    this.repo = repo;
  }

  async start() {
    const startedAt = now();
    this.child = spawn(this.bin, ['mcp', '--repo', this.repo], { cwd: this.repo, stdio: ['pipe', 'pipe', 'pipe'] });
    this.child.stderr.resume();
    this.child.stdin.on('error', () => {});
    this.pending = new Map();
    this.nextId = 1;
    this.lines = createInterface({ input: this.child.stdout });
    this.lines.on('line', line => {
      const message = parseJson(line);
      const waiter = message && this.pending.get(message.id);
      if (waiter) {
        this.pending.delete(message.id);
        waiter(message);
      }
    });
    this.child.once('exit', () => {
      for (const waiter of this.pending.values()) {
        waiter({ error: { message: 'the MCP process stopped' } });
      }
      this.pending.clear();
    });
    await this.rpc('initialize', {
      protocolVersion: '2025-06-18',
      capabilities: {},
      clientInfo: { name: 'api-latency', version: '0' },
    });
    this.child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized' })}\n`);
    return now() - startedAt;
  }

  rpc(method, params) {
    const id = this.nextId++;
    return new Promise(resolve => {
      this.pending.set(id, resolve);
      this.child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', id, method, params })}\n`);
    });
  }

  async tools() {
    const answer = await this.rpc('tools/list', {});
    return answer.result?.tools ?? [];
  }

  /** Call one tool. The result is the tool's JSON text, parsed. */
  async tool(name, args) {
    const start = now();
    const answer = await this.rpc('tools/call', { name, arguments: args });
    const ms = now() - start;
    if (answer.error) {
      return { ok: false, status: 0, error: answer.error, ms };
    }
    const text = (answer.result?.content ?? []).map(item => item.text ?? '').join('');
    const value = parseJson(text) ?? answer.result?.structuredContent ?? text;
    if (answer.result?.isError) {
      return { ok: false, status: 0, error: value?.error ?? value, ms };
    }
    return { ok: true, status: 0, data: value?.data ?? value, meta: value?.meta, ms };
  }

  async call({ method, path, query = {}, headers = {}, body = null }) {
    const args = { path, method: method.toUpperCase(), query: stringValues(query), header: headers };
    if (body !== null && body !== undefined) {
      args.input = body;
    }
    return this.tool('api', args);
  }

  async stop() {
    this.child?.stdin.end();
    await stopChild(this.child);
  }
}

function stringValues(object) {
  return Object.fromEntries(Object.entries(object).map(([name, value]) => [name, String(value)]));
}
