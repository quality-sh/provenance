export type VerificationMethod =
  | "exhaustion"
  | "property"
  | "examples"
  | "conformance"
  | "construction"
  | "proof";

type AnyFunction = (...args: never[]) => unknown;

/** Bind a Rule's primary implementation without changing the function. */
export function rule<FunctionType extends AnyFunction>(
  _ruleId: string,
  implementation: FunctionType,
): FunctionType {
  return implementation;
}

/** Mark the containing test with its verification method. */
export function verifies(
  _ruleId: string,
  _method: VerificationMethod,
): void {}
