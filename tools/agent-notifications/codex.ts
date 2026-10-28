import { recordData } from "./envelope";
import { realpathSync } from "node:fs";
import { object, requiredString } from "./validation";
import { FollowStream, type Selection, type StateFrame } from "./stream";

export interface Configuration extends Selection {
  clientEndpoint: string;
  threadId: string;
}

class Rejected extends Error {}

class AppServer {
  private nextId = 0;
  private pending = new Map<number, { resolve: (value: unknown) => void; reject: (error: Error) => void }>();
  readonly closed: Promise<Error>;
  private ended!: (error: Error) => void;
  private constructor(private socket: WebSocket) {
    this.closed = new Promise(resolve => this.ended = resolve);
    socket.onmessage = event => {
      try {
        const value = object(JSON.parse(String(event.data)));
        if (typeof value.id !== "number") return;
        const request = this.pending.get(value.id);
        if (!request) return;
        if (value.error) {
          const error = object(value.error);
          if (typeof error.code !== "number" || typeof error.message !== "string") throw new Error("Invalid App Server error");
          request.reject(new Rejected(`App Server rejected request (${error.code}): ${error.message}`));
        }
        else request.resolve(value.result);
        this.pending.delete(value.id);
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
      await server.call("initialize", { clientInfo: { name: "provenance_notifications", version: "" }, capabilities: { experimentalApi: true } }, object);
      socket.send(JSON.stringify({ method: "initialized" }));
      return server;
    } catch (error) { server.close(); throw error; }
  }
  private fail(error: Error) {
    for (const request of this.pending.values()) request.reject(error);
    this.pending.clear(); this.ended(error);
  }
  async call<T>(method: string, params: unknown, decode: (value: unknown) => T): Promise<T> {
    const id = ++this.nextId;
    let timer: ReturnType<typeof setTimeout>;
    const response = new Promise<unknown>((resolve, reject) => this.pending.set(id, { resolve, reject }));
    try {
      this.socket.send(JSON.stringify({ id, method, params }));
      return decode(await Promise.race([response, new Promise<never>((_, reject) => {
        timer = setTimeout(() => reject(new Error("App Server acknowledgement timed out")), 10000);
      })]));
    } finally { clearTimeout(timer!); this.pending.delete(id); }
  }
  close() { this.socket.close(); this.fail(new Error("App Server connection closed")); }
}

function configuration(value: unknown): Configuration {
  let config: Record<string, unknown>;
  try { config = object(value); }
  catch { throw new Error("Adapter configuration must be an object"); }
  const fields = {
    clientEndpoint: requiredString(config, "clientEndpoint"),
    threadId: requiredString(config, "threadId"),
    repository: requiredString(config, "repository"),
    scope: requiredString(config, "scope"),
    rootId: requiredString(config, "rootId"),
  };
  return { ...fields, repository: realpathSync(fields.repository) };
}

function threadResponse(value: unknown): { thread: { id: string; cwd: string } } {
  const thread = object(object(value).thread);
  if (typeof thread.id !== "string" || typeof thread.cwd !== "string") throw new Error("Invalid App Server thread response");
  return { thread: { id: thread.id, cwd: thread.cwd } };
}

interface QueueAcceptance { queuedSubmission: { id: string; clientUserMessageId: string } }
function queueResponse(value: unknown): QueueAcceptance {
  const submission = object(object(value).queuedSubmission);
  if (typeof submission.id !== "string" || typeof submission.clientUserMessageId !== "string") {
    throw new Error("Invalid queue acknowledgement");
  }
  return { queuedSubmission: { id: submission.id, clientUserMessageId: submission.clientUserMessageId } };
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
    const { thread } = await client.call("thread/read", { threadId: config.threadId, includeTurns: false }, threadResponse);
    if (thread.id !== config.threadId || realpathSync(thread.cwd) !== config.repository) {
      throw new Error("thread repository does not match the configured binding");
    }
    stream = new FollowStream(config);
    let queued: Promise<void> | undefined;
    const deliver = async () => {
      const frames = stream!.frames();
      while (true) {
        const next = await frames.next();
        if (next.done) break;
        queued = enqueue(client!, config, next.value);
        await queued;
        queued = undefined;
        if (next.value.type === "disconnected") throw new Error("Provenance host disconnected; continuity is unknown");
        if (next.value.type === "read-error") throw new Error(`Provenance read failed: ${next.value.reason}`);
      }
    };
    const disconnected = client.closed.then(async error => {
      // An in-flight queue request owns the uncertain-acceptance diagnostic.
      if (queued) await queued;
      throw error;
    });
    await Promise.race([deliver(), disconnected, cancellation]);
  } catch (error) { if (!cancelled) throw error; }
  finally {
    client?.close(); await stream?.close();
    process.removeListener("SIGINT", cancel); process.removeListener("SIGTERM", cancel);
  }
}

async function enqueue(client: AppServer, config: Configuration, state: StateFrame) {
  const clientUserMessageId = crypto.randomUUID();
  let result: QueueAcceptance;
  try {
    result = await client.call("thread/queue/add", {
      threadId: config.threadId, clientUserMessageId,
      input: [{ type: "text", text: recordData(config, state) }],
    }, queueResponse);
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
