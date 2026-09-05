import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const packageRoot = fileURLToPath(new URL("..", import.meta.url));
const tsc = fileURLToPath(new URL("../node_modules/typescript/bin/tsc", import.meta.url));

function typecheck(fixture) {
  return spawnSync(process.execPath, [tsc, "-p", `test/fixtures/${fixture}/tsconfig.json`], {
    cwd: packageRoot,
    encoding: "utf8",
  });
}

const built = spawnSync(process.execPath, [tsc, "-p", "tsconfig.json"], {
  cwd: packageRoot,
  encoding: "utf8",
});
assert.equal(built.status, 0, built.stdout + built.stderr);

const valid = typecheck("valid");
assert.equal(valid.status, 0, valid.stdout + valid.stderr);

const queryEnvelope = typecheck("query-envelope");
assert.equal(queryEnvelope.status, 0, queryEnvelope.stdout + queryEnvelope.stderr);

const contextValid = typecheck("context-valid");
assert.equal(contextValid.status, 0, contextValid.stdout + contextValid.stderr);

const compositionValid = typecheck("composition-valid");
assert.equal(compositionValid.status, 0, compositionValid.stdout + compositionValid.stderr);

const compositionCrossContext = typecheck("composition-cross-context");
assert.notEqual(
  compositionCrossContext.status,
  0,
  "a public helper unexpectedly accepted declarations from different specs",
);
assert.match(compositionCrossContext.stdout + compositionCrossContext.stderr, /TS2345/);

const declarationImmutability = typecheck("declaration-immutability");
assert.notEqual(
  declarationImmutability.status,
  0,
  "a public construction declaration unexpectedly allowed mutation",
);
assert.match(declarationImmutability.stdout + declarationImmutability.stderr, /TS2540/);

const contextLocalRuleMismatch = typecheck("context-local-rule-mismatch");
assert.notEqual(
  contextLocalRuleMismatch.status,
  0,
  "a requirement-local Rule unexpectedly attached to another Requirement",
);
assert.match(contextLocalRuleMismatch.stdout + contextLocalRuleMismatch.stderr, /TS2345/);

const sourceKindClosed = typecheck("source-kind-closed");
const sourceKindOutput = sourceKindClosed.stdout + sourceKindClosed.stderr;
assert.notEqual(
  sourceKindClosed.status,
  0,
  "an unsupported source kind unexpectedly typechecked",
);
assert.match(sourceKindOutput, /TS2345/);
// Both fluent builder surfaces close the kind: the top-level
// `source(key).kind` and the spec-scoped `defineSpec(key).source(key).kind`.
const closedKindErrors = sourceKindOutput
  .split("\n")
  .filter((line) => line.includes("TS2345") && line.includes("SourceKind"));
assert.equal(
  closedKindErrors.length,
  2,
  "both fluent builder surfaces must reject an unsupported kind",
);

const contextCrossSpec = typecheck("context-cross-spec");
assert.notEqual(contextCrossSpec.status, 0, "a Source unexpectedly crossed spec contexts");
assert.match(contextCrossSpec.stdout + contextCrossSpec.stderr, /TS2345/);

const implementedByValid = typecheck("implemented-by-valid");
assert.equal(implementedByValid.status, 0, implementedByValid.stdout + implementedByValid.stderr);

const implementedByClassValid = typecheck("implemented-by-class-valid");
assert.equal(
  implementedByClassValid.status,
  0,
  implementedByClassValid.stdout + implementedByClassValid.stderr,
);

const implementedByRemoved = typecheck("implemented-by-removed");
assert.notEqual(
  implementedByRemoved.status,
  0,
  "a removed implementation export unexpectedly typechecked",
);
assert.match(implementedByRemoved.stdout + implementedByRemoved.stderr, /TS2305/);
assert.match(implementedByRemoved.stdout + implementedByRemoved.stderr, /startWorkflow/);

const implementedByClassRemoved = typecheck("implemented-by-class-removed");
assert.notEqual(
  implementedByClassRemoved.status,
  0,
  "a removed class implementation export unexpectedly typechecked",
);
assert.match(implementedByClassRemoved.stdout + implementedByClassRemoved.stderr, /TS2305/);
assert.match(implementedByClassRemoved.stdout + implementedByClassRemoved.stderr, /WorkflowRunner/);

const missingKey = typecheck("missing-key");
assert.notEqual(missingKey.status, 0, "verification without a key unexpectedly typechecked");
assert.match(missingKey.stdout + missingKey.stderr, /TS2554/);

const renamed = typecheck("renamed");
assert.notEqual(renamed.status, 0, "renamed export unexpectedly typechecked");
assert.match(renamed.stdout + renamed.stderr, /TS2339/);
assert.match(renamed.stdout + renamed.stderr, /expiry/);
