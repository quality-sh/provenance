import type { VerificationMethod } from '../../../dist/rules.js';
import type { VerificationTarget } from '../../../dist/verification.js';

type ExpectedMethod = 'exhaustion' | 'property' | 'examples' | 'conformance' | 'construction' | 'proof';
type ExpectedTarget = { rule: string } | { declaration: { declared_by: string; address: readonly string[] } };
type Assert<Check extends true> = Check;
type Extends<Left, Right> = [Left] extends [Right] ? true : false;
type MethodForward = Assert<Extends<VerificationMethod, ExpectedMethod>>;
type MethodBackward = Assert<Extends<ExpectedMethod, VerificationMethod>>;
type TargetForward = Assert<Extends<VerificationTarget, ExpectedTarget>>;
type TargetBackward = Assert<Extends<ExpectedTarget, VerificationTarget>>;
