import { test, expect } from "bun:test";
import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { Readable } from "node:stream";
import { createInterface } from "node:readline";
import { Harness, deadline } from "./client-harness";

class ChannelClient {
  nextId = 0;
  pending = new Map<number, (value: any) => void>();
  messages: any[] = [];
  changed = new Set<() => void>();
  child;
  constructor(harness: Harness) {
    const config = join(harness.directory, "channel.json");
    writeFileSync(config, JSON.stringify({ repository: harness.repository, scope: "default", rootId: "req_notifications" }));
    this.child = Bun.spawn(["bun", join(import.meta.dir, "../claude.ts"), config], {
      env: harness.env, stdin: "pipe", stdout: "pipe", stderr: "pipe",
    });
    harness.children.push(this.child); harness.drain(this.child.stderr);
    const lines = createInterface({ input: Readable.fromWeb(this.child.stdout as any) });
    lines.on("line", line => {
      const value = JSON.parse(line);
      if (value.id != null) { this.pending.get(value.id)?.(value); this.pending.delete(value.id); }
      else { this.messages.push(value); for (const notify of this.changed) notify(); }
    });
  }
  async initialize() {
    const id = ++this.nextId;
    const response = new Promise<any>(resolve => this.pending.set(id, resolve));
    this.child.stdin.write(JSON.stringify({ id, jsonrpc: "2.0", method: "initialize", params: {
      protocolVersion: "2025-11-25", capabilities: {}, clientInfo: { name: "channel_test", version: "" },
    } }) + "\n");
    const result = await deadline(Promise.race([response, this.child.exited.then(status => {
      throw new Error(`Channel exited before initialization (${status})`);
    })]), "channel initialize");
    expect(result.error).toBeUndefined();
    expect(result.result.capabilities.experimental["claude/channel"]).toEqual({});
    expect(result.result.capabilities.tools).toBeUndefined();
    this.child.stdin.write(JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" }) + "\n");
  }
  async state(text: string, after = 0) {
    const find = () => this.messages.slice(after).find(value => value.method === "notifications/claude/channel" && value.params.content.includes(text));
    if (find()) return find();
    let listener: () => void;
    const observed = new Promise<any>(resolve => {
      listener = () => { const value = find(); if (value) resolve(value); };
      this.changed.add(listener);
    });
    return deadline(Promise.race([observed, this.child.exited.then(status => {
      throw new Error(`Channel exited before notification (${status})`);
    })]), `channel state ${text}`).finally(() => this.changed.delete(listener!));
  }
  close() { this.child.stdin.end(); }
}

// Implementation aid: validates the channel transport, independent of account policy.
// Real Claude model visibility is checked by the interactive development-session probe.
test("the opted-in channel sends decisions, Questions and replies with the selected metadata", async () => {
  const harness = new Harness();
  try {
    await harness.start(false);
    const channel = new ChannelClient(harness);
    await channel.initialize();
    const initial = await channel.state("req_notifications");
    expect(initial.params.meta.repository).toBe(harness.repository);
    expect(initial.params.meta.rootId).toBe("req_notifications");
    expect(initial.params.meta.scope).toBe("default");
    await harness.question("q_channel");
    await channel.state("q_channel");
    await harness.decision();
    await channel.state("accepted");
    await harness.reply();
    const reply = await channel.state("reply_probe");
    expect(JSON.parse(reply.params.content).records.some((entry: any) => entry.message?.body === "reply_probe")).toBe(true);
    channel.close();
    expect(await deadline(channel.child.exited, "session closure stops channel")).toBe(0);
  } finally { await harness.close(); }
}, 60000);

// Implementation aid: reset and host loss must retain their continuity status on the wire.
test("reset and host loss reach the channel and do not start a replay", async () => {
  const harness = new Harness();
  try {
    await harness.start(false);
    const channel = new ChannelClient(harness);
    await channel.initialize(); await channel.state("req_notifications");
    const after = channel.messages.length;
    await harness.reset();
    const reset = await channel.state('"type":"reset"', after);
    expect(JSON.parse(reset.params.content).continuity).toBe("unknown");
    harness.children[0].kill();
    const lost = await channel.state('"type":"disconnected"');
    expect(JSON.parse(lost.params.content).continuity).toBe("unknown");
    expect(await deadline(channel.child.exited, "host-loss channel exit")).not.toBe(0);
  } finally { await harness.close(); }
}, 60000);
