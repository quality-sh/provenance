// Test harness only. SDK consumers connect to the already-running fixture.
import { spawn } from "node:child_process";
import { randomBytes } from "node:crypto";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const defaultBinary = fileURLToPath(new URL(`../../../target/debug/existing-root-host-fixture${process.platform === "win32" ? ".exe" : ""}`, import.meta.url));
function deadline(promise, message) {
  let timer;
  return Promise.race([promise, new Promise((_, reject) => {
    timer = setTimeout(() => reject(new Error(message)), 10_000);
  })]).finally(() => clearTimeout(timer));
}

export async function startFixtureHost({ root, binary = process.env.PROVENANCE_TEST_HOST ?? defaultBinary, arguments: args = [], repositoryId = "fixture", scope = "default" }) {
  const token = randomBytes(24).toString("hex");
  const localRoot = resolve(root);
  const child = spawn(resolve(binary), args, {
    env: { ...process.env, PROVENANCE_FIXTURE_ROOT: localRoot, PROVENANCE_FIXTURE_REPOSITORY_ID: repositoryId, PROVENANCE_FIXTURE_SCOPE: scope, PROVENANCE_FIXTURE_TOKEN: token },
    stdio: ["pipe", "pipe", "pipe"],
  });
  let stderr = "";
  child.stderr.on("data", chunk => { stderr = (stderr + chunk.toString()).slice(-65_536); });
  const exited = new Promise(resolveExit => {
    child.once("error", error => resolveExit({ error }));
    child.once("exit", (code, signal) => resolveExit({ code, signal }));
  });
  const killOnExit = () => child.kill();
  process.once("exit", killOnExit);
  let shutdown;
  function close() {
    return shutdown ??= stop();
  }
  async function stop() {
    child.stdin.end();
    try {
      const result = await deadline(exited, "Fixture host did not stop after stdin EOF");
      if (result.error) throw result.error;
      if (result.code !== 0) throw new Error(`Fixture host failed (${result.code ?? result.signal}): ${stderr}`);
    } finally { child.kill(); process.removeListener("exit", killOnExit); }
  }
  try {
    const ready = new Promise((resolveReady, reject) => {
      let buffer = "";
      let settled = false;
      function finish(error, metadata) {
        if (settled) return;
        settled = true;
        buffer = "";
        child.stdout.removeListener("data", onData);
        child.stdout.resume();
        if (error) reject(error); else resolveReady(metadata);
      }
      function onData(chunk) {
        const text = chunk.toString();
        const line = text.indexOf("\n");
        const segment = line === -1 ? text : text.slice(0, line);
        if (buffer.length + segment.length > 65_536) {
          finish(new Error("Fixture readiness message is too large"));
          return;
        }
        buffer += segment;
        if (line !== -1) {
          try { finish(null, JSON.parse(buffer)); } catch (error) { finish(error); }
        }
      }
      child.stdout.on("data", onData);
      exited.then(() => finish(new Error(`Fixture host exited before readiness: ${stderr}`)));
    });
    const metadata = await deadline(ready, "Fixture host did not report readiness");
    const url = new URL(metadata.url);
    if (url.protocol !== "http:" || url.hostname !== "127.0.0.1" || metadata.repository !== repositoryId || metadata.scope !== scope) throw new Error("Invalid fixture readiness metadata");
    return { environment: { PROVENANCE_ENDPOINT: metadata.url, PROVENANCE_TOKEN: token, PROVENANCE_REPOSITORY_ID: repositoryId, PROVENANCE_LOCAL_ROOT: localRoot, PROVENANCE_SCOPE: scope, PROVENANCE_BIN: undefined, PROVENANCE_REPO: undefined }, close };
  } catch (error) {
    child.kill();
    process.removeListener("exit", killOnExit);
    throw error;
  }
}
