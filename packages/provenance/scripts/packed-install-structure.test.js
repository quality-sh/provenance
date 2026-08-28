import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

test("the packed install scenario stays below the repository file limit", () => {
  const source = readFileSync(new URL("./test-packed-install.js", import.meta.url), "utf8");
  const lines = source.endsWith("\n")
    ? source.slice(0, -1).split("\n").length
    : source.split("\n").length;

  assert.ok(lines < 500, `test-packed-install.js has ${lines} lines`);
});
