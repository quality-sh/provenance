import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

// Implementation aid: prove the public isolation assertion rejects a wrong queue target.
const path = join(import.meta.dir, "../codex.ts");
const original = readFileSync(path, "utf8");
const target = "threadId: config.threadId, clientUserMessageId,";
if (!original.includes(target)) throw new Error("Queue target mutation no longer applies");
try {
  writeFileSync(path, original.replace(target, `threadId: (await client.call("thread/start", {
    model: "fixture", modelProvider: "fixture", cwd: config.repository,
    approvalPolicy: "never", sandbox: "read-only",
  }, threadResponse)).thread.id, clientUserMessageId,`));
  const child = Bun.spawn(["bun", "test", join(import.meta.dir, "codex.test.ts"), "-t", "every captured"], {
    stdout: "pipe", stderr: "pipe",
  });
  const [status, stdout, stderr] = await Promise.all([
    child.exited, new Response(child.stdout).text(), new Response(child.stderr).text(),
  ]);
  console.log(stdout + stderr);
  if (status === 0 || !stderr.includes("Received: false")) {
    throw new Error("Wrong-thread mutation did not fail the isolation assertion");
  }
  console.log("Wrong-thread mutation failed the isolation assertion as required.");
} finally { writeFileSync(path, original); }
