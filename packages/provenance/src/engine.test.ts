import assert from "node:assert/strict";
import { chmodSync, existsSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { configure, defineSpec, plan } from "./index.js";

interface HttpSettings { endpoint?: string; bearer?: string; repositoryId?: string; localRoot?: string; scope?: string }
function configureHttp(settings: HttpSettings): void { configure(settings); }
function spec() {
  return defineSpec("connection-test", ({ requirement }) => ({
    sharing: requirement("sharing", { statement: "The graph remains readable." }),
  }));
}
function named(name: string) { return (error: unknown) => error instanceof Error && error.constructor.name === name; }
async function recordingHost(initialVersion = 7) {
  let version = initialVersion;
  const requests: { path: string; authorization?: string; body: unknown }[] = [];
  const server = createServer(async (request, response) => {
    let body = "";
    for await (const chunk of request) body += String(chunk);
    requests.push({ path: request.url ?? "", authorization: request.headers.authorization, body: body ? JSON.parse(body) : null });
    response.setHeader("content-type", "application/json");
    if (request.url === "/metadata") response.end(JSON.stringify({ engine_version: "fixture", protocol_version: version }));
    else {
      response.statusCode = 403;
      response.end(JSON.stringify({ protocol_version: 7, operation: "plan", error: { kind: "access_denied" } }));
    }
  });
  await new Promise<void>(resolve => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  assert.ok(address && typeof address !== "string");
  return { endpoint: `http://127.0.0.1:${address.port}`, requests, repair() { version = 7; }, async close() { server.closeAllConnections(); await new Promise<void>(resolve => server.close(() => resolve())); } };
}

// @provenance verification: examples
// @provenance rule: rule_sdk_protocol_handshake
async function rejectsIncompatibleHostBeforeOperation(): Promise<void> {
  const host = await recordingHost(9);
  try {
    configureHttp({ endpoint: host.endpoint, bearer: "fixture-token", repositoryId: "opaque", localRoot: process.cwd() });
    await assert.rejects(plan(spec()), named("ProtocolMismatchError"));
    assert.deepEqual(host.requests.map(request => request.path), ["/metadata"]);
  } finally { await host.close(); }
}
test("HTTP compatibility check rejects an incompatible host before any operation", rejectsIncompatibleHostBeforeOperation);

test("failed compatibility checks do not poison a repaired endpoint", async () => {
  const host = await recordingHost(4);
  try {
    configureHttp({ endpoint: host.endpoint, bearer: "fixture-token", repositoryId: "opaque", localRoot: process.cwd() });
    await assert.rejects(plan(spec()), named("ProtocolMismatchError"));
    host.repair();
    await assert.rejects(plan(spec()), named("OperationError"));
    assert.deepEqual(host.requests.map(request => request.path), ["/metadata", "/metadata", "/v7/operations/plan"]);
  } finally { await host.close(); }
});

test("endpoint switching sends credentials and the opaque repository to the selected host", async () => {
  const first = await recordingHost();
  const second = await recordingHost();
  try {
    for (const [host, bearer, repositoryId] of [[first, "first-token", "first"], [second, "second-token", "second"]] as const) {
      configureHttp({ endpoint: host.endpoint, bearer, repositoryId, localRoot: process.cwd(), scope: "selected" });
      await assert.rejects(plan(spec()), named("OperationError"));
      assert.deepEqual(host.requests.map(request => request.path), ["/metadata", "/v7/operations/plan"]);
      assert.ok(host.requests.every(request => request.authorization === `Bearer ${bearer}`));
      assert.deepEqual((host.requests[1].body as { context: unknown }).context, { repository: repositoryId, scope: "selected" });
    }
    assert.equal(first.requests.length, 2);
  } finally { await first.close(); await second.close(); }
});

test("connection failure never falls back to an engine subprocess", async () => {
  const root = mkdtempSync(join(tmpdir(), "provenance-no-fallback-"));
  const executable = join(root, "engine.mjs");
  const marker = join(root, "launched");
  const previous = process.env.PROVENANCE_BIN;
  writeFileSync(executable, `#!/usr/bin/env node\nimport {writeFileSync} from 'node:fs';writeFileSync(${JSON.stringify(marker)},'launched');`);
  chmodSync(executable, 0o755);
  process.env.PROVENANCE_BIN = executable;
  try {
    configureHttp({ endpoint: "http://127.0.0.1:0", bearer: "fixture-token", repositoryId: "opaque", localRoot: root });
    await assert.rejects(plan(spec()), named("ConnectionError"));
    assert.equal(existsSync(marker), false);
  } finally { if (previous === undefined) delete process.env.PROVENANCE_BIN; else process.env.PROVENANCE_BIN = previous; rmSync(root, { recursive: true, force: true }); }
});

async function withoutConnectionEnvironment(run: () => Promise<void>) {
  const names = ["PROVENANCE_ENDPOINT", "PROVENANCE_REPOSITORY_ID", "PROVENANCE_REPO"];
  const previous = names.map(name => process.env[name]);
  names.forEach(name => delete process.env[name]);
  try { await run(); } finally { names.forEach((name, index) => { if (previous[index] === undefined) delete process.env[name]; else process.env[name] = previous[index]; }); }
}
test("a legacy repository path does not become an opaque repository identifier", async () => {
  const host = await recordingHost();
  try {
    await withoutConnectionEnvironment(async () => {
      process.env.PROVENANCE_REPO = "/legacy/repository/path";
      await assert.rejects(async () => {
        configureHttp({ endpoint: host.endpoint, bearer: "fixture-token", localRoot: process.cwd() });
        await plan(spec());
      }, /repositoryId|PROVENANCE_REPOSITORY_ID|repository identifier/i);
      assert.deepEqual(host.requests, []);
    });
  } finally { await host.close(); }
});
test("an endpoint must be configured explicitly", async () => {
  await withoutConnectionEnvironment(async () => {
    await assert.rejects(async () => {
      configureHttp({ repositoryId: "opaque", localRoot: process.cwd() });
      await plan(spec());
    }, /endpoint|PROVENANCE_ENDPOINT/i);
  });
});
