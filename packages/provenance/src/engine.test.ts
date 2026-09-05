import assert from "node:assert/strict";
import { chmodSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { STATE_SCHEMA_VERSION } from "./protocol.js";

import { configure, defineSpec, plan } from "./index.js";
import type { ImplementationBinding } from "./protocol.js";

interface RecordedRequest {
  command: string;
  args: string[];
}

const retiredBinding: ImplementationBinding = {
  id: "implementation_binding_start",
  rule_id: "rule_start",
  declared_by: "spec://typescript/workflows",
  retired: true,
  file: "src/runtime.ts",
  symbol: "startWorkflow",
};

test("the SDK preserves retired implementation history in engine results", () => {
  assert.equal(retiredBinding.retired, true);
});

function recordingEngine(responses: Readonly<Record<string, unknown>>): {
  engine: string;
  requests: () => RecordedRequest[];
} {
  const directory = mkdtempSync(join(tmpdir(), "provenance-handshake-engine-"));
  const executable = join(directory, "engine.mjs");
  const log = join(directory, "requests.jsonl");
  writeFileSync(
    executable,
    `#!/usr/bin/env node
import { appendFileSync } from "node:fs";
const command = process.argv[3];
const args = process.argv.slice(2);
const responses = ${JSON.stringify(responses)};
appendFileSync(${JSON.stringify(log)}, JSON.stringify({ command, args }) + "\\n");
process.stdout.write(JSON.stringify(responses[command]));
`,
  );
  chmodSync(executable, 0o755);
  return {
    engine: executable,
    requests: () => readFileSync(log, "utf8")
      .trim()
      .split("\n")
      .filter(Boolean)
      .map((line) => JSON.parse(line) as RecordedRequest),
  };
}

function spec() {
  return defineSpec("share-links", ({ requirement }) => ({
    sharing: requirement("sharing", {
      statement: "Users can securely share documentation",
    }),
  }));
}

// @provenance verification: examples
// @provenance rule: rule_sdk_protocol_handshake
async function rejectsIncompatibleEngineBeforeCommand(): Promise<void> {
  const recorder = recordingEngine({
    info: {
      engine_version: "9.0.0",
      protocol_version: 9,
      state_schema_version: STATE_SCHEMA_VERSION,
      repository: "/project",
    },
    plan: {
      created: 0, updated: 0, moved: 0, retired: 0, conflicts: 0, unchanged: 0,
      resources: [], affected_rules: [],
    },
  });
  configure({ engine: recorder.engine, repository: "/project" });

  await assert.rejects(plan(spec()), /protocol version 9/);
  assert.deepEqual(recorder.requests().map(({ command }) => command), ["info"]);
}

test("an incompatible engine is rejected before the requested command", rejectsIncompatibleEngineBeforeCommand);

test("the adoption SDK rejects a protocol-4 engine before the requested command", async () => {
  const recorder = recordingEngine({
    info: {
      engine_version: "0.1.0",
      protocol_version: 4,
      state_schema_version: STATE_SCHEMA_VERSION,
      repository: "/project",
    },
    plan: {
      created: 0, updated: 0, moved: 0, retired: 0, conflicts: 0, unchanged: 0,
      resources: [], affected_rules: [],
    },
  });
  configure({ engine: recorder.engine, repository: "/project" });

  await assert.rejects(plan(spec()), /protocol version 4/);
  assert.deepEqual(recorder.requests().map(({ command }) => command), ["info"]);
});

async function leavesRepositoryDiscoveryToRust(): Promise<void> {
  const recorder = recordingEngine({
    info: {
      engine_version: "0.1.0",
      protocol_version: 6,
      state_schema_version: STATE_SCHEMA_VERSION,
      repository: "/project",
    },
    plan: {
      declared_by: "spec://typescript",
      created: 0,
      updated: 0,
      moved: 0,
      retired: 0,
      conflicts: 0,
      unchanged: 1,
      resources: [],
      affected_rules: [],
    },
  });
  configure({ engine: recorder.engine });

  await plan(spec());

  for (const request of recorder.requests()) {
    assert.equal(request.args.includes("--repo"), false);
  }
}

test("omitted repository configuration is left for Rust to discover", leavesRepositoryDiscoveryToRust);

test("an engine that cannot start reports the override and reinstall paths", async () => {
  const directory = mkdtempSync(join(tmpdir(), "provenance-missing-engine-"));
  configure({ engine: join(directory, "missing-provenance") });

  await assert.rejects(
    plan(spec()),
    /could not start.*PROVENANCE_BIN.*install the development dependency again/s,
  );
});
