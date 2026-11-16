import assert from "node:assert/strict";
import test from "node:test";

import { verifies } from "@quality-sh/provenance/rules";

test("verifies marks a test without changing execution", function verifiesHelperExamples() {
  assert.equal(
    verifies("rule_typescript_verifies_helper_marker", "examples"),
    undefined,
  );
});
