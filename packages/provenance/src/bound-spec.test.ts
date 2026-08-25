import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { chmodSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

import { apply, configure, defineSpec, plan } from "./index.js";
import { startWorkflow } from "./implementation-target.test-helper.js";

const engine = fileURLToPath(
  new URL("../../../target/debug/provenance", import.meta.url),
);

function repository(): string {
  const repo = mkdtempSync(join(tmpdir(), "provenance-bound-sdk-"));
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

function recordingEngine(): {
  engine: string;
  requests: () => Array<{ command: string; input: unknown }>;
} {
  const directory = mkdtempSync(join(tmpdir(), "provenance-bound-recorder-"));
  const executable = join(directory, "engine.mjs");
  const log = join(directory, "requests.jsonl");
  writeFileSync(
    executable,
    `#!/usr/bin/env node
import { appendFileSync, readFileSync } from "node:fs";
const command = process.argv[3];
const source = readFileSync(0, "utf8");
const input = source === "" ? undefined : JSON.parse(source);
appendFileSync(${JSON.stringify(log)}, JSON.stringify({ command, input }) + "\\n");
if (command === "info") process.stdout.write(JSON.stringify({
  engine_version: "0.1.0", protocol_version: 5, state_schema_version: 1, repository: "/project"
}));
else if (command === "begin-verification") process.stdout.write(JSON.stringify({
  id: "run_1", binding_id: "binding_1", rule_id: "rule_1", status: "running"
}));
else if (command === "complete-verification") process.stdout.write(JSON.stringify({
  id: "run_1", binding_id: "binding_1", rule_id: "rule_1", status: "passed"
}));
else process.stdout.write(JSON.stringify({
  declared_by: "spec://typescript", created: 0, updated: 0, moved: 0,
  retired: 0, conflicts: 0, unchanged: 0,
  resources: [], affected_rules: []
}));
`,
  );
  chmodSync(executable, 0o755);
  return {
    engine: executable,
    requests: () => {
      try {
        return readFileSync(log, "utf8")
          .trim()
          .split("\n")
          .filter(Boolean)
          .map((line) => JSON.parse(line) as { command: string; input: unknown });
      } catch {
        return [];
      }
    },
  };
}

function engineJson(repo: string, args: string[]): unknown {
  return JSON.parse(
    execFileSync(engine, [...args, "--repo", repo, "--format", "json"], {
      encoding: "utf8",
    }),
  );
}

test("a spec-bound Rule is its own immutable verification handle", async () => {
  const recorder = recordingEngine();
  configure({ engine: recorder.engine, owner: "spec://typescript/bound" });
  const provenance = defineSpec("share-links");
  const policy = provenance
    .source("policy")
    .name("Sharing policy")
    .document("docs/policy.md");
  const sharing = provenance
    .requirement("sharing")
    .statement("Users can securely share documentation")
    .description("Controls for shared documentation")
    .from(policy);
  const expiry = sharing
    .rule("expiry")
    .statement("Share links expire within 30 days");
  const spec = provenance.build(sharing.rules(expiry));

  assert.equal(Object.isFrozen(expiry), true);
  assert.equal(Object.isFrozen(spec), true);
  assert.deepEqual(recorder.requests(), []);

  await apply(spec);
  await expiry.verify("share-link-expiry", () => undefined);

  const begin = recorder.requests().find(({ command }) => command === "begin-verification");
  assert.deepEqual(begin?.input, {
    declaration: {
      declared_by: "spec://typescript/bound",
      address: ["share-links", "requirement", "sharing", "rule", "expiry"],
    },
    key: "share-link-expiry",
    method: "examples",
    declared_by: "ci://typescript",
    file: fileURLToPath(import.meta.url),
  });
});

test("a spec-bound Requirement serializes an immutable explicit ID", async () => {
  const recorder = recordingEngine();
  configure({ engine: recorder.engine, owner: "spec://typescript/bound-requirement-id" });
  const provenance = defineSpec("bound-requirement-id");
  const draft = provenance.requirement("canonical");
  const identified = draft.id("req_existing");
  const stated = identified.statement("The canonical Requirement keeps its identity");

  assert.notEqual(draft, identified);
  assert.notEqual(identified, stated);
  await apply(provenance.build(stated));

  const request = recorder.requests().find(({ command }) => command === "apply");
  assert.deepEqual((request?.input as { requirements?: unknown }).requirements, [
    {
      key: "canonical",
      id: "req_existing",
      statement: "The canonical Requirement keeps its identity",
      sources: [],
    },
  ]);
});

test("spec-bound declarations serialize exact unowned adoption targets", async () => {
  const recorder = recordingEngine();
  configure({ engine: recorder.engine, owner: "spec://typescript/bound-adoption" });
  const provenance = defineSpec("bound-adoption");
  const policy = provenance
    .source("policy")
    .adoptUnowned("source_existing")
    .document("docs/policy.md");
  const enforcement = provenance
    .rule("enforcement")
    .adoptUnowned("rule_existing")
    .statement("The migration keeps the canonical Rule");
  const canonical = provenance
    .requirement("canonical")
    .adoptUnowned("req_existing")
    .statement("The canonical Requirement keeps its identity")
    .from(policy)
    .rules(enforcement);

  await apply(provenance.build(canonical));

  const input = recorder.requests().find(({ command }) => command === "apply")?.input as {
    adopt_unowned?: unknown;
  };
  assert.deepEqual(input.adopt_unowned, [
    { kind: "source", id: "source_existing" },
    { kind: "requirement", id: "req_existing" },
    { kind: "rule", id: "rule_existing" },
  ]);

  const ordinary = provenance
    .requirement("ordinary")
    .adoptUnowned("req_old")
    .id("req_existing")
    .statement("Ordinary identity selection does not request adoption")
    .from(policy.id("source_existing"))
    .rules(enforcement.id("rule_existing"));
  await apply(provenance.build(ordinary));
  const ordinaryInput = recorder.requests().filter(({ command }) => command === "apply").at(-1)
    ?.input as { adopt_unowned?: unknown };
  assert.equal(ordinaryInput.adopt_unowned, undefined);
});

test("spec-bound declarations adopt exact unowned engine records", async () => {
  const repo = repository();
  configure({ engine, repository: repo, owner: "spec://typescript/bound-adoption-runtime" });
  execFileSync(engine, [
    "sources",
    "create",
    "--repo",
    repo,
    "--scope",
    "default",
    "--id",
    "source_existing",
    "--name",
    "policy",
    "--source-type",
    "document",
    "--reference",
    "docs/policy.md",
  ]);
  execFileSync(engine, [
    "requirements",
    "create",
    "--repo",
    repo,
    "--scope",
    "default",
    "--id",
    "req_existing",
    "--statement",
    "The canonical Requirement keeps its identity",
  ]);
  execFileSync(engine, [
    "requirements",
    "source-ref",
    "add",
    "--repo",
    repo,
    "--scope",
    "default",
    "--requirement-id",
    "req_existing",
    "--source-id",
    "source_existing",
  ]);
  execFileSync(engine, [
    "rules",
    "create",
    "--repo",
    repo,
    "--scope",
    "default",
    "--id",
    "rule_existing",
    "--requirement-id",
    "req_existing",
    "--statement",
    "The canonical Rule keeps its identity",
  ]);
  const provenance = defineSpec("bound-adoption-runtime");
  const policy = provenance
    .source("policy")
    .adoptUnowned("source_existing")
    .document("docs/policy.md");
  const enforcement = provenance
    .rule("enforcement")
    .adoptUnowned("rule_existing")
    .statement("The canonical Rule keeps its identity");
  const canonical = provenance
    .requirement("canonical")
    .adoptUnowned("req_existing")
    .statement("The canonical Requirement keeps its identity")
    .from(policy)
    .rules(enforcement);
  const spec = provenance.build(canonical);

  const preview = await plan(spec);
  assert.equal(preview.created, 0);
  assert.equal(preview.conflicts, 0);
  const applied = await apply(spec);
  assert.deepEqual(
    applied.resources.map(({ id }) => id).sort(),
    ["req_existing", "rule_existing", "source_existing"],
  );
  const replay = await plan(spec);
  assert.equal(replay.unchanged, 3);
});

test("a spec-scoped Rule materializes once for several Requirements", async () => {
  const repo = repository();
  configure({ engine, repository: repo, owner: "spec://typescript/bound-shared" });
  const provenance = defineSpec("lifecycles");
  const policy = provenance.source("access-policy").document("docs/access-policy.md");
  const expiry = provenance.rule("expiry").statement("Authenticated access expires");
  const sharing = provenance
    .requirement("sharing")
    .statement("Share links are time bounded")
    .from(policy)
    .rules(expiry);
  const sessions = provenance
    .requirement("sessions")
    .statement("Sessions are time bounded")
    .from(policy)
    .rules(expiry);

  const result = await apply(provenance.build(sharing, sessions));
  const rules = result.resources.filter(({ kind }) => kind === "rule");

  assert.equal(result.resources.filter(({ kind }) => kind === "source").length, 1);
  assert.equal(rules.length, 1);
  assert.deepEqual(rules[0]?.address, ["lifecycles", "rule", "expiry"]);
  const edges = readFileSync(join(repo, ".provenance/state/edges/edges-00.jsonl"), "utf8")
    .trim()
    .split("\n")
    .map((line) => JSON.parse(line) as { edge_type: string; to_id: string });
  assert.equal(
    edges.filter(({ edge_type, to_id }) => edge_type === "produces" && to_id === rules[0]?.id)
      .length,
    2,
  );
});

test("source names and Requirement descriptions are immutable canonical metadata", async () => {
  const repo = repository();
  configure({ engine, repository: repo, owner: "spec://typescript/bound-metadata" });
  const provenance = defineSpec("metadata");
  const sourceDraft = provenance.source("policy").document("docs/policy.md");
  const namedSource = sourceDraft.name("Security policy");
  const requirementBase = provenance
    .requirement("sharing")
    .statement("Users can securely share documentation");
  const requirementDraft = requirementBase
    .from(sourceDraft)
    .description("The first canonical description");
  const revisedRequirement = requirementBase
    .from(namedSource)
    .description("The revised canonical description");

  await apply(provenance.build(requirementDraft));
  const result = await apply(provenance.build(revisedRequirement));

  assert.notEqual(sourceDraft, namedSource);
  assert.notEqual(requirementDraft, revisedRequirement);
  assert.equal(Object.isFrozen(namedSource), true);
  assert.equal(Object.isFrozen(revisedRequirement), true);
  assert.deepEqual(
    result.resources.find(({ kind }) => kind === "source")?.changes,
    [{ field: "name", before: "policy", after: "Security policy" }],
  );
  assert.deepEqual(
    result.resources.find(({ kind }) => kind === "requirement")?.changes,
    [
      {
        field: "description",
        before: "The first canonical description",
        after: "The revised canonical description",
      },
    ],
  );
});

test("one Requirement keeps a spec-scoped implemented Rule at its exact root address", async () => {
  configure({
    engine,
    repository: fileURLToPath(new URL("../../..", import.meta.url)),
    owner: "spec://typescript/bound-one-parent",
  });
  const provenance = defineSpec("bound-one-parent");
  const expiry = provenance
    .rule("expiry")
    .statement("Authenticated access expires")
    .implementedBy(startWorkflow);
  const sharing = provenance
    .requirement("sharing")
    .statement("Shares are time bounded")
    .rules(expiry);

  const result = await plan(provenance.build(sharing));
  const materialized = result.resources.find(({ kind }) => kind === "rule")!;

  assert.deepEqual(materialized.address, ["bound-one-parent", "rule", "expiry"]);
  assert.equal(materialized.parent, undefined);
  assert.equal(result.implementation_bindings?.[0]?.rule_id, materialized.id);
  assert.match(
    result.implementation_bindings?.[0]?.file ?? "",
    /implementation-target\.test-helper\.js$/,
  );
});

test("equal requirement-local Rule keys keep distinct addresses", async () => {
  const repo = repository();
  configure({ engine, repository: repo, owner: "spec://typescript/bound-local" });
  const provenance = defineSpec("lifecycles");
  const sharing = provenance.requirement("sharing").statement("Shares expire");
  const sessions = provenance.requirement("sessions").statement("Sessions expire");
  const shareExpiry = sharing.rule("expiry").statement("Share links expire");
  const sessionExpiry = sessions.rule("expiry").statement("Sessions expire");

  const result = await apply(
    provenance.build(sharing.rules(shareExpiry), sessions.rules(sessionExpiry)),
  );

  assert.deepEqual(
    result.resources
      .filter(({ kind }) => kind === "rule")
      .map(({ address }) => address)
      .sort(),
    [
      ["lifecycles", "requirement", "sessions", "rule", "expiry"],
      ["lifecycles", "requirement", "sharing", "rule", "expiry"],
    ],
  );
});

test("a requirement rejects another Requirement's local Rule", () => {
  const provenance = defineSpec("lifecycles");
  const sharing = provenance.requirement("sharing").statement("Shares expire");
  const sessions = provenance.requirement("sessions").statement("Sessions expire");
  const expiry = sharing.rule("expiry").statement("Share links expire");

  assert.throws(
    () => sessions.rules(expiry as never),
    /local Rule `expiry` belongs to Requirement `sharing`, not `sessions`/,
  );
});

test("a local Rule attaches to another immutable snapshot of its Requirement", () => {
  const provenance = defineSpec("lifecycles");
  const sharingDraft = provenance.requirement("sharing");
  const expiry = sharingDraft.rule("expiry").statement("Share links expire");
  const sharing = sharingDraft.statement("Shares expire");

  assert.doesNotThrow(() => provenance.build(sharing.rules(expiry)));
});

test("a local Rule rejects another Requirement declaration with the same key", () => {
  const provenance = defineSpec("lifecycles");
  const first = provenance.requirement("sharing").statement("Shares expire");
  const second = provenance.requirement("sharing").statement("Links are time bounded");
  const expiry = first.rule("expiry").statement("Share links expire");

  assert.throws(
    () => second.rules(expiry),
    /local Rule `expiry` belongs to another `sharing` Requirement declaration/,
  );
});

test("separate spec-scoped Rule declarations cannot claim one address", () => {
  const provenance = defineSpec("lifecycles");
  const sharing = provenance.requirement("sharing").statement("Shares expire");
  const sessions = provenance.requirement("sessions").statement("Sessions expire");
  const shareExpiry = provenance.rule("expiry").statement("Share links expire");
  const sessionExpiry = provenance.rule("expiry").statement("Sessions expire");

  assert.throws(
    () => provenance.build(sharing.rules(shareExpiry), sessions.rules(sessionExpiry)),
    /distinct Rule declarations claim address `lifecycles \/ rule \/ expiry`/,
  );
});

test("structured declaration addresses cannot collide through display delimiters", () => {
  const provenance = defineSpec("structured-addresses");
  const first = provenance
    .requirement("alpha / rule / beta")
    .statement("The first Requirement holds");
  const second = provenance
    .requirement("alpha")
    .statement("The second Requirement holds");
  const firstRule = first.rule("gamma").statement("The first Rule holds");
  const secondRule = second
    .rule("beta / rule / gamma")
    .statement("The second Rule holds");

  assert.doesNotThrow(() =>
    provenance.build(first.rules(firstRule), second.rules(secondRule)),
  );
});

test("one Requirement cannot contain shared and local Rules with the same key", () => {
  const provenance = defineSpec("lifecycles");
  const sharing = provenance.requirement("sharing").statement("Shares expire");
  const shared = provenance.rule("expiry").statement("Authenticated access expires");
  const local = sharing.rule("expiry").statement("Share links expire");

  assert.throws(
    () => provenance.build(sharing.rules(shared, local)),
    /distinct Rule declarations with key `expiry` collide under Requirement `sharing`/,
  );
});

test("a build rejects declarations from another same-key context", () => {
  const first = defineSpec("lifecycles");
  const second = defineSpec("lifecycles");
  const requirement = second.requirement("sharing").statement("Shares expire");

  assert.throws(
    () => first.build(requirement as never),
    /Requirement `sharing` belongs to another spec authoring context/,
  );
});

test("spec-bound verify rejects an unapplied Rule before running its callback", async () => {
  const repo = repository();
  configure({ engine, repository: repo, owner: "spec://typescript/bound-unapplied" });
  const provenance = defineSpec("unapplied");
  const sharing = provenance.requirement("sharing").statement("Shares expire");
  const expiry = sharing.rule("expiry").statement("Share links expire");
  provenance.build(sharing.rules(expiry));
  let called = false;

  await assert.rejects(
    expiry.verify(
      "share-expiry",
      () => {
        called = true;
      },
      { file: "tests/share-links.test.ts" },
    ),
    /has not been applied/i,
  );
  assert.equal(called, false);
});

test("spec-bound verify records failure and rethrows the callback error", async () => {
  const repo = repository();
  configure({ engine, repository: repo, owner: "spec://typescript/bound-failed" });
  const provenance = defineSpec("failed-verification");
  const sharing = provenance.requirement("sharing").statement("Shares expire");
  const expiry = sharing.rule("expiry").statement("Share links expire");
  await apply(provenance.build(sharing.rules(expiry)));
  const failure = new Error("bound expiry assertion failed");

  await assert.rejects(
    expiry.verify(
      "share-expiry",
      () => {
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
  ]) as Array<{ status: string; error?: string }>;
  assert.equal(runs.at(-1)?.status, "failed");
  assert.match(runs.at(-1)?.error ?? "", /bound expiry assertion failed/);
});
