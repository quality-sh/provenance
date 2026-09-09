import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync, rmSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

test("fixture harness supplies explicit connection settings and stops on EOF", async () => {
  const directory = mkdtempSync(join(tmpdir(), "provenance-host-harness-"));
  const script = join(directory, "fixture.mjs");
  const stopped = join(directory, "stopped");
  writeFileSync(script, `import {writeFileSync} from 'node:fs';
console.log(JSON.stringify({url:'http://127.0.0.1:45678',repository:process.env.PROVENANCE_FIXTURE_REPOSITORY_ID,scope:process.env.PROVENANCE_FIXTURE_SCOPE}));
process.stdin.resume(); process.stdin.on('end',()=>writeFileSync(${JSON.stringify(stopped)},'closed'));`);
  try {
    const { startFixtureHost } = await import("./fixture-host.js");
    const fixture = await startFixtureHost({ root: directory, binary: process.execPath, arguments: [script], repositoryId: "opaque", scope: "default" });
    assert.equal(fixture.environment.PROVENANCE_ENDPOINT, "http://127.0.0.1:45678");
    assert.equal(fixture.environment.PROVENANCE_REPOSITORY_ID, "opaque");
    assert.equal(fixture.environment.PROVENANCE_LOCAL_ROOT, directory);
    assert.ok(fixture.environment.PROVENANCE_TOKEN.length >= 32);
    assert.equal(fixture.environment.PROVENANCE_BIN, undefined);
    await fixture.close();
    assert.equal(readFileSync(stopped,"utf8"), "closed");
  } finally { rmSync(directory, { recursive: true, force: true }); }
});

test("fixture startup failure reaches the harness before a consumer runs", async () => {
  const { startFixtureHost } = await import("./fixture-host.js");
  await assert.rejects(startFixtureHost({ root: tmpdir(), binary: process.execPath, arguments: ["-e", "process.stderr.write('fixture refused configuration');process.exit(2)"] }), /fixture refused configuration/);
});

test("fixture startup rejects a mismatched opaque target", async () => {
  const { startFixtureHost } = await import("./fixture-host.js");
  await assert.rejects(startFixtureHost({ root: tmpdir(), binary: process.execPath, arguments: ["-e", "console.log(JSON.stringify({url:'http://127.0.0.1:1',repository:'wrong',scope:'default'}));process.stdin.resume()"] }), /Invalid fixture readiness metadata/);
});

test("concurrent and later close calls preserve the same shutdown failure", async () => {
  const { startFixtureHost } = await import("./fixture-host.js");
  const fixture = await startFixtureHost({
    root: tmpdir(),
    binary: process.execPath,
    arguments: ["-e", `
console.log(JSON.stringify({url:'http://127.0.0.1:1',repository:'fixture',scope:'default'}));
process.stdin.resume();
process.stdin.on('end',()=>{process.stderr.write('shutdown failed');process.exitCode=2;});`],
  });
  const results = await Promise.allSettled([fixture.close(), fixture.close()]);
  assert.equal(results[0].status, "rejected");
  assert.equal(results[1].status, "rejected");
  assert.match(results[0].reason.message, /shutdown failed/);
  assert.equal(results[1].reason, results[0].reason);
  await assert.rejects(fixture.close(), error => error === results[0].reason);
});
