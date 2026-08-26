import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const packageRoot = new URL("..", import.meta.url);
const manifest = JSON.parse(
  readFileSync(new URL("package.json", packageRoot), "utf8"),
);

test("the rules subpath has runtime and TypeScript exports", () => {
  assert.deepEqual(manifest.exports["./rules"], {
    types: "./dist/rules.d.ts",
    import: "./dist/rules.js",
    default: "./dist/rules.js",
  });
});

test("the implementation binder does not replace the authoring rule builder", async () => {
  const authoring = await import("@quality-sh/provenance");
  const bindings = await import("@quality-sh/provenance/rules");

  assert.equal(authoring.rule.length, 1);
  assert.equal(bindings.rule.length, 2);
  assert.notEqual(authoring.rule, bindings.rule);
});
