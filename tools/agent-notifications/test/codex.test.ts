import { test, expect } from "bun:test";
import { Harness, deadline } from "./client-harness";
import { verifies } from "../../../packages/provenance/src/rules";

// Real host, CLI stream and App Server; only inference is deterministic.
test("saved Questions reach the bound idle thread and its model-visible transcript", async function deliversSavedChanges() {
  verifies("rule_review_codex_mailbox", "examples");
  const harness = new Harness();
  try {
    await harness.start();
    const thread = await harness.thread();
    const other = await harness.thread();
    harness.startAdapter(thread);
    await harness.input("req_notifications");
    await harness.rpc.event(event => event.method === "turn/completed" && event.params.threadId === thread);
    const before = harness.requests.length;
    await harness.question("q_idle");
    await harness.input("q_idle", before);
    await harness.rpc.event(event => event.method === "item/completed" && event.params.threadId === thread && JSON.stringify(event.params.item).includes("q_idle"));
    const questionInput = harness.requests.length;
    await harness.decision();
    await harness.input('\\"decision\\":\\"accepted\\"', questionInput);
    const decisionInput = harness.requests.length;
    await harness.reply();
    await harness.input("reply_probe", decisionInput);
    await harness.rpc.event(event => event.method === "item/completed" && event.params.threadId === thread && JSON.stringify(event.params.item).includes("reply_probe"));
    const transcript = JSON.stringify(await harness.transcript(thread));
    expect(transcript).toContain("q_idle");
    expect(transcript).toContain('\\"decision\\":\\"accepted\\"');
    expect(transcript).toContain("reply_probe");
    expect(transcript).toContain("clientId");
    expect(harness.requestThreads.length).toBeGreaterThan(0);
    expect(harness.requestThreads.every(id => id === thread)).toBe(true);
    expect(harness.requestThreads).not.toContain(other);
    harness.children[1].kill();
    expect(await deadline(harness.adapter!.exited, "client-loss adapter exit")).not.toBe(0);
  } finally { await harness.close(); }
}, 60000);

// Implementation aid: queue order must survive a busy model and cancellation of the host.
test("busy delivery preserves message order and reports a stream disconnect", async () => {
  const harness = new Harness();
  try {
    await harness.start();
    const thread = await harness.thread();
    harness.holdInference();
    harness.startAdapter(thread);
    await harness.input("req_notifications");
    const queued = async (text: string) => {
      let offset = harness.rpc.events.length;
      while (true) {
        const page = await harness.rpc.call("thread/queue/list", { threadId: thread });
        const match = page.data.find((entry: any) => JSON.stringify(entry.input).includes(text));
        if (match) return page.data;
        await harness.rpc.event(event => harness.rpc.events.indexOf(event) >= offset && event.method === "thread/queue/changed" && event.params.threadId === thread);
        offset = harness.rpc.events.length;
      }
    };
    await harness.question("q_busy_first");
    const first = await queued("q_busy_first");
    await harness.question("q_busy_second");
    const both = await queued("q_busy_second");
    expect(both[0].clientUserMessageId).toBe(first[0].clientUserMessageId);
    expect(both[1].clientUserMessageId).not.toBe(both[0].clientUserMessageId);
    expect(JSON.stringify(both[0].input)).not.toContain("q_busy_second");
    expect(JSON.stringify(both[1].input)).toContain("q_busy_second");
    await harness.reset();
    const reset = await queued("reset");
    expect(reset).toHaveLength(3);
    expect(reset[2].input[0].text).toContain('"type":"reset"');
    expect(reset[2].input[0].text).toContain('"continuity":"unknown"');
    harness.children[0].kill();
    expect(await deadline(harness.adapter!.exited, "host-loss adapter exit")).not.toBe(0);
    const disconnected = await harness.rpc.call("thread/queue/list", { threadId: thread });
    expect(disconnected.data).toHaveLength(4);
    expect(disconnected.data[3].input[0].text).toContain('"type":"disconnected"');
    expect(disconnected.data[3].input[0].text).toContain('"continuity":"unknown"');
    expect(harness.requestThreads.every(id => id === thread)).toBe(true);
  } finally { await harness.close(); }
}, 60000);

// Implementation aid: binding must be checked before a child stream or queue request starts.
test("a thread in another repository refuses the binding", async () => {
  const harness = new Harness();
  try {
    await harness.start();
    const thread = await harness.thread(harness.directory);
    const adapter = harness.startAdapter(thread);
    expect(await deadline(adapter.exited, "wrong repository refusal")).not.toBe(0);
    expect(harness.requests).toHaveLength(0);
    expect(await harness.adapterError).toContain("thread repository does not match");
    expect(JSON.stringify(await harness.transcript(thread, false))).not.toContain("req_notifications");
  } finally { await harness.close(); }
}, 60000);

// Implementation aid: an accepted message with a lost response must not be sent again.
test("a lost queue acknowledgement stops with an uncertain identity", async () => {
  const harness = new Harness();
  const upstream = new Set<WebSocket>();
  let accepted = 0;
  const proxy = Bun.serve<{ upstream: WebSocket; queueIds: Set<number> }>({
    port: 0,
    fetch(request, server) {
      const socket = new WebSocket(harness.endpoint);
      upstream.add(socket);
      if (server.upgrade(request, { data: { upstream: socket, queueIds: new Set() } })) return;
      return new Response("Upgrade required", { status: 400 });
    },
    websocket: {
      open(socket) {
        socket.data.upstream.onmessage = event => {
          const value = JSON.parse(String(event.data));
          if (socket.data.queueIds.has(value.id) && value.result) {
            accepted++; socket.close(); return;
          }
          socket.send(String(event.data));
        };
      },
      async message(socket, bytes) {
        const text = String(bytes); const value = JSON.parse(text);
        if (value.method === "thread/queue/add") socket.data.queueIds.add(value.id);
        if (socket.data.upstream.readyState !== WebSocket.OPEN) {
          await deadline(new Promise<void>((resolve, reject) => {
            socket.data.upstream.onopen = () => resolve();
            socket.data.upstream.onerror = () => reject(new Error("proxy upstream failed"));
          }), "proxy connection");
        }
        socket.data.upstream.send(text);
      },
      close(socket) { socket.data.upstream.close(); },
    },
  });
  try {
    await harness.start();
    const thread = await harness.thread();
    harness.startAdapter(thread, `ws://127.0.0.1:${proxy.port}`);
    expect(await deadline(harness.adapter!.exited, "uncertain adapter exit")).not.toBe(0);
    expect(accepted).toBe(1);
    expect(await harness.adapterError).toContain("uncertain");
    expect(await harness.adapterError).toContain("clientUserMessageId");
  } finally {
    for (const socket of upstream) socket.close();
    proxy.stop(true); await harness.close();
  }
}, 60000);

// Implementation aid: each inference request must identify the configured thread.
test("every captured model request belongs to the bound thread", async () => {
  const harness = new Harness();
  try {
    await harness.start();
    const thread = await harness.thread();
    const other = await harness.thread();
    harness.startAdapter(thread);
    await harness.input("req_notifications");
    expect(harness.requestThreads.length).toBeGreaterThan(0);
    expect(harness.requestThreads.every(id => id === thread)).toBe(true);
    expect(harness.requestThreads).not.toContain(other);
  } finally { await harness.close(); }
}, 60000);
