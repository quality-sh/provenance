import type { Subprocess } from "bun";

export interface Selection {
  repository: string;
  scope: string;
  rootId: string;
}

export interface StateFrame {
  type: "state" | "reset" | "disconnected";
  epoch: string;
  sequence: number;
  continuity: "current" | "unknown";
  records: unknown[];
  removedIds: string[];
  continuations: unknown[];
}

function frame(line: string): StateFrame {
  const value = JSON.parse(line);
  if (!value || !["state", "reset", "disconnected"].includes(value.type)
    || typeof value.epoch !== "string" || !Number.isSafeInteger(value.sequence)
    || value.sequence < 0 || !["current", "unknown"].includes(value.continuity)
    || !Array.isArray(value.records) || !Array.isArray(value.removedIds)
    || !value.removedIds.every((id: unknown) => typeof id === "string")
    || !Array.isArray(value.continuations)) {
    throw new Error("Invalid Provenance state frame; continuity is unknown");
  }
  return value;
}

/** Owns one CLI stream and stops it when client delivery ends. */
export class FollowStream {
  private child: Subprocess<"ignore", "pipe", "pipe">;
  private reader: ReadableStreamDefaultReader<Uint8Array>;
  private diagnostics: Promise<string>;
  constructor(selection: Selection) {
    this.child = Bun.spawn(["provenance", selection.rootId, "get", "--view", "review",
      "--follow", "--format", "json", "--repo", selection.repository, "--scope", selection.scope],
      { stdin: "ignore", stdout: "pipe", stderr: "pipe" });
    this.reader = this.child.stdout.getReader();
    this.diagnostics = new Response(this.child.stderr).text();
  }
  async *frames(): AsyncGenerator<StateFrame> {
    const decoder = new TextDecoder("utf-8", { fatal: true });
    let pending = "";
    while (true) {
      const { value, done } = await this.reader.read();
      pending += done ? decoder.decode() : decoder.decode(value, { stream: true });
      // The canonical query bounds pages. Refuse an unexpected unbounded line.
      if (pending.length > 16 * 1024 * 1024) throw new Error("Provenance state page exceeds the stream bound");
      let newline: number;
      while ((newline = pending.indexOf("\n")) >= 0) {
        const line = pending.slice(0, newline);
        pending = pending.slice(newline + 1);
        yield frame(line);
      }
      if (done) break;
    }
    if (pending) throw new Error("Incomplete Provenance state frame; continuity is unknown");
    const status = await this.child.exited;
    const detail = await this.diagnostics;
    throw new Error(`Provenance stream ended (${status}); continuity is unknown${detail ? `: ${detail.trim()}` : ""}`);
  }
  async close() {
    this.child.kill();
    await this.reader.cancel();
    await this.child.exited;
  }
}
