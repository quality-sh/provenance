import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { Readable } from "node:stream";
import { createInterface } from "node:readline";
import type { Subprocess } from "bun";

export const binary = process.env.PROVENANCE_TEST_BIN ?? join(import.meta.dir, "../../../target/debug/provenance");
const limit = 20000;
export function deadline<T>(promise: Promise<T>, label: string): Promise<T> {
  let timer: ReturnType<typeof setTimeout>;
  return Promise.race([promise, new Promise<T>((_, reject) => {
    timer = setTimeout(() => reject(new Error(`Timed out: ${label}`)), limit);
  })]).finally(() => clearTimeout(timer!));
}

export async function command(args: string[], env: Record<string, string | undefined>) {
  const child = Bun.spawn([binary, ...args], { env, stdout: "pipe", stderr: "pipe" });
  const [status, stdout, stderr] = await deadline(Promise.all([
    child.exited, new Response(child.stdout).text(), new Response(child.stderr).text(),
  ]), args.join(" "));
  if (status !== 0) throw new Error(`CLI failed (${status}): ${stderr}`);
  return stdout;
}

export class Rpc {
  counter = 0;
  pending = new Map<number, { resolve: (value: any) => void; reject: (error: Error) => void }>();
  events: any[] = [];
  changed = new Set<() => void>();
  constructor(public socket: WebSocket) {
    socket.onmessage = event => {
      const value = JSON.parse(String(event.data));
      if (value.id != null) {
        const pending = this.pending.get(value.id);
        this.pending.delete(value.id);
        if (value.error) pending?.reject(new Error(JSON.stringify(value.error)));
        else pending?.resolve(value.result);
      } else {
        this.events.push(value);
        for (const notify of this.changed) notify();
      }
    };
    socket.onclose = () => {
      for (const pending of this.pending.values()) pending.reject(new Error("Client disconnected"));
      this.pending.clear();
    };
  }
  static async connect(endpoint: string) {
    const socket = new WebSocket(endpoint);
    await deadline(new Promise<void>((resolve, reject) => {
      socket.onopen = () => resolve();
      socket.onerror = () => reject(new Error("WebSocket connect failed"));
    }), "App Server connect");
    const rpc = new Rpc(socket);
    await rpc.call("initialize", { clientInfo: { name: "provenance_test", version: "test" }, capabilities: { experimentalApi: true } });
    socket.send(JSON.stringify({ method: "initialized" }));
    return rpc;
  }
  call(method: string, params: any): Promise<any> {
    const id = ++this.counter;
    return deadline(new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.socket.send(JSON.stringify({ id, method, params }));
    }), method);
  }
  async event(predicate: (event: any) => boolean) {
    const find = () => this.events.find(predicate);
    if (find()) return find();
    let listener: () => void;
    return deadline(new Promise<any>(resolve => {
      listener = () => { const value = find(); if (value) resolve(value); };
      this.changed.add(listener);
    }), "App Server event").finally(() => this.changed.delete(listener!));
  }
  close() { this.socket.close(); }
}

