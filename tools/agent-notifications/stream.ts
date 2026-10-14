import Ajv from "ajv";
import schema from "../../crates/provenance-cli/src/review/events/output.schema.json";
import type { Subprocess } from "bun";

export interface Selection {
  repository: string;
  scope: string;
  rootId: string;
}

interface Position {
  epoch: string;
  sequence: number;
  continuity: "current" | "unknown";
}
interface DocumentFrame extends Position {
  records: unknown[];
  removedIds: string[];
  continuations: { rootId: string; cursor: string }[];
}
export type StateFrame =
  | ({ type: "state" } & DocumentFrame)
  | ({ type: "reset" } & DocumentFrame)
  | (Position & { type: "disconnected"; continuity: "unknown"; records: []; removedIds: []; continuations: [] })
  | (Position & { type: "read-error"; continuity: "unknown"; reason: string });

const validate = new Ajv({ strict: false }).compile<StateFrame>(schema);
function frame(line: string): StateFrame {
  const value: unknown = JSON.parse(line);
  if (!validate(value)) throw new Error("Invalid Provenance state frame; continuity is unknown");
  return value;
}

interface Reader {
  read(): Promise<{ done: boolean; value?: Uint8Array }>;
  cancel(): Promise<void>;
}

/** Decodes bounded lines independently of the transport's chunk sizes. */
export async function* decodeFrames(reader: Reader): AsyncGenerator<StateFrame> {
  const decoder = new TextDecoder("utf-8", { fatal: true });
  let pending = "";
  let pendingBytes = 0;
  const encoder = new TextEncoder();
  const bound = 16 * 1024 * 1024;
  while (true) {
    const { value, done } = await reader.read();
    pendingBytes += value?.byteLength ?? 0;
    pending += done ? decoder.decode() : decoder.decode(value, { stream: true });
    let newline: number;
    while ((newline = pending.indexOf("\n")) >= 0) {
      const line = pending.slice(0, newline);
      pending = pending.slice(newline + 1);
      const bytes = encoder.encode(line).byteLength;
      if (bytes > bound) throw new Error("Provenance state page exceeds the stream bound");
      pendingBytes -= bytes + 1;
      yield frame(line);
    }
    if (pendingBytes > bound) throw new Error("Provenance state page exceeds the stream bound");
    if (done) break;
  }
  if (pending) throw new Error("Incomplete Provenance state frame; continuity is unknown");
}

/** Owns one CLI stream and stops it when client delivery ends. */
export class FollowStream {
  private child: Subprocess<"ignore", "pipe", "pipe">;
  private reader: Reader;
  private diagnostics: Promise<string>;
  private closing?: Promise<void>;
  private stopChild = () => { if (this.child.exitCode === null) this.child.kill(); };
  constructor(selection: Selection) {
    this.child = Bun.spawn(["provenance", selection.rootId, "get", "--view", "review",
      "--follow", "--format", "json", "--repo", selection.repository, "--scope", selection.scope],
      { stdin: "ignore", stdout: "pipe", stderr: "pipe" });
    this.reader = this.child.stdout.getReader();
    this.diagnostics = new Response(this.child.stderr).text();
    process.once("exit", this.stopChild);
  }
  async *frames(): AsyncGenerator<StateFrame> {
    yield* decodeFrames(this.reader);
    const status = await this.child.exited;
    const detail = await this.diagnostics;
    throw new Error(`Provenance stream ended (${status}); continuity is unknown${detail ? `: ${detail.trim()}` : ""}`);
  }
  close(): Promise<void> {
    return this.closing ??= (async () => {
      process.removeListener("exit", this.stopChild);
      this.stopChild();
      try { await this.reader.cancel(); }
      finally { await this.child.exited; }
    })();
  }
}
