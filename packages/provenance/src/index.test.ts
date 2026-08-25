import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { chmodSync, existsSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

import {
  apply,
  configure,
  defineSpec,
  plan,
  requirement,
  source,
} from "./index.js";
const engine = fileURLToPath(
  new URL("../../../target/debug/provenance", import.meta.url),
);

// Captured from Bun 1.3.14 running `test("...", () => rule.verify(key, callback))`
// against the published dist. Bun eliminates the calling frame of a tail call, so
// the stack holds SDK frames only. `test/bun/tail-call.test.ts` runs the same shape
// under Bun itself. SDK_DIRECTORY stands in for the directory these tests load the
// SDK from, so the recorded frames name the running SDK modules.
const bunTailCallStack = readFileSync(
  fileURLToPath(new URL("../test/bun/tail-call.stack", import.meta.url)),
  "utf8",
).replaceAll("SDK_DIRECTORY", fileURLToPath(new URL(".", import.meta.url)).replace(/[/\\]$/, ""));

function whileStackIs<T>(stack: string, call: () => T): T {
  const prepare = Error.prepareStackTrace;
  Error.prepareStackTrace = () => stack;
  try {
    return call();
  } finally {
    Error.prepareStackTrace = prepare;
  }
}

function repository(): string {
  const repo = mkdtempSync(join(tmpdir(), "provenance-ts-sdk-"));
  execFileSync(engine, [
    "init",
    "--path",
    repo,
    "--scope",
    "default",
    "--path-prefix",
    ".",
  ]);
  return repo;
}

function declareFixture(repo: string) {
  configure({
    engine,
    repository: repo,
    scope: "default",
    owner: "spec://typescript/share-links",
    verificationOwner: "ci://node-test",
  });
  const linear = source("linear:ABC-123", {
    kind: "linear",
    name: "Linear ABC-123",
    url: "https://linear.app/example/issue/ABC-123",
  });
  const sharing = requirement("sharing", {
    id: "req_existing_sharing",
    statement: "Users can securely share documentation",
    sources: [linear],
  });
  const expiry = sharing.rule("expiry", {
    statement: "Share links expire within 30 days",
  });
  return { expiry, sharing };
}

function engineJson(repo: string, args: string[]): unknown {
  return JSON.parse(
    execFileSync(engine, [...args, "--repo", repo, "--format", "json"], {
      encoding: "utf8",
    }),
  );
}

function recordingEngine(responses: Readonly<Record<string, unknown>> = {}): {
  engine: string;
  requests: () => Array<{ command: string; args: string[]; input: unknown }>;
} {
  const directory = mkdtempSync(join(tmpdir(), "provenance-recording-engine-"));
  const executable = join(directory, "engine.mjs");
  const log = join(directory, "requests.jsonl");
  writeFileSync(
    executable,
    `#!/usr/bin/env node
import { appendFileSync, readFileSync } from "node:fs";
const command = process.argv[3];
const args = process.argv.slice(2);
const responses = ${JSON.stringify(responses)};
const source = readFileSync(0, "utf8");
const input = source === "" ? undefined : JSON.parse(source);
appendFileSync(${JSON.stringify(log)}, JSON.stringify({ command, args, input }) + "\\n");
if (Object.hasOwn(responses, command)) {
  process.stdout.write(JSON.stringify(responses[command]));
} else if (command === "info") {
  process.stdout.write(JSON.stringify({
    engine_version: "0.1.0",
    protocol_version: 5,
    state_schema_version: 1,
    repository: "/project",
  }));
} else if (command === "begin-verification") {
  process.stdout.write(JSON.stringify({
    id: "run_" + input.key,
    binding_id: "verification_binding_" + input.key,
    rule_id: "rule_expiry",
    status: "running",
    commit: "0123456789abcdef",
    file: input.file,
    symbol: input.symbol,
  }));
} else {
  process.stdout.write(JSON.stringify({
    id: input.run,
    binding_id: "verification_binding_completed",
    rule_id: "rule_expiry",
    status: input.status,
  }));
}
`,
  );
  chmodSync(executable, 0o755);
  return {
    engine: executable,
    requests: () =>
      (existsSync(log) ? readFileSync(log, "utf8") : "")
        .trim()
        .split("\n")
        .filter(Boolean)
        .map((line) => JSON.parse(line) as { command: string; args: string[]; input: unknown }),
  };
}

test("the callback option-object Requirement keeps an explicit ID", async () => {
  const recorder = recordingEngine({
    apply: {
      declared_by: "spec://typescript/callback-requirement-id",
      created: 1,
      updated: 0,
      moved: 0,
      retired: 0,
      conflicts: 0,
      unchanged: 0,
      resources: [],
    },
  });
  configure({
    engine: recorder.engine,
    owner: "spec://typescript/callback-requirement-id",
  });
  const spec = defineSpec("callback-requirement-id", ({ requirement }) => ({
    canonical: requirement("canonical", {
      id: "req_existing",
      statement: "The canonical Requirement keeps its identity",
    }),
  }));

  await apply(spec);

  const request = recorder.requests().find(({ command }) => command === "apply");
  assert.equal(
    (request?.input as { requirements?: Array<{ id?: string }> }).requirements?.[0]?.id,
    "req_existing",
  );
});

test("verify sends the same durable binding key on repeated runs", async () => {
  const recorder = recordingEngine();
  configure({ engine: recorder.engine, repository: repository() });
  const spec = defineSpec("share-links", ({ requirement }) => {
    const sharing = requirement("sharing", {
      statement: "Users can securely share documentation",
    });
    return {
      expiry: sharing.rule("expiry", {
        statement: "Share links expire within 30 days",
      }),
    };
  });
  const options = {
    method: "examples",
    file: "src/share-links.test.ts",
    symbol: "checkExpiry",
  } as const;

  await spec.handles.expiry.verify("share-link-expiry", () => undefined, options);
  await spec.handles.expiry.verify("share-link-expiry", () => undefined, options);

  const begins = recorder.requests().filter(({ command }) => command === "begin-verification");
  assert.equal(begins.length, 2);
  assert.deepEqual(begins.map(({ input }) => input), [
    {
      declaration: {
        declared_by: "spec://typescript",
        address: ["share-links", "requirement", "sharing", "rule", "expiry"],
      },
      key: "share-link-expiry",
      method: "examples",
      declared_by: "ci://typescript",
      file: "src/share-links.test.ts",
      symbol: "checkExpiry",
    },
    {
      declaration: {
        declared_by: "spec://typescript",
        address: ["share-links", "requirement", "sharing", "rule", "expiry"],
      },
      key: "share-link-expiry",
      method: "examples",
      declared_by: "ci://typescript",
      file: "src/share-links.test.ts",
      symbol: "checkExpiry",
    },
  ]);
});

test("verify sends distinct durable binding keys from one test file", async () => {
  const recorder = recordingEngine();
  configure({ engine: recorder.engine, repository: repository() });
  const spec = defineSpec("share-links", ({ requirement }) => {
    const sharing = requirement("sharing", {
      statement: "Users can securely share documentation",
    });
    return {
      expiry: sharing.rule("expiry", {
        statement: "Share links expire within 30 days",
      }),
    };
  });

  await spec.handles.expiry.verify("maximum-expiry", () => undefined, {
    file: "src/share-links.test.ts",
  });
  await spec.handles.expiry.verify("expired-link", () => undefined, {
    file: "src/share-links.test.ts",
  });

  const keys = recorder.requests()
    .filter(({ command }) => command === "begin-verification")
    .map(({ input }) => (input as { key?: string }).key);
  assert.deepEqual(keys, ["maximum-expiry", "expired-link"]);
});

test("plan sends the finalized spec to the read-only engine command", async () => {
  const recorder = recordingEngine({
    info: {
      engine_version: "0.1.0",
      protocol_version: 5,
      state_schema_version: 1,
      repository: "/project",
    },
    plan: {
      declared_by: "spec://typescript",
      created: 0,
      updated: 1,
      moved: 0,
      retired: 0,
      conflicts: 0,
      unchanged: 1,
      resources: [],
      affected_rules: [],
    },
  });
  configure({ engine: recorder.engine, repository: repository() });
  const spec = defineSpec("share-links", ({ requirement }) => {
    const sharing = requirement("sharing", {
      statement: "Users can securely share documentation",
    });
    return {
      expiry: sharing.rule("expiry", {
        statement: "Share links expire within 14 days",
      }),
    };
  });

  const result = await plan(spec);

  assert.equal(result.updated, 1);
  assert.deepEqual(recorder.requests().map(({ command }) => command), ["info", "plan"]);
  assert.deepEqual((recorder.requests()[1]?.input as { rules: unknown[] }).rules, [
    {
      key: "expiry",
      requirement: "sharing",
      statement: "Share links expire within 14 days",
    },
  ]);
});

test("typed declarations reconcile to canonical Provenance records", async () => {
  const repo = repository();
  const { expiry, sharing } = declareFixture(repo);

  const result = await apply();

  assert.match(expiry.id, /^rule_legacy_sharing_expiry_/);
  assert.equal(sharing.id, "req_existing_sharing");
  assert.equal(result.created, 3);
  const rule = engineJson(repo, [
    "rules",
    "show",
    "--scope",
    "default",
    "--id",
    expiry.id,
  ]) as { statement: string; declared_by: string };
  assert.equal(rule.statement, "Share links expire within 30 days");
  assert.equal(rule.declared_by, "spec://typescript/share-links");
});

test("requirement source order stays unchanged after apply", async () => {
  const repo = repository();
  configure({ engine, repository: repo, owner: "spec://typescript/source-order" });
  const spec = defineSpec("source-order", ({ source, requirement }) => {
    const policy = source("z-policy", {
      kind: "document",
      name: "Policy",
      reference: "docs/policy.md",
    });
    const design = source("a-design", {
      kind: "document",
      name: "Design",
      reference: "docs/design.md",
    });
    const behavior = requirement("behavior", {
      statement: "The behavior follows its accepted sources",
      sources: [policy, design],
    });
    return {
      behavior: behavior.rule("observable", {
        statement: "The accepted behavior remains observable",
      }),
    };
  });

  await apply(spec);
  const result = await plan(spec);

  assert.equal(result.updated, 0);
  assert.equal(result.unchanged, 4);
});

test("omitted declarations retire and later reactivate with the same ids", async () => {
  const repo = repository();
  configure({
    engine,
    repository: repo,
    owner: "spec://typescript/retirement",
  });
  const full = defineSpec("retirement", ({ requirement }) => {
    const sharing = requirement("sharing", {
      statement: "Users can securely share documentation",
    });
    return {
      expiry: sharing.rule("expiry", {
        statement: "Share links expire within 30 days",
      }),
    };
  });
  const empty = defineSpec("retirement", () => ({}));

  const first = await apply(full);
  const ids = first.resources.map(({ id }) => id).sort();
  const preview = await plan(empty);
  assert.equal(preview.retired, 2);
  assert.deepEqual(preview.resources.map(({ state }) => state), ["retired", "retired"]);

  await apply(empty);
  const reactivated = await apply(full);
  assert.equal(reactivated.updated, 2);
  assert.deepEqual(reactivated.resources.map(({ id }) => id).sort(), ids);
});

test("equal local rule keys under different requirements reconcile separately", async () => {
  const repo = repository();
  configure({
    engine,
    repository: repo,
    owner: "spec://typescript/lifecycles",
  });
  const sharing = requirement("sharing", {
    statement: "Users can securely share documentation",
  });
  const shareLinkExpiry = sharing.rule("expiry", {
    statement: "Share links expire within 30 days",
  });
  const sessions = requirement("sessions", {
    statement: "User sessions are time bounded",
  });
  const sessionExpiry = sessions.rule("expiry", {
    statement: "Inactive sessions expire within 24 hours",
  });

  await apply();

  assert.notEqual(shareLinkExpiry.id, sessionExpiry.id);
});

test("defineSpec finalizes pure builders into immutable hierarchical handles", () => {
  configure({ engine: "/engine/must/not/start" });
  let escapedRequirement: { rule(key: string, options: unknown): unknown } | undefined;
  const spec = defineSpec("lifecycles", ({ requirement }) => {
    const sharing = requirement("sharing", {
      statement: "Users can securely share documentation",
    });
    escapedRequirement = sharing;
    const shareLinkExpiry = sharing.rule("expiry", {
      statement: "Share links expire within 30 days",
    });
    const sessions = requirement("sessions", {
      statement: "User sessions are time bounded",
    });
    const sessionExpiry = sessions.rule("expiry", {
      statement: "Inactive sessions expire within 24 hours",
    });
    return { sharing, shareLinkExpiry, sessions, sessionExpiry };
  });

  assert.deepEqual(spec.handles.shareLinkExpiry.address, [
    "lifecycles",
    "requirement",
    "sharing",
    "rule",
    "expiry",
  ]);
  assert.deepEqual(spec.handles.sessionExpiry.address, [
    "lifecycles",
    "requirement",
    "sessions",
    "rule",
    "expiry",
  ]);
  assert.equal(Object.isFrozen(spec), true);
  assert.equal(Object.isFrozen(spec.handles), true);
  assert.equal(Object.isFrozen(spec.handles.sharing), true);
  assert.equal(Object.isFrozen(spec.handles.shareLinkExpiry), true);
  assert.throws(
    () => escapedRequirement?.rule("late", {}),
    /finalized/i,
  );
});

test("immutable rule handles verify through an applied declaration address", async () => {
  const repo = repository();
  configure({
    engine,
    repository: repo,
    owner: "spec://typescript",
    verificationOwner: "ci://node-test",
  });
  const spec = defineSpec("share-links", ({ requirement }) => {
    const sharing = requirement("sharing", {
      statement: "Users can securely share documentation",
    });
    const expiry = sharing.rule("expiry", {
      statement: "Share links expire within 30 days",
    });
    return { sharing, expiry };
  });
  let called = false;

  await assert.rejects(
    spec.handles.expiry.verify(
      "share-link-expiry",
      () => {
        called = true;
      },
      { file: "tests/share-links.test.ts" },
    ),
    /has not been applied/i,
  );
  assert.equal(called, false);
  assert.equal("id" in spec.handles.expiry, false);

  await apply(spec);
  await spec.handles.expiry.verify(
    "share-link-expiry",
    () => {
      called = true;
    },
    { file: "tests/share-links.test.ts" },
  );

  assert.equal(called, true);
  const runs = engineJson(repo, [
    "sdk",
    "verification-runs",
    "--scope",
    "default",
  ]) as Array<{ file?: string; rule_id: string; status: string }>;
  assert.equal(runs.at(-1)?.status, "passed");
  assert.match(runs.at(-1)?.rule_id ?? "", /^rule_share-links_sharing_expiry_/);
  assert.equal(runs.at(-1)?.file, "tests/share-links.test.ts");
});

test("reapplying an address reuses the canonical id already assigned by Rust", async () => {
  const repo = repository();
  configure({ engine, repository: repo, owner: "spec://typescript" });
  const declared = (id?: string) =>
    defineSpec("share-links", ({ requirement }) => {
      const sharing = requirement("sharing", {
        statement: "Users can securely share documentation",
      });
      const expiry = sharing.rule("expiry", {
        id,
        statement: "Share links expire within 30 days",
      });
      return { sharing, expiry };
    });

  const first = await apply(declared("rule_existing_expiry"));
  const second = await apply(declared());
  const firstRule = first.resources.find((resource) => resource.kind === "rule");
  const secondRule = second.resources.find((resource) => resource.kind === "rule");

  assert.equal(firstRule?.id, "rule_existing_expiry");
  assert.equal(secondRule?.id, "rule_existing_expiry");
  assert.equal(second.created, 0);
});

test("verify records a passed Node callback against the imported rule", async () => {
  const repo = repository();
  const { expiry } = declareFixture(repo);

  let called = false;
  await expiry.verify(
    "share-link-expiry",
    () => {
      called = true;
    },
    { file: "tests/share-links.test.ts" },
  );

  assert.equal(called, true);
  const runs = engineJson(repo, [
    "sdk",
    "verification-runs",
    "--scope",
    "default",
    "--rule",
    expiry.id,
  ]) as Array<{ status: string; rule_id: string; file?: string }>;
  assert.equal(runs.at(-1)?.status, "passed");
  assert.equal(runs.at(-1)?.rule_id, expiry.id);
  assert.equal(runs.at(-1)?.file, "tests/share-links.test.ts");
});

test("verify records a failed callback and rethrows the original error", async () => {
  const repo = repository();
  const { expiry } = declareFixture(repo);
  await apply();
  const failure = new Error("expiry assertion failed");

  await assert.rejects(
    expiry.verify(
      "share-link-expiry",
      async () => {
        throw failure;
      },
      { file: "tests/share-links.test.ts" },
    ),
    (error) => error === failure,
  );

  const runs = engineJson(repo, [
    "sdk",
    "verification-runs",
    "--scope",
    "default",
    "--rule",
    expiry.id,
  ]) as Array<{ status: string; error?: string }>;
  assert.equal(runs.at(-1)?.status, "failed");
  assert.match(runs.at(-1)?.error ?? "", /expiry assertion failed/);
});

function shareLinksSpec() {
  return defineSpec("share-links", ({ requirement }) => {
    const sharing = requirement("sharing", {
      statement: "Users can securely share documentation",
    });
    return {
      expiry: sharing.rule("expiry", {
        statement: "Share links expire within 30 days",
      }),
    };
  });
}

function beginVerification(
  requests: Array<{ command: string; args: string[]; input: unknown }>,
): Array<{ file?: string }> {
  return requests
    .filter(({ command }) => command === "begin-verification")
    .map(({ input }) => input as { file?: string });
}

test("verify names import.meta when the runtime hides the calling file", async () => {
  const recorder = recordingEngine();
  configure({ engine: recorder.engine, repository: repository() });
  const spec = shareLinksSpec();
  let called = false;

  const pending = whileStackIs(bunTailCallStack, () =>
    spec.handles.expiry.verify("share-link-expiry", () => {
      called = true;
    }),
  );

  await assert.rejects(pending, /import\.meta\.path/);
  await assert.rejects(pending, /share-link-expiry/);
  assert.equal(called, false);
  assert.deepEqual(beginVerification(recorder.requests()), []);
});

test("verify fails before applying when the stack holds no frames", async () => {
  const recorder = recordingEngine();
  configure({ engine: recorder.engine, repository: repository() });
  const sharing = requirement("sharing", {
    statement: "Users can securely share documentation",
  });
  const expiry = sharing.rule("expiry", { statement: "Share links expire within 30 days" });
  let called = false;

  const pending = whileStackIs("Error", () =>
    expiry.verify("share-link-expiry", () => {
      called = true;
    }),
  );

  await assert.rejects(pending, /import\.meta\.path/);
  assert.equal(called, false);
  assert.deepEqual(recorder.requests().map(({ command }) => command), []);
});

test("verify accepts import.meta as the file the test runs in", async () => {
  const recorder = recordingEngine();
  configure({ engine: recorder.engine, repository: repository() });
  const spec = shareLinksSpec();

  await whileStackIs(bunTailCallStack, () =>
    spec.handles.expiry.verify("share-link-expiry", () => undefined, import.meta),
  );

  assert.deepEqual(
    beginVerification(recorder.requests()).map(({ file }) => file),
    [fileURLToPath(import.meta.url)],
  );
});

test("verify prefers the module URL over Bun's bare file name", async () => {
  const recorder = recordingEngine();
  configure({ engine: recorder.engine, repository: repository() });
  const spec = shareLinksSpec();
  // Bun's import.meta carries `url` alongside a `file` holding the file name alone.
  const bunImportMeta = { url: import.meta.url, file: "index.test.js" };

  await whileStackIs(bunTailCallStack, () =>
    spec.handles.expiry.verify("share-link-expiry", () => undefined, bunImportMeta),
  );

  assert.deepEqual(
    beginVerification(recorder.requests()).map(({ file }) => file),
    [fileURLToPath(import.meta.url)],
  );
});

test("verify accepts a module URL as the stated file", async () => {
  const recorder = recordingEngine();
  configure({ engine: recorder.engine, repository: repository() });
  const spec = shareLinksSpec();

  await whileStackIs(bunTailCallStack, () =>
    spec.handles.expiry.verify("share-link-expiry", () => undefined, {
      file: import.meta.url,
      method: "property",
    }),
  );

  assert.deepEqual(
    beginVerification(recorder.requests()).map(({ file }) => file),
    [fileURLToPath(import.meta.url)],
  );
});
