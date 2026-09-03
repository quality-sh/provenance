import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { chmodSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

import {
  apply,
  configure,
  defineSpec,
  requirement,
  rule,
  source,
} from "./index.js";
import * as implementationTargets from "./implementation-target.test-helper.js";
import {
  startWorkflow,
  WorkflowRunner,
} from "./implementation-target.test-helper.js";

const engine = fileURLToPath(
  new URL("../../../target/debug/provenance", import.meta.url),
);

function repository(): string {
  const repo = mkdtempSync(join(tmpdir(), "provenance-fluent-sdk-"));
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

/** One canonical record as the state shard holds it. */
interface WrittenRecord {
  id: string;
  requirement_ids?: string[];
  resolution_ids?: string[];
  refines?: string;
  depends_on?: string[];
  supersedes?: string[];
  spawned_by?: string;
}

function readRecords(repo: string, relative: string): WrittenRecord[] {
  return readFileSync(join(repo, ".provenance/state/scopes/default", relative), "utf8")
    .trim()
    .split("\n")
    .filter(Boolean)
    .map((line) => JSON.parse(line) as WrittenRecord);
}

/** A requirement and a resolution the writers already hold, for the
 *  declarations that name a resolution by canonical id. */
function seedResolution(repo: string): void {
  execFileSync(engine, [
    "requirements",
    "create",
    "--repo",
    repo,
    "--scope",
    "default",
    "--id",
    "req_seed",
    "--statement",
    "The seed requirement stands",
  ]);
  execFileSync(engine, [
    "resolutions",
    "create",
    "--repo",
    repo,
    "--scope",
    "default",
    "--id",
    "res_seed",
    "--title",
    "Seed decision",
    "--requirement-id",
    "req_seed",
    "--position",
    "Adopt",
    "--rationale",
    "Seeds the relations",
    "--status",
    "proposed",
  ]);
}

function recordingEngine(): {
  engine: string;
  requests: () => Array<{ command: string; input: unknown }>;
} {
  const directory = mkdtempSync(join(tmpdir(), "provenance-fluent-recorder-"));
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
  engine_version: "0.1.0", protocol_version: 6, state_schema_version: 1, repository: "/project"
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

test("fluent declarations and their finalized handles are immutable", () => {
  const recorder = recordingEngine();
  configure({ engine: recorder.engine });
  const sourceDraft = source("policy");
  const policy = sourceDraft.document("docs/policy.md");
  const ruleDraft = rule("expiry");
  const identified = ruleDraft.id("rule_expiry");
  const statedRule = identified.statement("Share links expire within 30 days");
  const expiry = statedRule.implementedBy(startWorkflow);
  const requirementDraft = requirement("sharing");
  const stated = requirementDraft.statement("Users can securely share documentation");
  const sourced = stated.from(policy);
  const sharing = sourced.rules(expiry);
  const specDraft = defineSpec("share-links");
  const withSources = specDraft.sources(policy);
  const withRequirements = withSources.requirements(sharing);
  const spec = withRequirements.build();

  assert.notEqual(sourceDraft, policy);
  assert.notEqual(ruleDraft, expiry);
  assert.notEqual(ruleDraft, identified);
  assert.notEqual(statedRule, expiry);
  assert.equal(expiry.explicitId, "rule_expiry");
  assert.equal(expiry.implementation?.symbol, "startWorkflow");
  assert.match(expiry.implementation?.file ?? "", /implementation-target\.test-helper\.js$/);
  assert.notEqual(requirementDraft, stated);
  assert.notEqual(stated, sourced);
  assert.notEqual(sourced, sharing);
  assert.notEqual(specDraft, withSources);
  assert.notEqual(withSources, withRequirements);
  for (const value of [policy, expiry, sharing, spec, spec.handles]) {
    assert.equal(Object.isFrozen(value), true);
  }
  assert.equal(Object.isFrozen(policy.declaration), true);
  assert.deepEqual(spec.handles.requirements.sharing.rules.expiry.address, [
    "share-links",
    "requirement",
    "sharing",
    "rule",
    "expiry",
  ]);
  assert.deepEqual(recorder.requests(), []);
});

test("exported classes bind without construction or runtime inspection", () => {
  const direct = rule("direct-class").implementedBy(WorkflowRunner);
  const namespaced = rule("namespaced-class").implementedBy(
    implementationTargets.WorkflowRunner,
  );

  assert.equal(WorkflowRunner.constructions, 0);
  assert.equal(direct.implementation?.symbol, "WorkflowRunner");
  assert.equal(namespaced.implementation?.symbol, "WorkflowRunner");
});

test("top-level fluent declarations author source names and Requirement descriptions", async () => {
  const recorder = recordingEngine();
  configure({ engine: recorder.engine, owner: "spec://typescript/fluent-metadata" });
  const policyDraft = source("policy");
  const policyDocument = policyDraft.document("docs/policy.md");
  const namedPolicy = policyDocument.name("Security policy");
  const requirementDraft = requirement("sharing")
    .description("Covers externally shared documentation");
  const statedRequirement = requirementDraft
    .statement("Users can securely share documentation");
  const expiry = rule("expiry").statement("Shared links expire");
  const sharing = statedRequirement.from(namedPolicy).rules(expiry);
  const spec = defineSpec("fluent-metadata")
    .sources(namedPolicy)
    .requirements(sharing)
    .build();

  assert.notEqual(policyDraft, namedPolicy);
  assert.notEqual(requirementDraft, sharing);
  assert.deepEqual(recorder.requests(), []);

  await apply(spec);

  const request = recorder.requests().find(({ command }) => command === "apply");
  assert.deepEqual(request?.input, {
    schema_version: 1,
    spec: "fluent-metadata",
    declared_by: "spec://typescript/fluent-metadata",
    sources: [
      {
        key: "policy",
        name: "Security policy",
        kind: "document",
        reference: "docs/policy.md",
      },
    ],
    requirements: [
      {
        key: "sharing",
        statement: "Users can securely share documentation",
        description: "Covers externally shared documentation",
        sources: ["policy"],
      },
    ],
    rules: [
      {
        key: "expiry",
        requirements: ["sharing"],
        statement: "Shared links expire",
      },
    ],
  });
});

test("top-level fluent Requirements serialize an immutable explicit ID", async () => {
  const recorder = recordingEngine();
  configure({ engine: recorder.engine, owner: "spec://typescript/requirement-id" });
  const draft = requirement("canonical");
  const identified = draft.id("req_existing");
  const stated = identified.statement("The canonical Requirement keeps its identity");
  const spec = defineSpec("requirement-id").requirements(stated).build();

  assert.notEqual(draft, identified);
  assert.notEqual(identified, stated);
  await apply(spec);

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

test("top-level fluent declarations serialize exact unowned adoption targets", async () => {
  const recorder = recordingEngine();
  configure({ engine: recorder.engine, owner: "spec://typescript/adoption" });
  const policyDraft = source("policy");
  const policy = policyDraft
    .adoptUnowned("source_existing")
    .document("docs/policy.md");
  const ruleDraft = rule("enforcement");
  const enforcement = ruleDraft
    .adoptUnowned("rule_existing")
    .statement("The migration keeps the canonical Rule");
  const requirementDraft = requirement("canonical");
  const canonical = requirementDraft
    .adoptUnowned("req_existing")
    .statement("The canonical Requirement keeps its identity")
    .from(policy)
    .rules(enforcement);
  const spec = defineSpec("adoption").requirements(canonical).build();

  assert.notEqual(policyDraft, policy);
  assert.notEqual(ruleDraft, enforcement);
  assert.notEqual(requirementDraft, canonical);
  await apply(spec);

  const input = recorder.requests().find(({ command }) => command === "apply")?.input as {
    adopt_unowned?: unknown;
    sources?: Array<{ id?: string }>;
    requirements?: Array<{ id?: string }>;
    rules?: Array<{ id?: string }>;
  };
  assert.deepEqual(input.adopt_unowned, [
    { kind: "source", id: "source_existing" },
    { kind: "requirement", id: "req_existing" },
    { kind: "rule", id: "rule_existing" },
  ]);
  assert.equal(input.sources?.[0]?.id, "source_existing");
  assert.equal(input.requirements?.[0]?.id, "req_existing");
  assert.equal(input.rules?.[0]?.id, "rule_existing");
});

test("build collects Sources referenced by Requirements", async () => {
  const recorder = recordingEngine();
  configure({ engine: recorder.engine, owner: "spec://typescript/collected-source" });
  const spec = defineSpec("collected-source")
    .requirements(
      requirement("sharing")
        .statement("Users can securely share documentation")
        .from(source("policy").name("Sharing policy").document("docs/policy.md"))
        .rules(rule("expiry").statement("Share links expire")),
    )
    .build();

  assert.deepEqual(recorder.requests(), []);
  await apply(spec);

  const request = recorder.requests().find(({ command }) => command === "apply");
  assert.deepEqual((request?.input as { sources?: unknown }).sources, [
    {
      key: "policy",
      name: "Sharing policy",
      kind: "document",
      reference: "docs/policy.md",
    },
  ]);
});

test("a fluent Source declares a supported non-document kind", async () => {
  const recorder = recordingEngine();
  configure({ engine: recorder.engine, owner: "spec://typescript/source-kind" });
  const spec = defineSpec("source-kind")
    .requirements(
      requirement("intake")
        .statement("The catalogue records the source type of every citation")
        .from(source("brief").name("Workflowd integration brief").kind("external_integration")),
    )
    .build();

  await apply(spec);

  // `kind` adds no optional URL or reference metadata.
  const request = recorder.requests().find(({ command }) => command === "apply");
  assert.deepEqual((request?.input as { sources?: unknown }).sources, [
    {
      key: "brief",
      name: "Workflowd integration brief",
      kind: "external_integration",
    },
  ]);
});

test("document stays the short form of the document kind", async () => {
  const recorder = recordingEngine();
  configure({ engine: recorder.engine, owner: "spec://typescript/source-kind-document" });
  const spec = defineSpec("source-kind-document")
    .requirements(
      requirement("intake")
        .statement("The catalogue records the source type of every citation")
        .from(source("brief").document("docs/brief.md")),
    )
    .build();

  await apply(spec);

  const request = recorder.requests().find(({ command }) => command === "apply");
  assert.deepEqual((request?.input as { sources?: unknown }).sources, [
    {
      key: "brief",
      name: "brief",
      kind: "document",
      reference: "docs/brief.md",
    },
  ]);
});

test("built fluent specs expose direct typed semantic handles", () => {
  const spec = defineSpec("direct-handles")
    .requirements(
      requirement("sharing")
        .statement("Shares are time bounded")
        .rules(rule("expiry").statement("Share links expire")),
    )
    .build();

  assert.equal(spec.requirements, spec.handles.requirements);
  assert.equal(
    spec.requirements.sharing.rules.expiry,
    spec.handles.requirements.sharing.rules.expiry,
  );
  assert.equal(Object.isFrozen(spec.requirements), true);
});

test("direct nested Rule handles apply and verify through the Rust engine", async () => {
  const repo = repository();
  configure({ engine, repository: repo, owner: "spec://typescript/direct-verification" });
  const spec = defineSpec("direct-verification")
    .requirements(
      requirement("sharing")
        .statement("Shares are time bounded")
        .rules(rule("expiry").statement("Share links expire")),
    )
    .build();

  await apply(spec);
  writeFileSync(join(repo, "share-links.test.ts"), "");
  let callbackRan = false;
  await spec.requirements.sharing.rules.expiry.verify(
    "share-links-expire",
    () => {
      callbackRan = true;
    },
    { file: join(repo, "share-links.test.ts") },
  );

  assert.equal(callbackRan, true);
  const runs = JSON.parse(
    execFileSync(
      engine,
      [
        "sdk",
        "verification-runs",
        "--repo",
        repo,
        "--scope",
        "default",
        "--format",
        "json",
      ],
      { encoding: "utf8" },
    ),
  ) as Array<{ status: string }>;
  assert.deepEqual(runs.map(({ status }) => status), ["passed"]);
});

test("a preferred fluent spec collects linked Sources and exposes its typed Rule", async () => {
  const repo = repository();
  configure({ engine, repository: repo, owner: "spec://typescript/preferred" });
  const shareLinks = defineSpec("share-links")
    .requirements(
      requirement("sharing")
        .statement("Users can securely share documentation")
        .description("Controls for links shared outside the organization")
        .from(
          source("sharing-policy")
            .name("Sharing policy")
            .document("docs/sharing-policy.md"),
        )
        .rules(
          rule("expiry").statement("Share links must expire within 30 days"),
        ),
    )
    .build();

  const result = await apply(shareLinks);
  writeFileSync(join(repo, "share-links.test.ts"), "");
  await shareLinks.requirements.sharing.rules.expiry.verify(
    "share-links-expire",
    () => undefined,
    { file: join(repo, "share-links.test.ts") },
  );

  assert.equal(result.resources.filter(({ kind }) => kind === "source").length, 1);
  assert.equal(result.resources.filter(({ kind }) => kind === "rule").length, 1);
  assert.equal(
    shareLinks.requirements.sharing.rules.expiry,
    shareLinks.handles.requirements.sharing.rules.expiry,
  );
});

test("one shared Rule materializes once and refines both Requirements", async () => {
  const repo = repository();
  configure({ engine, repository: repo, owner: "spec://typescript/shared" });
  const policy = source("access-policy").document("docs/access-policy.md");
  const expiry = rule("expiry").statement("Authenticated access expires");
  const sharing = requirement("sharing")
    .statement("Share links are time bounded")
    .from(policy)
    .rules(expiry);
  const sessions = requirement("sessions")
    .statement("Sessions are time bounded")
    .from(policy)
    .rules(expiry);
  const spec = defineSpec("lifecycles")
    .sources(policy)
    .requirements(sharing, sessions)
    .build();

  const result = await apply(spec);
  const rules = result.resources.filter(({ kind }) => kind === "rule");

  assert.equal(rules.length, 1);
  assert.equal(result.resources.filter(({ kind }) => kind === "source").length, 1);
  assert.deepEqual(rules[0]?.address, ["lifecycles", "rule", "expiry"]);
  assert.equal(
    spec.handles.requirements.sharing.rules.expiry,
    spec.handles.requirements.sessions.rules.expiry,
  );
  const written = readRecords(repo, "rules/rule.jsonl").find(({ id }) => id === rules[0]?.id);
  assert.equal(written?.requirement_ids?.length, 2);
});

test("distinct local Rules may reuse a key under unrelated Requirements", async () => {
  const repo = repository();
  configure({ engine, repository: repo, owner: "spec://typescript/local" });
  const shareExpiry = rule("expiry").statement("Share links expire within 30 days");
  const sessionExpiry = rule("expiry").statement("Sessions expire within 24 hours");
  const sharing = requirement("sharing").statement("Shares are time bounded").rules(shareExpiry);
  const sessions = requirement("sessions").statement("Sessions are time bounded").rules(sessionExpiry);
  const spec = defineSpec("lifecycles").requirements(sharing, sessions).build();

  const result = await apply(spec);
  const rules = result.resources.filter(({ kind }) => kind === "rule");

  assert.equal(rules.length, 2);
  assert.notEqual(rules[0]?.id, rules[1]?.id);
  assert.deepEqual(
    rules.map(({ address }) => address).sort(),
    [
      ["lifecycles", "requirement", "sessions", "rule", "expiry"],
      ["lifecycles", "requirement", "sharing", "rule", "expiry"],
    ],
  );
});

test("an explicit Rule id resolves an ambiguous local-to-shared merge", async () => {
  const repo = repository();
  configure({ engine, repository: repo, owner: "spec://typescript/merge" });
  const sharing = requirement("sharing")
    .statement("Shares are time bounded")
    .rules(rule("expiry").statement("Share links expire"));
  const sessions = requirement("sessions")
    .statement("Sessions are time bounded")
    .rules(rule("expiry").statement("Sessions expire"));
  const locals = await apply(defineSpec("lifecycles").requirements(sharing, sessions).build());
  const chosen = locals.resources.find(
    ({ kind, parent }) => kind === "rule" && parent === "sharing",
  )!;
  const sharedExpiry = rule("expiry")
    .id(chosen.id)
    .statement("Authenticated access expires");
  const shared = defineSpec("lifecycles")
    .requirements(
      requirement("sharing").statement("Shares are time bounded").rules(sharedExpiry),
      requirement("sessions").statement("Sessions are time bounded").rules(sharedExpiry),
    )
    .build();

  const result = await apply(shared);
  const materialized = result.resources.find(({ kind }) => kind === "rule")!;

  assert.equal(materialized.id, chosen.id);
  assert.deepEqual(materialized.address, ["lifecycles", "rule", "expiry"]);
});

test("distinct Rule declarations collide explicitly inside one Requirement", () => {
  const first = rule("expiry").statement("Share links expire within 30 days");
  const second = rule("expiry").statement("Share links expire within 14 days");
  const sharing = requirement("sharing")
    .statement("Share links are time bounded")
    .rules(first, second);

  assert.throws(
    () => defineSpec("share-links").requirements(sharing).build(),
    /distinct Rule declarations.*expiry.*sharing/i,
  );
});

test("the callback defineSpec form remains compatible", () => {
  const spec = defineSpec("legacy", ({ requirement }) => {
    const sharing = requirement("sharing", { statement: "Shares are time bounded" });
    return {
      expiry: sharing.rule("expiry", { statement: "Share links expire" }),
    };
  });

  assert.deepEqual(spec.handles.expiry.address, [
    "legacy",
    "requirement",
    "sharing",
    "rule",
    "expiry",
  ]);
});

test("relation fields on fluent declarations reach the written records", async () => {
  const repo = repository();
  configure({ engine, repository: repo, owner: "spec://typescript/fluent-relations" });
  seedResolution(repo);
  const older = source("policy-2024").document("docs/policy-2024.md");
  const policy = source("policy").document("docs/policy.md").supersedes(older);
  const parent = requirement("parent").statement("Access is governed").from(policy);
  const retired = requirement("retired").statement("Access was governed loosely").from(older);
  const decision = rule("decision")
    .statement("Access follows the seed decision")
    .resolutions("res_seed");
  const refined = requirement("child")
    .statement("Share links are governed")
    .from(policy)
    .refines(parent)
    .dependsOn(parent)
    .supersedes(retired)
    .spawnedBy("res_seed")
    .rules(decision);
  const spec = defineSpec("relations")
    .sources(older, policy)
    .requirements(parent, retired, refined)
    .build();

  const result = await apply(spec);

  const idOf = (kind: string, key: string): string =>
    result.resources.find((resource) => resource.kind === kind && resource.key === key)!.id;
  const child = readRecords(repo, "requirements/req.jsonl").find(
    ({ id }) => id === idOf("requirement", "child"),
  );
  assert.equal(child?.refines, idOf("requirement", "parent"));
  assert.deepEqual(child?.depends_on, [idOf("requirement", "parent")]);
  assert.deepEqual(child?.supersedes, [idOf("requirement", "retired")]);
  assert.equal(child?.spawned_by, "res_seed");
  const writtenPolicy = readRecords(repo, "sources/source.jsonl").find(
    ({ id }) => id === idOf("source", "policy"),
  );
  assert.deepEqual(writtenPolicy?.supersedes, [idOf("source", "policy-2024")]);
  const writtenDecision = readRecords(repo, "rules/rule.jsonl").find(
    ({ id }) => id === idOf("rule", "decision"),
  );
  assert.deepEqual(writtenDecision?.resolution_ids, ["res_seed"]);
});

test("a fluent relation to a requirement outside the build is refused", () => {
  const parent = requirement("parent").statement("Access is governed");
  const child = requirement("child").statement("Shares are governed").refines(parent);
  assert.throws(
    () => defineSpec("relations").requirements(child).build(),
    /Requirement `child` names Requirement `parent` not included in the spec/,
  );
});
