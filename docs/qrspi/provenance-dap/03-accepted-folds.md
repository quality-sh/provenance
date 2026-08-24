# Accepted Design revision 7 fold delta

This delta records the seven optional controls that Ben accepted with the
fold-and-accept response on 2026-08-24. The accepted Design is the immutable reviewed
Design revision 7 plus this exact delta. The delta does not replace or edit the reviewed
bytes.

1. **CO-R7-1 — complete the two citations.** In DR1 point 2(i), the statement-check
   call sites also include `bound-declarations.ts:163` and `:215`. In DR2, the pinned
   wire call sites also include `rule_declaration_ids` at `typed_specs.rs:77` and
   `normalize_rule_relationships` through `prepare_typed_spec` at `typed_specs.rs:310`.
2. **CO-R7-2 — complete the revision disclosure.** The semantics-neutral revision-7
   residue also includes these changes: removal of the duplicate DR3 ordering-delta
   sentence because DR11(c) keeps it; removal of the old reviewer-convergence note;
   removal of the duplicate V4 macro-home and V10 scope notes; removal of stale fold
   tags and revision-5 wording; the DR4 heading was shortened; and DR2 now states that
   the retained envelope check includes its repository-state-dependent scope check.
3. **CO-R7-3 — pin intra-list error order.** V2 includes a multi-defect document with
   one structural defect and one resolution-dependent defect. It also includes a list
   with two structural defects. Each case asserts the first reported error and pins the
   current per-item interleaving.
4. **CO-R7-4 — capture wording before relocation.** Before any DR4(b) relocation, V2
   captures the current engine wording for every newly pinned rejection class. These
   wording pins land before the kernel takes ownership of those checks.
5. **CO-R7-5 — separate the two wording regimes.** Wire-facing rejection text stays
   identical to the engine text. TypeScript-parity wording applies only to checks that
   run at authoring `build()`. If a shared kernel predicate runs during wire ingestion,
   it emits the engine wording.
6. **CO-R7-6 — pin trim behavior.** The parity ledger records that kernel content
   predicates must match `requireText` trim behavior. Plan and Implementation must
   verify this behavior.
7. **CO-R7-7 — cover discovery fallback.** V2 includes a start directory that has no
   repository. It verifies the canonicalized-start-directory fallback of
   `resolve_repository` at `handlers/sdk.rs:170`.
