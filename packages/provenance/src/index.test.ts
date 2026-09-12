import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import test, { type TestContext } from "node:test";
import { startFixtureHost } from "../scripts/fixture-host.js";
import { recordingHost } from "./http-recorder.test-helper.js";
import { OperationError } from "./client.js";
import { PROTOCOL_VERSION } from "./generated/client.js";
import { STATE_SCHEMA_VERSION } from "./protocol.js";

import {
  apply,
  configure,
  defineSpec,
  plan,
  requirement,
  source,
} from "./index.js";
const engine = fileURLToPath(
  new URL(`../../../target/debug/provenance${process.platform === "win32" ? ".exe" : ""}`, import.meta.url),
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

function localFiles(t: TestContext): string {
  const root = mkdtempSync(join(tmpdir(), "provenance-ts-sdk-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  for (const directory of ["tests", "src"]) {
    mkdirSync(join(root, directory));
    writeFileSync(join(root, directory, "share-links.test.ts"), "// Real verification fixture.\n");
  }
  return root;
}

async function repository(t: TestContext) {
  const repo = localFiles(t);
  execFileSync(engine, ["init", "--path", repo, "--scope", "default", "--path-prefix", "."], { stdio: "pipe" });
  const host = await startFixtureHost({ root: repo, repositoryId: "index-fixture" });
  t.after(() => host.close());
  return {
    repo,
    settings: {
      endpoint: host.environment.PROVENANCE_ENDPOINT,
      bearer: host.environment.PROVENANCE_TOKEN,
      repositoryId: host.environment.PROVENANCE_REPOSITORY_ID,
      localRoot: repo,
      scope: "default",
    },
  };
}

function declareFixture(settings: Parameters<typeof configure>[0]) {
  configure({
    ...settings,
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


test("the callback option-object Requirement keeps an explicit ID", async (t) => {
  const recorder = await recordingHost({
    apply: {
      declared_by: "spec://typescript/callback-requirement-id",
      created: 1,
      updated: 0,
      moved: 0,
      deleted: 0,
      conflicts: 0,
      unchanged: 0,
      resources: [],
    },
  });
  t.after(() => recorder.close());
  configure({
    ...recorder.settings,
    localRoot: localFiles(t),
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

test("verify sends the same durable binding key on repeated runs", async (t) => {
  const recorder = await recordingHost();
  t.after(() => recorder.close());
  configure({ ...recorder.settings, localRoot: localFiles(t) });
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

test("verify sends distinct durable binding keys from one test file", async (t) => {
  const recorder = await recordingHost();
  t.after(() => recorder.close());
  configure({ ...recorder.settings, localRoot: localFiles(t) });
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

test("plan sends the finalized spec to the read-only HTTP operation", async (t) => {
  const recorder = await recordingHost({
    info: {
      engine_version: "0.1.0",
      protocol_version: PROTOCOL_VERSION,
      state_schema_version: STATE_SCHEMA_VERSION,
      repository: "/project",
    },
    plan: {
      declared_by: "spec://typescript",
      created: 0,
      updated: 1,
      moved: 0,
      deleted: 0,
      conflicts: 0,
      unchanged: 1,
      resources: [],
      affected_rules: [],
    },
  });
  t.after(() => recorder.close());
  configure({ ...recorder.settings, localRoot: localFiles(t) });
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
  assert.deepEqual(recorder.requests().map(({ command }) => command), ["plan"]);
  assert.deepEqual((recorder.requests()[0]?.input as { rules: unknown[] }).rules, [
    {
      key: "expiry",
      requirement: "sharing",
      statement: "Share links expire within 14 days",
    },
  ]);
});

test("typed declarations reconcile to canonical Provenance records", async (t) => {
  const { repo, settings } = await repository(t);
  const { expiry, sharing } = declareFixture(settings);

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

test("requirement source order stays unchanged after apply", async (t) => {
  const { repo, settings } = await repository(t);
  configure({ ...settings, owner: "spec://typescript/source-order" });
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

test("omitted declarations are deleted and later recreated with the same ids", async (t) => {
  const { repo, settings } = await repository(t);
  configure({
    ...settings,
    owner: "spec://typescript/deletion",
  });
  const full = defineSpec("deletion", ({ requirement }) => {
    const sharing = requirement("sharing", {
      statement: "Users can securely share documentation",
    });
    return {
      expiry: sharing.rule("expiry", {
        statement: "Share links expire within 30 days",
      }),
    };
  });
  const empty = defineSpec("deletion", () => ({}));

  const first = await apply(full);
  const ids = first.resources.map(({ id }) => id).sort();
  const preview = await plan(empty);
  assert.equal(preview.deleted, 2);
  assert.deepEqual(preview.resources.map(({ state }) => state), ["deleted", "deleted"]);

  await apply(empty);
  const reactivated = await apply(full);
  assert.equal(reactivated.created, 2);
  assert.deepEqual(reactivated.resources.map(({ id }) => id).sort(), ids);
});

test("equal local rule keys under different requirements reconcile separately", async (t) => {
  const { repo, settings } = await repository(t);
  configure({
    ...settings,
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
  configure({ endpoint: "http://127.0.0.1:0", repositoryId: "pure-builders" });
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

test("immutable rule handles verify through an applied declaration address", async (t) => {
  const { repo, settings } = await repository(t);
  configure({
    ...settings,
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
    (error) => error instanceof OperationError && error.failure.error.kind === "invalid_verification_target",
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

test("reapplying an address reuses the canonical id already assigned by Rust", async (t) => {
  const { repo, settings } = await repository(t);
  configure({ ...settings, owner: "spec://typescript" });
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

test("verify records a passed Node callback against the imported rule", async (t) => {
  const { repo, settings } = await repository(t);
  const { expiry } = declareFixture(settings);

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

test("verify records a failed callback and rethrows the original error", async (t) => {
  const { repo, settings } = await repository(t);
  const { expiry } = declareFixture(settings);
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
  requests: Array<{ command: string; input: unknown }>,
): Array<{ file?: string }> {
  return requests
    .filter(({ command }) => command === "begin-verification")
    .map(({ input }) => input as { file?: string });
}

test("verify names import.meta when the runtime hides the calling file", async (t) => {
  const recorder = await recordingHost();
  t.after(() => recorder.close());
  configure({ ...recorder.settings, localRoot: localFiles(t) });
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

test("verify fails before applying when the stack holds no frames", async (t) => {
  const recorder = await recordingHost();
  t.after(() => recorder.close());
  configure({ ...recorder.settings, localRoot: localFiles(t) });
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

test("verify accepts import.meta as the file the test runs in", async (t) => {
  const recorder = await recordingHost();
  t.after(() => recorder.close());
  configure({ ...recorder.settings, localRoot: fileURLToPath(new URL(".", import.meta.url)) });
  const spec = shareLinksSpec();

  await whileStackIs(bunTailCallStack, () =>
    spec.handles.expiry.verify("share-link-expiry", () => undefined, import.meta),
  );

  assert.deepEqual(
    beginVerification(recorder.requests()).map(({ file }) => file),
    ["index.test.js"],
  );
});

test("verify prefers the module URL over Bun's bare file name", async (t) => {
  const recorder = await recordingHost();
  t.after(() => recorder.close());
  configure({ ...recorder.settings, localRoot: fileURLToPath(new URL(".", import.meta.url)) });
  const spec = shareLinksSpec();
  // Bun's import.meta carries `url` alongside a `file` holding the file name alone.
  const bunImportMeta = { url: import.meta.url, file: "index.test.js" };

  await whileStackIs(bunTailCallStack, () =>
    spec.handles.expiry.verify("share-link-expiry", () => undefined, bunImportMeta),
  );

  assert.deepEqual(
    beginVerification(recorder.requests()).map(({ file }) => file),
    ["index.test.js"],
  );
});

test("verify accepts a module URL as the stated file", async (t) => {
  const recorder = await recordingHost();
  t.after(() => recorder.close());
  configure({ ...recorder.settings, localRoot: fileURLToPath(new URL(".", import.meta.url)) });
  const spec = shareLinksSpec();

  await whileStackIs(bunTailCallStack, () =>
    spec.handles.expiry.verify("share-link-expiry", () => undefined, {
      file: import.meta.url,
      method: "property",
    }),
  );

  assert.deepEqual(
    beginVerification(recorder.requests()).map(({ file }) => file),
    ["index.test.js"],
  );
});
