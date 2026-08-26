export type VerificationMethod =
  | "exhaustion"
  | "property"
  | "examples"
  | "conformance"
  | "construction"
  | "proof";

type AnyFunction = (...args: never[]) => unknown;

/** Bind a Rule's primary implementation without changing the function. */
// @provenance rule: rule_typescript_rule_helper_identity
export function rule<FunctionType extends AnyFunction>(
  _ruleId: string,
  implementation: FunctionType,
): FunctionType {
  return implementation;
}

/** Mark the containing test with its verification method. */
// @provenance rule: rule_typescript_verifies_helper_marker
export function verifies(
  _ruleId: string,
  _method: VerificationMethod,
): void {}
