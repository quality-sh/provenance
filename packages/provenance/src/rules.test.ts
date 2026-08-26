import assert from "node:assert/strict";
import test from "node:test";

import {
  rule as bindRule,
  verifies,
} from "@quality-sh/provenance/rules";

test("rule returns the exact implementation with its callable type", function ruleHelperIdentityExamples() {
  verifies("rule_typescript_rule_helper_identity", "examples");
  const implementation = (hours: number): boolean => hours > 38;
  const bound = bindRule("rule_typescript_rule_helper_identity", implementation);
  const typed: (hours: number) => boolean = bound;

  assert.equal(bound, implementation);
  assert.equal(typed(39), true);
});

test("verifies marks a test without changing execution", function verifiesHelperExamples() {
  assert.equal(
    verifies("rule_typescript_verifies_helper_marker", "examples"),
    undefined,
  );
});
