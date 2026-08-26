import {
  rule as authoringRule,
  type FluentRule,
} from "@quality-sh/provenance";
import {
  rule as bindRule,
  verifies,
  type VerificationMethod,
} from "@quality-sh/provenance/rules";

const implementation = (hours: number): boolean => hours > 38;
const bound: (hours: number) => boolean = bindRule(
  "rule_overtime",
  implementation,
);
const authored: FluentRule<"overtime"> = authoringRule("overtime");
const method: VerificationMethod = "examples";

void bound(39);
void authored;
verifies("rule_overtime", method);

// @ts-expect-error Rule bindings preserve the implementation parameter types.
bound("39");
// @ts-expect-error Verification methods use the scanner's fixed vocabulary.
verifies("rule_overtime", "sample");