export class Harness {
  directory = mkdtempSync(join(tmpdir(), "provenance-client-"));
  repository = join(this.directory, "repository");
  env = { ...process.env, XDG_CACHE_HOME: join(this.directory, "cache"), PATH: `${dirname(binary)}:${process.env.PATH}` };
  children: Subprocess[] = [];
  rpc!: Rpc;
  endpoint!: string;
  requests: any[] = [];
  requestListeners = new Set<() => void>();
  release?: () => void;
  blocked?: Promise<void>;
  server?: ReturnType<typeof Bun.serve>;
  adapter?: Subprocess;
  adapterError?: Promise<string>;
  errors: string[] = [];
  constructor() { mkdirSync(this.repository); }
  holdInference() { this.blocked = new Promise(resolve => this.release = resolve); }
  async start() {
    await command(["init", "--path", this.repository, "--disposition-actor-id", "reviewer"], this.env);
    await this.create("req_notifications", "requirement", ["--statement", "The agent receives saved changes."]);
    await this.create("topic_notifications", "topic", ["--requirement-id", "req_notifications", "--title", "Agent messages"]);
    const host = Bun.spawn([binary, "review", "--repo", this.repository, "--repository-id", "test", "--scope", "default", "--no-open"], { env: this.env, stdout: "pipe", stderr: "pipe" });
    this.children.push(host);
    const lines = createInterface({ input: Readable.fromWeb(host.stdout as any) });
    await deadline(new Promise(resolve => lines.once("line", line => resolve(JSON.parse(line)))), "review host startup");
    this.drain(host.stderr);
    this.server = Bun.serve({ port: 0, fetch: async request => {
      this.requests.push(await request.json());
      for (const notify of this.requestListeners) notify();
      const blocked = this.blocked; this.blocked = undefined;
      if (blocked) await blocked;
      const id = crypto.randomUUID();
      const item = { id, type: "message", role: "assistant", status: "completed", content: [{ type: "output_text", text: "Fixture received the message.", annotations: [] }] };
      const response = { id: `response_${id}`, object: "response", created_at: 0, status: "completed", error: null, incomplete_details: null, output: [item], usage: { input_tokens: 1, output_tokens: 1, total_tokens: 2 } };
      const events = [
        { type: "response.created", response: { ...response, status: "in_progress", output: [] } },
        { type: "response.output_item.added", output_index: 0, item: { ...item, status: "in_progress", content: [] } },
        { type: "response.output_text.delta", item_id: id, output_index: 0, content_index: 0, delta: item.content[0].text },
        { type: "response.output_item.done", output_index: 0, item },
        { type: "response.completed", response },
      ];
      return new Response(events.map((value, sequence_number) => `event: ${value.type}\ndata: ${JSON.stringify({ ...value, sequence_number })}\n\n`).join(""), { headers: { "content-type": "text/event-stream" } });
    } });
    const portProbe = Bun.serve({ port: 0, fetch: () => new Response() });
    this.endpoint = `ws://127.0.0.1:${portProbe.port}`;
    portProbe.stop(true);
    mkdirSync(join(this.directory, "codex"));
    const client = Bun.spawn(["codex", "app-server", "--listen", this.endpoint,
      "-c", 'model_provider="fixture"', "-c", 'model="fixture"',
      "-c", 'model_providers.fixture.name="Fixture"', "-c", `model_providers.fixture.base_url="http://127.0.0.1:${this.server.port}"`,
      "-c", 'model_providers.fixture.wire_api="responses"', "-c", "model_providers.fixture.requires_openai_auth=false"],
      { env: { ...this.env, CODEX_HOME: join(this.directory, "codex") }, stdout: "pipe", stderr: "pipe" });
    this.children.push(client); this.drain(client.stdout); this.drain(client.stderr);
    // Test readiness only. Production adapters make one connection attempt.
    const until = Date.now() + 10000;
    while (true) {
      try { this.rpc = await Rpc.connect(this.endpoint); break; }
      catch (error) { if (Date.now() > until) throw new Error(`${error}: ${this.errors.join("\n")}`); await Bun.sleep(50); }
    }
    return this;
  }
  async thread(cwd = this.repository) {
    const result = await this.rpc.call("thread/start", { model: "fixture", modelProvider: "fixture", cwd, approvalPolicy: "never", sandbox: "read-only" });
    return result.thread.id as string;
  }
  async create(id: string, type: string, fields: string[]) {
    return command([id, "create", "--type", type, ...fields, "--repo", this.repository], this.env);
  }
  async question(id: string) {
    await this.create(id, "question", ["--topic-id", "topic_notifications", "--question", `Question ${id}`, "--resolution-method", "research"]);
  }
  startAdapter(threadId: string, clientEndpoint = this.endpoint) {
    const config = join(this.directory, "adapter.json");
    writeFileSync(config, JSON.stringify({ clientEndpoint, threadId, repository: this.repository, scope: "default", rootId: "req_notifications" }));
    this.adapter = Bun.spawn(["bun", join(import.meta.dir, "../codex.ts"), config], { env: this.env, stdout: "pipe", stderr: "pipe" });
    this.children.push(this.adapter); this.drain(this.adapter.stdout);
    this.adapterError = new Response(this.adapter.stderr).text();
    void this.adapterError.then(text => this.errors.push(text));
    return this.adapter;
  }
  async input(text: string, start = 0) {
    const find = () => this.requests.slice(start).find(value => JSON.stringify(value.input).includes(text));
    if (find()) return find();
    let listener: () => void;
    const observed = new Promise<any>(resolve => {
      listener = () => { if (find()) resolve(find()); };
      this.requestListeners.add(listener);
    });
    const ended = this.adapter?.exited.then(status => { throw new Error(`Adapter exited before delivery (${status}): ${this.errors.join("\n")}`); });
    return deadline(ended ? Promise.race([observed, ended]) : observed, `model input ${text}`).finally(() => this.requestListeners.delete(listener!));
  }
  async transcript(threadId: string) { return (await this.rpc.call("thread/read", { threadId, includeTurns: true })).thread; }
  drain(stream: ReadableStream) {
    const lines = createInterface({ input: Readable.fromWeb(stream as any) });
    lines.on("line", line => this.errors.push(line));
  }
  async close() {
    this.release?.(); this.rpc?.close();
    for (const child of this.children.reverse()) child.kill();
    await Promise.all(this.children.map(child => child.exited));
    this.server?.stop(true); rmSync(this.directory, { recursive: true, force: true });
  }
}
