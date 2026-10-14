import { test, expect } from "bun:test";
import { existsSync, readFileSync } from "node:fs";
import { Harness, deadline } from "./client-harness";

function running(pid: number): boolean {
  try { return readFileSync(`/proc/${pid}/stat`, "utf8").split(") ")[1][0] !== "Z"; }
  catch { return false; }
}

// Implementation aid: Unix termination must stop the actual owned CLI child.
for (const signal of ["SIGTERM", "SIGINT"] as const) {
  test(`adapter ${signal} stops its follow child`, async () => {
    if (process.platform !== "linux") return;
    const harness = new Harness();
    try {
      await harness.start();
      const thread = await harness.thread();
      const adapter = harness.startAdapter(thread);
      await harness.input("req_notifications");
      const children = readFileSync(`/proc/${adapter.pid}/task/${adapter.pid}/children`, "utf8").trim().split(/\s+/).map(Number);
      expect(children.length).toBeGreaterThan(0);
      expect(children.every(running)).toBe(true);
      adapter.kill(signal);
      await deadline(adapter.exited, "adapter termination");
      expect(children.some(running)).toBe(false);
    } finally { await harness.close(); }
  }, 60000);
}

// Implementation aid: an abrupt ordinary process exit must stop the owned stream.
test("ordinary adapter process exit stops its child", async () => {
  if (process.platform !== "linux") return;
  const harness = new Harness();
  let pid: number | undefined;
  try {
    await harness.start(false);
    const script = `import { FollowStream } from ${JSON.stringify(new URL("../stream.ts", import.meta.url).pathname)};
      const stream = new FollowStream(${JSON.stringify({ repository: harness.repository, scope: "default", rootId: "req_notifications" })});
      await stream.frames().next();
      console.log(await Bun.file('/proc/' + process.pid + '/task/' + process.pid + '/children').text());
      process.exit(1);`;
    const child = Bun.spawn(["bun", "-e", script], { env: harness.env, stdout: "pipe", stderr: "pipe" });
    harness.children.push(child);
    const [status, stdout] = await deadline(Promise.all([child.exited, new Response(child.stdout).text()]), "ordinary exit");
    expect(status).toBe(1);
    pid = Number(stdout.trim());
    expect(pid).toBeGreaterThan(0);
    // The process must have stopped; an adopted zombie has already stopped.
    const until = Date.now() + 2000;
    while (running(pid) && Date.now() < until) await Bun.sleep(25);
    expect(running(pid)).toBe(false);
  } finally {
    if (pid && existsSync(`/proc/${pid}`)) { try { process.kill(pid, "SIGKILL"); } catch {} }
    await harness.close();
  }
}, 60000);
