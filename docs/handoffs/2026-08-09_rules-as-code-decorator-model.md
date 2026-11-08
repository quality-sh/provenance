---
date: 2026-08-09
git_commit: c9165d319edbbff580b1ab4b6e1f35c4eab8b634
branch: worktree-rules-runtime-prototype
topic: "Session Handoff: Rules as code — fiat demoted, decorator model adopted"
tags: [handoff, rules-as-code, prototype]
status: complete
---

# Handoff: rules-as-code prototype session

> **Superseded. Kept as a record of the session, not as guidance.** What shipped is not
> the design below. A rule is a function, or a type whose construction is the proof,
> bound by `#[rule("rule_id")]`; tests verify it with `#[verifies("rule_id", method)]`
> naming one of `exhaustion`, `property`, `examples`, `conformance`, or `construction`.
> There is no binding ladder, no `#[enforces]` attribute, and no rule code: ids only.
> Read `README.md`, `docs/shaping.md`, and `docs/cli.md` for the current model. The body
> is unchanged below.

This session continued `docs/research/2026-08-09-rules-as-code-fiat-handoff.md` and
**substantially revised its central decision**. Read that doc for background, then this
doc as the correction. Where they disagree, this doc wins.

## Where the design landed (ratified by Ben in conversation)

1. **The docs' definition of rule stands.** README line 5: source → requirement → rule →
   "the code and tests that enforce them." A resolution is the decision; a rule is the
   decision restated as an enforceable statement; **code AND tests enforce rules**. Fiat
   ("the function IS the rule") is demoted from the model to the strongest rung of a
   binding ladder.
2. **The binding ladder.** A rule is enforced by, in ascending strength: (a) a test —
   the default and the on-ramp, non-invasive, most rules live here; (b) a production
   function on the live decision path; (c) by-construction (types, DB constraints).
3. **Decorator, not primitive.** The API is one marker — `#[enforces("RULE-CODE")]`
   (Rust attribute; equivalent per language) — on tests and enforcing functions. It is a
   compiled symbol, not a comment: moves with refactors, dies with deleted code, unknown
   rule codes fail the scan. NO `define_rule` primitive, NO registry table, NO runtime —
   a rule primitive earns existence only when someone wants decision reports (runtime
   participation / typed declines), and not before.
4. **Value seam vs status quo**: meaning-fidelity of a binding stays human (reviewed
   once); everything downstream — existence, location, liveness, last-run verdict — is
   deterministic. Enforcement bindings get a heartbeat via CI (tests emit per-rule
   verdicts), and agents get machine-readable "you are editing an enforcer of X" context.
5. **Where fiat's payoff actually lives**: not in restructuring clean code. The unit of
   value is the decision's *footprint* — single-homing drifted periphery onto a canonical
   predicate, conformance tests holding replicas to it, funneling bypasses. "Register
   what's clean, refactor what drifts."

## The open question this session ended on (UNANSWERED — start here)

Ben asked, then cut for handoff: **"What if we need to use this rule in creation of
other rules?"** — i.e. composition under the decorator model. Fiat handled composition
by rules-calling-rules as functions. With enforcement-by-decorator, what does it mean
for rule B's enforcement to depend on rule A (e.g. PROV-EDGE-ENDPOINT-TABLE composes the
per-arm rules; an app's `canCheckout` uses `customer.active`)? Candidate directions
discussed earlier in session: rule→rule edges in the graph (currently forbidden — the
planned repeal of PROV-EDGE-RULE-LEAF), shared enforcer symbols (many rules → one
function), or the rule primitive returning for composed cases. Not resolved. Do not
assume an answer; work it with Ben.

## Graph state — SPLIT ACROSS TWO STORES, both uncommitted

- **Main checkout** (`/home/ben/Documents/repos/provenance/.provenance/`, uncommitted):
  morning session landed `req_rules_executable_definitions` (anchor),
  `res_rules_executable_by_fiat`, `res_facts_are_composed_rules`,
  `res_cross_language_rules_are_replicas`, `res_rule_scope_is_users_call`,
  `topic_rules_as_code_model` + 8 questions,
  `source_rules_as_code_fiat_handoff`. **Several of these are now stale**:
  `res_rules_executable_by_fiat` needs revision (fiat → strongest rung, not the model);
  `question_rule_decision_reports` is answered in spirit (boundary emission; primitive
  deferred); the facts/composition resolutions survive but through the test lens.
  Revising these was NOT done. Do not silently rewrite ratified resolutions — propose
  supersessions to Ben.
