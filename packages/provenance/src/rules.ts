import type { components } from "./generated/schema.js";

export type VerificationMethod = components["schemas"]["GetRuleEvidenceSuccessVerificationMethod"];

/** Mark the containing test with its verification method. */
// @provenance rule: rule_typescript_verifies_helper_marker
export function verifies(
  _ruleId: string,
  _method: VerificationMethod,
): void {}
