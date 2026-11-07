# ADR 0002: Rules are independent of implementation

## Status

Accepted.

## Decision

A Rule is an identified atomic behavioural obligation that refines a Requirement and
may also be produced by a Resolution. It can exist before implementation or
verification. `#[rule]` and equivalent language helpers bind its primary production
implementation, limited to one primary implementation per Rule for now; `#[verifies]`
binds evidence.

The former model made the deciding function or construction define the Rule. That gave
retrofit scanning a strong code anchor, but it made specification-first Rules
impossible and conflated what should be true with where production code realizes it.
Separating the obligation from both implementation and evidence lets typed declarations
and scanner-discovered bindings target one canonical Rule.

## Consequences

Rule records, source-location fields, attributes, decorators, and
scanner syntax remain compatible. Existing source locations remain citations; only
scanner-recognized attributes, helpers, decorators, and comments bind implementation.
Validation and reports distinguish Unimplemented from Unverified; both remain derived
absence rather than stored Rule fields. This decision does not introduce a durable
test-to-Rule record or change transient verification runs.
