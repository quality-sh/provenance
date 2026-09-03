import assert from "node:assert/strict";
import { execFileSync, } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

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
  trace,
} from "./index.js";

const engine = fileURLToPath(
  new URL("../../../target/debug/provenance", import.meta.url),
);

function repository(): string {
  const repo = mkdtempSync(join(tmpdir(), "provenance-ts-query-"));
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

async function readsTheEnginesBoundedAnswers(): Promise<void> {
  const repo = repository();
  configure({
    engine,
    repository: repo,
    scope: "default",
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
  assert.equal(fetched.protocol_version, 6);
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
}

test(
  "structured queries return the engine's bounded answers unchanged",
  readsTheEnginesBoundedAnswers,
);
