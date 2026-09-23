import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { startFixtureHost } from "../scripts/fixture-host.js";
import { OperationError } from "./client.js";

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
      statement: "Share links expire in 30 days",
    });
    await apply();

    const fetched = await get({ node_type: "rule", id: expiry.id });
    assert.equal(fetched.data.id, expiry.id);
    assert.ok(fetched.meta.stamp);
    assert.equal(fetched.meta.freshness_error, null);

    const matched = await search({ collection: "requirements", text: "time bounded" });
    assert.equal(matched.meta.has_more, false);
    assert.deepEqual(matched.data.items.map((node) => node.id), [sharing.id]);

    const around = await neighbors({ node_type: "rule", id: expiry.id });
    assert.deepEqual(
      around.data.neighbors.map((neighbor) => neighbor.node.id),
      [sharing.id],
    );

    // The source is named by the requirement's citation and the requirement
    // by the rule's list, so the walk to the rule reads `in` at every hop.
    const walked = await trace({ node_type: "source", id: retention.id, direction: "in" });
    assert.ok(
      walked.data.nodes.some((reached) => reached.node.id === expiry.id && reached.depth === 2),
    );

    const reached = await impact({ node_type: "requirement", id: sharing.id });
    assert.deepEqual(reached.data.affected_rules.map((rule) => rule.id), [expiry.id]);

    const behind = await evidence({ rule: expiry.id });
    assert.equal(behind.data.rule_id, expiry.id);
    assert.equal(behind.data.review_required, false);
    assert.equal(behind.data.stale, null);

    const resolved = await resolveSymbol({ file: "share-links.ts" });
    assert.deepEqual(resolved.data.items, []);
  } finally {
    await fixture.close();
  }
}

test(
  "structured queries return the engine's bounded answers unchanged",
  readsTheEnginesBoundedAnswers,
);

test("a missing member is a typed resource failure", async () => {
  const fixture = await repository();
  try {
    configure({ ...fixture.settings, owner: "spec://freshness" });
    const request = { node_type: "requirement" as const, id: "req_missing" };
    await assert.rejects(get(request), error =>
      error instanceof OperationError && error.status === 404);
  } finally {
    await fixture.close();
  }
});
