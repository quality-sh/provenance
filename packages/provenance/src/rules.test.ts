import assert from "node:assert/strict";
import test from "node:test";

import { rule, verifies } from "@quality-sh/provenance/rules";

test("rule returns the exact implementation with its callable type", () => {
  const implementation = (hours: number): boolean => hours > 38;
  const bound = rule("rule_overtime", implementation);
  const typed: (hours: number) => boolean = bound;

  assert.equal(bound, implementation);
  assert.equal(typed(39), true);
});

test("verifies accepts every scanner verification method and returns nothing", () => {
  for (const method of [
    "exhaustion",
    "property",
    "examples",
    "conformance",
    "construction",
    "proof",
  ] as const) {
    assert.equal(verifies("rule_overtime", method), undefined);
  }
});
