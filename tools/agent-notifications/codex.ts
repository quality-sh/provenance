import { realpathSync } from "node:fs";
import { FollowStream, type Selection, type StateFrame } from "./stream";

export interface Configuration extends Selection {
  clientEndpoint: string;
  threadId: string;
}

class Rejected extends Error {}

class AppServer {
  private nextId = 0;
  private pending = new Map<number, { resolve: (value: any) => void; reject: (error: Error) => void }>();
  readonly closed: Promise<Error>;
  private ended!: (error: Error) => void;
  private constructor(private socket: WebSocket) {
    this.closed = new Promise(resolve => this.ended = resolve);
    socket.onmessage = event => {
      try {
        const value = JSON.parse(String(event.data));
        const request = this.pending.get(value.id);
        if (!request) return;
        this.pending.delete(value.id);
        if (value.error) request.reject(new Rejected(`App Server rejected request (${value.error.code}): ${value.error.message}`));
        else request.resolve(value.result);
      } catch { this.fail(new Error("Invalid App Server response")); }
    };
    socket.onclose = () => this.fail(new Error("App Server disconnected"));
    socket.onerror = () => this.fail(new Error("App Server transport failed"));
  }
  static async connect(endpoint: string) {
    const url = new URL(endpoint);
    if (!["ws:", "wss:"].includes(url.protocol) || url.username || url.password || url.search || url.hash) {
      throw new Error("clientEndpoint must be a WebSocket URL without credentials or query parameters");
    }
    if (url.protocol === "ws:" && !["127.0.0.1", "localhost", "[::1]"].includes(url.hostname)) {
      throw new Error("A remote App Server endpoint requires wss");
    }
    const token = process.env.PROVENANCE_CODEX_TOKEN;
    const socket = new WebSocket(endpoint, token ? { headers: { Authorization: `Bearer ${token}` } } : undefined);
    const connected = new Promise<void>((resolve, reject) => {
      socket.onopen = () => resolve();
      socket.onerror = () => reject(new Error("Cannot connect to the App Server"));
      socket.onclose = () => reject(new Error("App Server closed during connection"));
    });
    let timer: ReturnType<typeof setTimeout>;
    try {
      await Promise.race([connected, new Promise<void>((_, reject) => {
        timer = setTimeout(() => reject(new Error("App Server connection timed out")), 10000);
      })]);
    } catch (error) { socket.close(); throw error; }
    finally { clearTimeout(timer!); }
    const server = new AppServer(socket);
    try {
      // The peer protocol requires clientInfo.version; no Provenance format is versioned.
      await server.call("initialize", { clientInfo: { name: "provenance_notifications", version: "" }, capabilities: { experimentalApi: true } });
      socket.send(JSON.stringify({ method: "initialized" }));
      return server;
    } catch (error) { server.close(); throw error; }
  }
  private fail(error: Error) {
    for (const request of this.pending.values()) request.reject(error);
    this.pending.clear(); this.ended(error);
  }
  async call(method: string, params: unknown) {
    const id = ++this.nextId;
    let timer: ReturnType<typeof setTimeout>;
    const response = new Promise<any>((resolve, reject) => this.pending.set(id, { resolve, reject }));
    try {
      this.socket.send(JSON.stringify({ id, method, params }));
      return await Promise.race([response, new Promise((_, reject) => {
        timer = setTimeout(() => reject(new Error("App Server acknowledgement timed out")), 10000);
      })]);
    } finally { clearTimeout(timer!); this.pending.delete(id); }
  }
  close() { this.socket.close(); this.fail(new Error("App Server connection closed")); }
}

function configuration(value: unknown): Configuration {
  if (!value || typeof value !== "object") throw new Error("Adapter configuration must be an object");
  const config = value as Configuration;
  for (const name of ["clientEndpoint", "threadId", "repository", "scope", "rootId"] as const) {
    if (typeof config[name] !== "string" || !config[name].trim()) throw new Error(`Configuration requires ${name}`);
  }
  return { ...config, repository: realpathSync(config.repository) };
}

/** Queues current graph state to the configured thread; acceptance is not model completion. */
// @provenance rule: rule_review_codex_mailbox
export async function run(value: unknown) {
  const config = configuration(value);
  let client: AppServer | undefined;
  let stream: FollowStream | undefined;
  let cancelled = false;
  let cancel!: () => void;
  const cancellation = new Promise<void>(resolve => cancel = () => { cancelled = true; resolve(); });
  process.once("SIGINT", cancel); process.once("SIGTERM", cancel);
  try {
    client = await AppServer.connect(config.clientEndpoint);
    const { thread } = await client.call("thread/read", { threadId: config.threadId, includeTurns: false });
    if (thread.id !== config.threadId || realpathSync(thread.cwd) !== config.repository) {
      throw new Error("thread repository does not match the configured binding");
    }
    stream = new FollowStream(config);
    const deliver = async () => {
      const frames = stream!.frames();
      while (true) {
        const next = await Promise.race([frames.next(), client!.closed.then(error => { throw error; })]);
        if (next.done) break;
        await enqueue(client!, config, next.value);
        if (next.value.type === "disconnected") throw new Error("Provenance host disconnected; continuity is unknown");
      }
    };
    await Promise.race([deliver(), cancellation]);
  } catch (error) { if (!cancelled) throw error; }
  finally {
    client?.close(); await stream?.close();
    process.removeListener("SIGINT", cancel); process.removeListener("SIGTERM", cancel);
  }
}

async function enqueue(client: AppServer, config: Configuration, state: StateFrame) {
  const clientUserMessageId = crypto.randomUUID();
  let result: any;
  try {
    result = await client.call("thread/queue/add", {
      threadId: config.threadId, clientUserMessageId,
      input: [{ type: "text", text: `Provenance current-state reconciliation. Repository: ${config.repository}; scope: ${config.scope}; root: ${config.rootId}.\n${JSON.stringify(state)}` }],
    });
  } catch (error) {
    if (error instanceof Rejected) throw error;
    throw new Error(`Queue acceptance is uncertain; clientUserMessageId=${clientUserMessageId}. No resend: ${error instanceof Error ? error.message : "transport failure"}`);
  }
  if (result?.queuedSubmission?.clientUserMessageId !== clientUserMessageId || typeof result.queuedSubmission.id !== "string") {
    throw new Error(`Queue acceptance is uncertain; clientUserMessageId=${clientUserMessageId}. Invalid acknowledgement; no resend`);
  }
  console.log(JSON.stringify({ type: "accepted", clientUserMessageId, queuedSubmissionId: result.queuedSubmission.id, epoch: state.epoch, sequence: state.sequence }));
}

if (import.meta.main) {
  try {
    const path = process.argv[2];
    if (!path) throw new Error("Usage: bun codex.ts CONFIGURATION.json");
    await run(await Bun.file(path).json());
  } catch (error) { console.error(error instanceof Error ? error.message : "Adapter failed"); process.exitCode = 1; }
}
