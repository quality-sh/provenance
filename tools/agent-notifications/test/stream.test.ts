import { expect, test } from "bun:test";
import { chmodSync, mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

// Implementation aid: an independent CLI peer checks the adapter's public stream contract.
async function withFrame(value: unknown) {
  const directory = mkdtempSync(join(tmpdir(), "provenance-frame-"));
  const program = join(directory, "provenance");
  writeFileSync(program, `#!/usr/bin/env bun\nprocess.stdout.write(${JSON.stringify(JSON.stringify(value) + "\n")});\nsetInterval(() => {}, 1000);\n`);
  chmodSync(program, 0o700);
  const code = `import { FollowStream } from ${JSON.stringify(join(import.meta.dir, "../stream.ts"))};
    const stream = new FollowStream({ repository: ${JSON.stringify(directory)}, scope: "default", rootId: "req_test" });
    try { console.log(JSON.stringify((await stream.frames().next()).value)); }
    catch (error) { console.error(error.message); process.exitCode = 1; }
    finally { await stream.close(); }`;
  const child = Bun.spawn(["bun", "-e", code], { env: { ...process.env, PATH: `${directory}:${process.env.PATH}` }, stdout: "pipe", stderr: "pipe" });
  try {
    const [status, stdout, error] = await Promise.all([child.exited, new Response(child.stdout).text(), new Response(child.stderr).text()]);
    return { status, stdout, error };
  } finally { child.kill(); rmSync(directory, { force: true, recursive: true }); }
}
const state = { type: "state", epoch: "host", sequence: 0, continuity: "unknown", records: [], removedIds: [] };

test("a read error retains its reason without claiming host loss", async () => {
  const value = { type: "read-error", epoch: "host", sequence: 1, continuity: "unknown", reason: "req_test was removed" };
  const result = await withFrame(value);
  expect(result.status).toBe(0);
  expect(JSON.parse(result.stdout)).toEqual(value);
});

test("disconnected frames cannot carry saved records", async () => {
  const result = await withFrame({ ...state, type: "disconnected", records: [{ id: "unexpected" }] });
  expect(result.status).not.toBe(0);
  expect(result.error).toContain("Invalid Provenance state frame");
});

