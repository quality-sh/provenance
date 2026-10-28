import { test, expect } from "bun:test";
import { decodeFrames } from "../stream";

function chunks(...text: string[]) {
  const encoder = new TextEncoder();
  return new ReadableStream<Uint8Array>({ start(controller) {
    for (const chunk of text) controller.enqueue(encoder.encode(chunk));
    controller.close();
  } }).getReader();
}
const state = { type: "state", epoch: "host", sequence: 0, continuity: "unknown", records: [], removedIds: [] };

// Implementation aid: transport chunks can contain more than one bounded page.
test("several bounded lines can share a chunk above the line limit", async () => {
  const line = JSON.stringify({ ...state, records: ["x".repeat(6 * 1024 * 1024)] }) + "\n";
  let count = 0;
  for await (const frame of decodeFrames(chunks(line.repeat(3)))) {
    expect(frame.type).toBe("state");
    count++;
  }
  expect(count).toBe(3);
});

// Implementation aid: enforce the bound on UTF-8 bytes, including a buffered partial line.
test("a multibyte line above the byte limit is refused", async () => {
  const line = JSON.stringify({ ...state, records: ["界".repeat(6 * 1024 * 1024)] }) + "\n";
  await expect(decodeFrames(chunks(line)).next()).rejects.toThrow("exceeds the stream bound");
});

test("a split unterminated line above the limit is refused", async () => {
  await expect(decodeFrames(chunks("x".repeat(8 * 1024 * 1024), "x".repeat(8 * 1024 * 1024 + 1))).next()).rejects.toThrow("exceeds the stream bound");
});
