import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { startFixtureHost } from "../scripts/fixture-host.js";

import {
  apply,
  configure,
  evidence,
  get,
  impact,
  neighbors,
  requirement,
  resolveSymbol,
  search,
  source,
  stale,
  trace,
} from "./index.js";

const engine = fileURLToPath(
  new URL(`../../../target/debug/provenance${process.platform === "win32" ? ".exe" : ""}`, import.meta.url),
);

async function repository() {
  const repo = mkdtempSync(join(tmpdir(), "provenance-ts-query-"));
  try {
    execFileSync(engine, [
      "--quiet", "init", "--path", repo, "--scope", "default", "--path-prefix", ".",
    ], { stdio: "pipe" });
    const host = await startFixtureHost({ root: repo, repositoryId: "query-fixture" });
    return {
      repo,
      settings: {
        endpoint: host.environment.PROVENANCE_ENDPOINT,
        bearer: host.environment.PROVENANCE_TOKEN,
        repositoryId: host.environment.PROVENANCE_REPOSITORY_ID,
        localRoot: repo,
        scope: "default",
      },
      async close() {
        try {
          await host.close();
        } finally {
          rmSync(repo, { recursive: true, force: true });
        }
      },
    };
  } catch (error) {
    rmSync(repo, { recursive: true, force: true });
    throw error;
  }
}

async function readsTheEnginesBoundedAnswers(): Promise<void> {
  const fixture = await repository();
  try {
    configure({
      ...fixture.settings,
      owner: "spec://typescript/share-links",
      verificationOwner: "ci://node-test",
    });
    const retention = source("retention", {
      kind: "policy",
      name: "Retention policy",
    });
    const sharing = requirement("sharing", {
      statement: "Shares are time bounded",
      sources: [retention],
    });
    const expiry = sharing.rule("expiry", {
      statement: "Share links expire within 30 days",
    });
    await apply();

    const fetched = await get({ node_type: "rule", id: expiry.id });
    assert.equal(fetched.protocol_version, 7);
    assert.equal(fetched.operation, "get");
    assert.equal(fetched.found, true);
    assert.equal(fetched.node?.id, expiry.id);

    const matched = await search({ text: "time bounded" });
    assert.equal(matched.has_more, false);
    assert.deepEqual(matched.nodes.map((node) => node.id), [sharing.id]);

    const around = await neighbors({ id: expiry.id });
    assert.deepEqual(
      around.neighbors.map((neighbor) => neighbor.node.id),
      [sharing.id],
    );

    // The source is named by the requirement's citation and the requirement
    // by the rule's list, so the walk to the rule reads `in` at every hop.
    const walked = await trace({ id: retention.id, direction: "in" });
    assert.ok(
      walked.nodes.some((reached) => reached.node.id === expiry.id && reached.depth === 2),
    );

    const reached = await impact({ id: sharing.id });
    assert.deepEqual(reached.affected_rules.map((rule) => rule.id), [expiry.id]);

    const behind = await evidence({ rule: expiry.id });
    assert.equal(behind.rule_id, expiry.id);
    assert.equal(behind.review_required, false);
    assert.equal(behind.stale, null);

    const resolved = await resolveSymbol({ file: "share-links.ts" });
    assert.deepEqual(resolved.rules, []);
  } finally {
    await fixture.close();
  }
}

test(
  "structured queries return the engine's bounded answers unchanged",
  readsTheEnginesBoundedAnswers,
);

test("a query freshness option overrides the repository setting", async () => {
  const fixture = await repository();
  const repo = fixture.repo;
  try {
    configure({ ...fixture.settings, owner: "spec://freshness" });
    const request = { node_type: "requirement" as const, id: "req_missing" };
    await get(request);
    writeFileSync(join(repo, ".provenance/settings.json"), JSON.stringify({ read: { freshness_policy: "catch_up" } }));
    execFileSync("git", ["init", "--quiet", repo]);
    execFileSync("git", ["-c", "user.name=Test", "-c", "user.email=test@example.com",
      "-c", "core.hooksPath=/dev/null", "commit", "--allow-empty", "--quiet", "-m", "Initialize test repository"], { cwd: repo });
    const options = { freshness: "annotate_only" as const };
    const queries = [
      () => get(request, options),
      () => search({ text: "missing" }, options),
      () => neighbors({ id: "req_missing" }, options),
      () => trace({ id: "req_missing" }, options),
      () => impact({ id: "req_missing" }, options),
      () => evidence({ rule: "rule_missing" }, options),
      () => stale({ base: "HEAD", head: "HEAD" }, options),
      () => resolveSymbol({ file: "missing.rs" }, options),
    ];
    for (const query of queries) {
      const answer = await query();
      assert.equal(answer.stamp.policy, "annotate_only");
    }
  } finally {
    await fixture.close();
  }
});