- **This worktree's store** (uncommitted on this branch): 4 requirements
  (`req_sources_ground_requirements`, `req_shaping_loop_graph_wiring`,
  `req_rules_are_produced_leaves`, `req_edge_writes_validated`), each source-pinned to
  `source_codebase_provenance_c9165d3` with `refines_into` from
  `req_implement_a_normalized_knowledge_graph_d`; 6 rule records
  (`rule_prov_edge_references`, `_req_structure`, `_shaping`, `_produces`, `_rule_leaf`,
  `_endpoint_table`) with `produces` edges; `source_document`/`source_section` point at
  symbols in `edge_validation.rs`. These remain valid under the decorator model.

## Code state in this worktree

`crates/provenance-core/src/edge_validation.rs` is currently the WRONG shape per the
final design: I split the nine-arm match into five arm predicates + dispatcher + a
`RULE_REGISTRATIONS` const table. Workspace tests green, clippy/fmt clean — but the
ratified direction is:

1. **Restore the original single-match `validate_edge_endpoint`** (git history has it).
2. **Keep the two new tests** (`rejects_every_edge_leaving_a_rule`,
   `endpoint_table_conforms_to_rule_leaf` — 324-triple conformance; also keep
   `edge_may_touch_rule` which the conformance test needs as its oracle).
3. **Delete `RULE_REGISTRATIONS`** and the arm extractions.
4. Bindings become `#[enforces]` attributes once that attribute exists.

## The audit findings (real bugs, fix as plain work — no new machinery needed)

Four opus agents audited one requirement each; key confirmed defects:

- `merge-jsonl` (`crates/provenance-cli/src/handlers/merge_jsonl.rs:12-23`): merges
  untyped JSON, writes edge shards with zero validation; wired via `.gitattributes` as
  the merge driver — can store invalid edges. The write-gate requirement is false today.
- `write_source_reference` (`provenance-store/src/state_store/writers.rs:135`): calls
  the validator with compile-time constants (can never fail) and duplicates the append
  block instead of using `add_edge`.
- `health.rs:257` `has_source`: satisfied by ANY sourced requirement in scope, not the
  rule's producer — vacuously true. `health.rs:246-256` requires BOTH producers while
  `gaps` accepts either — the two surfaces disagree on "orphan".
- `health.rs:179` `source_linked_requirements` counts edges only;
  `graph_query.rs:177 requirement_has_valid_source` accepts edges OR inline refs.
- Rules with neither `requirement_id` nor `resolution_id` are writable
  (`rule_writers.rs:97-187`) — no write-time producer enforcement.
- No writer creates `spawns` edges (convention only); resolutions have no persisted
  `requirement_id` — edges are the only wiring and `edges delete` breaks pairs silently.
- `questions answer --resolution-id` accepts a resolution of an unrelated requirement.
- `check` validates edge endpoints against a GLOBAL node index while writers use
  per-scope — cross-scope edges pass check but would be rejected at write.

## Prioritized next steps

1. Work the composition question with Ben (see above) — it was cut mid-flight.
2. Un-refactor `edge_validation.rs` per "Code state" above.
3. Build the decorator path: `#[enforces]` attribute crate; upgrade
   `provenance-scanner` (comment annotations → attribute parsing; `@provenance` comments
   are the legacy tier); two new `check` findings (active rule above severity threshold
   with no live enforcer; enforcer citing unknown rule code).
4. Fix the audit bugs as ordinary PRs.
5. Propose supersessions for the stale morning resolutions.
6. The venture-validation experiment Ben accepted: execute the rule→rule repeal of
   PROV-EDGE-RULE-LEAF *through* the graph when composition lands, and judge whether
   traceability made a real change safer. This decides how much more machinery gets
   built.

## How to work with Ben (hard-won this session)

- Plain language. No coined shorthand, no "the leaves agent" style nicknames, no
  arrow-chain compression. He will interrupt the moment you ramble.
- Do not be literal: rule/requirement granularity follows decisions made by humans, not
  code structure or counts.
- He steers closely: surface design forks BEFORE building (the TS-in-Rust-repo and
  check-reconciler detours were both rejected as unasked scope). But don't use
  AskUserQuestion — he wants free-form back-and-forth ("not on rails").
- Agent fan-out: one unit of work per agent, NO sibling context (he killed primed
  agents; convergence must be independent). Opus for these was accepted.
- Requirements before rules; the graph dictates structure, not vice versa.
- `provenance` CLI: use it from the worktree for this branch's store; cross-checkout
  writes are blocked by the worktree guard — run single plain commands (compound
  commands with loops/redirects get refused).

## Git state

Uncommitted (this worktree): `.provenance` state (4 files) + `edge_validation.rs`.
Branch `worktree-rules-runtime-prototype` at `c9165d3` (== origin/main).
Main checkout separately holds its own uncommitted `.provenance` changes — do not
discard either side; JSONL merges are append-friendly.
