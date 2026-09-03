# Review: PR 180, relation shapes cut (provenance-1wh.1)

Reviewed head `9302f1b` (branch `1wh-relation-shapes-cut` as pushed to the fork origin), nine commits on main
`be9b52c`. Governing texts: plan rev 5.1 (gist file 01), reviews 02-04, converter file 05, binding resolution
`res_relations_are_fields_or_action_records`, PR body with its thirteen declared deviations. Every claim below was
re-verified on this checkout: builds, the full workspace suite, the TS suite, the in-tree parity harness, six
mutation replays, the converter (file 05) run against a copy of main's state, and CLI probes on throwaway
repositories. No production code was changed.

## Verdict: NOT READY — two blocking items, both small, both the same principle

The conversion, the parity evidence, and the mechanical cut are the strongest parts of this PR and they check out
end to end. What does not check out is the binding resolution's core promise — "the declaration drives every
derivation so no list is maintained by hand" — in two enforcement paths that still hand-roll what the declaration
table already knows. Both fixes are small; neither touches the converted state.

1. **BLOCKING — the create writers' requiredness is hand-coded, not declaration-driven.**
   `crates/provenance-store/src/state_store/rule_writers.rs:33` (`"a resolution needs one requirement"`) and
   `rule_writers.rs:98` (`anyhow::ensure!(!requirement_ids.is_empty(), "a rule needs one requirement")`) duplicate
   what `missing_required`/`required_refusal` (`crates/provenance-core/src/model/relations/integrity.rs:24-38`)
   already derive from the table. Plan B says requiredness "is enforced from the table by the writer, the graph
   validator (section E), `check`, and the merge gate"; the writer arm is not from the table. The PR's own mutation
   table admits it: mutation 3 (make `Rule.requirement_ids` optional) turns the writer's checks green — I replayed
   the mutation and confirmed exactly that, while the three named tests (references, graph_validation,
   merge::validation) went red. This is nowhere in the thirteen declared deviations; it hides in a parenthetical.
   Concrete failure: someone relaxes or retargets a required list in one place (the attribute) and the writer's
   message and refusal behavior silently diverge from the table every other consumer reads. Fix: run
   `missing_required` over the constructed record (or check `decl.required` before writing) and delete the two
   literals.

2. **BLOCKING — `validate_graph_scope` refuses cycles on requirement chains only; a `supersedes` cycle between two
   resolutions or two sources in state is refused by nothing.**
   `crates/provenance-store/src/state_store/graph_validation.rs:17` (`REQUIREMENT_CHAINS`) and `:31`
   (`ensure_acyclic(&requirements)`) walk requirement records only, although `supersedes` is declared on resolution
   and source too (plan C; `crates/provenance-core/src/model/artifacts.rs:53,201`). Plan E promises the validator
   refuses "a `refines`/`depends_on`/`supersedes` cycle in state" and L13 repeats it; neither restricts to
   requirements. Verified empirically: I planted a mutual `supersedes` pair on two resolutions in a copy of the
   converted state — `provenance check` returned `status: ok`, `export` succeeded, and nothing else refuses it
   (`check`'s dangling pass does not walk cycles, the merge gate's doc delegates cross-record checks to `check`,
   import goes through `check`, catch-up goes through `validate_graph_scope`). The writers do refuse
   (`add_to_list`, `reference_writers.rs:127-134`), so the hole needs a hand edit or a merge to reach, but that is
   exactly the state the graph validator exists for. Fix: run `cycle_in` over resolutions and sources for
   `supersedes` beside the requirement pass.

Everything else below is non-blocking.

## Findings (most severe first)

1. *(blocking — see verdict)* Writer requiredness hand-coded: `rule_writers.rs:33,98`.
2. *(blocking — see verdict)* Validator cycle refusal scoped to requirements only:
   `graph_validation.rs:17,31,49-56`; empirical refusal escape via planted resolution cycle.
3. **The derive's compile-time guard does not cover via-struct, alias, or wrapper shapes.**
   `crates/provenance-macros/src/relations.rs:70` fires the error only for `Shape::Single | OptionalSingle | List`.
   Probe results (scratch crate over the real macro): `Vec<NestedStruct>` with no `#[relation]` compiles with an
   **empty** table; `type ReqAlias = StableId` with no attribute compiles silently (`is_stable_id`,
   `relations.rs:221-225`, matches only the literal last segment); `Box<StableId>` with no attribute compiles
   silently (`wrapped`, `relations.rs:227-242`, accepts only `Option`/`Vec`). The plan only promised the guard for
   StableId-shaped fields, so this is plan-conformant — but the module doc
   (`crates/provenance-core/src/model/relations.rs:6-8`) overclaims ("a reference field without a declaration
   cannot exist: the derive refuses it at compile time"), and plan B's visibility rule ("a field that points
   outside the table carries `#[relation(none)]` so the exemption is visible at the field") is broken in the live
   tree: `links` on Topic and Question (`shaping.rs:142,168`) carries **no** attribute and is exempt only because
   its shape is silently skipped. Today the serde-walking test (`model/tests/relation_walk.rs`) plus the hand
   `links` rows keep this correct; tomorrow, a new `Vec<NestedRef>`-shaped field with an empty fixture value is
   invisible to the compiler, the walk test (empty vecs serialize away), and every derivation. At minimum: fix the
   doc, require `#[relation(none)]` (or an explicit `via` declaration) on Option/Vec-of-struct fields, and add a
   trybuild case for the undeclared via shape.
4. **`check` reaches the graph validator only incidentally.** The plan (E) says `validate_graph_scope` is called
   "from `check`"; there is no such call (`crates/provenance-cli/src/handlers/check.rs`, `validate_locked`). The
   refusal I observed empirically arrives because `check`'s ideation loader
   (`handlers/check/scope/ideation.rs:19,24`) calls `list_proposal_cards_with_actor_ids` →
   `project_proposal_cards` → `validate_graph_scope` (`state_store.rs:301`). A refactor of the ideation load path
   silently deletes required-list and requirement-cycle refusal from `check`. Call it directly from
   `validate_locked` and the indirection risk disappears.
5. **The branch I was given to review is not the PR head.** Deviation 13 says the parity harness and fixtures were
   never committed and "the branch history was rewritten so no commit ever carried them". The fork-origin head
   `9302f1b` I reviewed carries all of it: 53 files under `crates/provenance-cli/tests/relation_cut_parity/`
   (before/after state copies, snapshots, oracle, expected-diff), added at `905f970`/`aaacb64`, never deleted. The
   PR head on quality-sh (`5ddf317`, per the GitHub PR object) does differ from `9302f1b` by exactly those files,
   so the production diff under review is identical either way — but the body's statement is false of the branch
   head the task names, and the two heads must be reconciled before merge.
6. **PR body misreports the pre-existing failure symptom.** The body says both pre-existing failures on main report
   "qualifying proposal prop_q_relations_record_owned requires an assertion". On a `be9b52c` worktree the lib test
   actually fails with "assertion claim claim_q_relations_record_owned must have exactly one owner"
   (`state_store/tests/legacy_coexistence.rs:78`), for a different reason than the CLI test (see item 7). Cosmetic,
   but the diagnosis in the body is wrong for one of the two tests.
7. **Pre-existing failures: root cause is the tests, not the state or the validator; follow-up, do not fix here.**
   Both reproduce identically on `be9b52c` (verified in a separate worktree) and are untouched by this cut.
   - `legacy_coexistence::modern_lifecycle_coexists_with_frozen_shipped_records` copies the live committed
     `.provenance/state` into a tempdir and then **replaces** `contributions.jsonl` and `synthesis_packets.jsonl`
     wholesale (`write_jsonl_atomic(..., &[contribution])`, `legacy_coexistence.rs:38-56`), destroying the live
     landed tournament's claim owner; the aggregate then refuses the live assertion
     `assertion_q_relations_record_owned`. Test bug: it reads live state it must not clobber.
   - `cli_import_legacy_audit::exact_shipped_promotion_decisions_export_is_accepted` runs `provenance export`
     against the live repository (`cli_import_legacy_audit.rs:231-246`), strips `dispositions` into
     `promotion_decisions` and deletes `assertion_records`; the live state now contains a modern landed tournament
     whose `prop_q_relations_record_owned` is qualified by `synth_q_relations`, so the stripped import leaves a
     qualifying proposal without an assertion and the validator rightly refuses. Test-fragility bug: it assumes the
     live scope holds only shipped-terminal legacy rows. (The state is fine — `check` is clean on it; the
     proposal's `proposed` state beside an accepted modern disposition is a shape the first test in the same file
     explicitly anticipates.)
   Recommendation: a follow-up bead making both tests synthetic (or filtering to shipped rows like the file's own
   first test does). Not this PR's obligation; the cut neither causes nor worsens them.
8. **Neighbor list can duplicate rows on un-deduped state.** `references()` walks every list entry
   (`relations/front.rs:250-266`), so a hand-edited shard with the same target twice in one list yields two
   identical `Neighbor` rows where the old edge shard's write-time dedupe guaranteed one. Cosmetic, dirty-state
   only; the writers still sort+dedupe (`reference_writers.rs:140-143`).

## Deviation verdicts (the thirteen)

1. **Trace deltas larger than plan L14 — sound.** Plan E itself says trace and neighbors follow `none` relations
   "as `direction` admits"; only L14's BFS delta prediction omitted them. The in-tree expected-diff enumerates the
   real delta (392 origins changed, 8174 nodes added, 14 horizon drops) and the harness proves the cut binary
   equals the oracle after-model. Not dropped scope — a corrected estimate.
2. **Cross-scope references dangling after the cut — sound.** Pre-cut edges carried one `scope_id` and were
   scope-filtered, so cross-scope relations were already unreachable per scope; the live state has none (check
   clean). Declared behavior change with no live-data impact.
3. **Oracle boundary-`cites` flow fix — sound, not oracle-bending.** Plan C declares boundary `source_ref` flow
   `none`; the K.4 fix moved oracle and code **to** the plan. The before side stays pinned to real pre-cut binary
   captures (`before_snapshots_match_the_oracle`, green here), so the diff could not be bent without breaking that
   test. Impact deltas 143/12, 149/18 match the committed expected-diff.
4. **Six wiki routes instead of byte-identical wiki — sound.** The plan's own G.5 needs-union necessarily changes
   the two resolution pages; enumerating the six moved routes and asserting all other digests byte-equal is a
   strict adaptation of an inconsistent plan prediction. Verified green here.
5. **Eight live-state export tests failing K.4-K.7 — sound.** Transitional, self-healed at K.7; head is green.
6. **Generic dangling pass reports `domain_id` — sound.** `domain_id` is a declared relation; reporting it is what
   plan E's "one generic pass over `declared_relations()`" says. Fixture probes confirm the wording.
7. **`ProjectionFamily::Edges` tests left at K.5 — sound.** The tests could not outlive the writer change; the
   family died at K.6 as scheduled.
8. **`rule_prov_relation_vocabulary_closed` statement in two sentences — sound.** ASD-STE100 gate is itself a
   repo rule; the K.8 ledger shows the checker forcing the split.
9. **New requirement refines the same parent — sound.** Verified in state: `refines ==
   req_create_an_end_to_end_product_research_to` on both, `supersedes` names the anchor. Keeps the root count
   stable; a reasonable addition beyond plan H's field list.
10. **`check` edge-pass tests left at K.7 — sound.** Their fixtures could not express version 2.
11. **TypeScript protocol literals — sound but untidy.** `STATE_SCHEMA_VERSION = 2` exists
    (`packages/provenance/src/protocol.ts`); mock engines still carry literals. Cosmetic follow-up.
12. **Converter not committed — sound.** Exactly what the plan and the owner ruling require; I re-ran file 05
    myself (below) and reproduced the PR's converter report line for line.
13. **Parity harness "not committed" — contradicted on the branch under review.** See finding 5. On the PR head it
    is true; on `9302f1b` it is not.

## Checked, clean

- **Derive shapes.** Probe crate over the real macro: bare → `required: true`, `Option` → optional, `Vec` → list,
  `via` → row emitted; `#[relation(none)]` takes no other keys; unknown key, missing target, bad field type all
  refused (trybuild cases + stderr pins). All four trybuild cases green at head.
- **Declaration tables.** 7 kinds, 18 rows + hand-walked `links` = 13 names over 20 declarations; hand-list test
  (`relation_tables.rs`) and serde-walking test (`relation_walk.rs`, 7 kinds, nested via-structs) green.
- **Walk direction.** Flow semantics match plan C for all 20 declarations, including the three inverted storage
  directions (cites, produces→lists, refines/spawned_by) — verified by reading `flow_neighbors`
  (`relations/front.rs:147-172`) against the table and by `relation_traversal` tests; the parity harness compares
  neighbors/trace/graph/traceability/impact/health for every record under `direction: both` and passes, and
  `expected-diff.json` (in-tree, 95 KB) enumerates exactly the oracle delta (`expected_diff_enumerates_the_oracle_delta`
  green). Direction `out`/`in` probed by hand on the CLI: correct partitions, no node lost or gained beyond the
  enumerated diff.
- **Ownership add/clear.** Self, mutual, and three-node cycles refused on refines/depends_on/supersedes (probed);
  `clear` of a required list's last entry refused for rule and resolution with the plan's wording (probed);
  `supersedes` written on the newer record leaves the older record untouched (probed); `spawned_by` lands on the
  requirement (probed); sorted+dedup on write.
- **Contradiction.** Settled by `resolution_id` or by either side listing the other in `supersedes` (both probed);
  the shared-resolution settle path is dropped as the plan demands; an answered question without a resolution
  still reports the pair gap, per plan C; contradicts questions excluded from `OpenQuestion`; the unordered pair is
  the gap identity; the live pair stays an open gap exactly as the expected diff says.
- **Derived relation table.** Per-scope load from fields including `links` rows (`relation_rows.rs`); catch-up
  equals full rebuild with the `relations` table in the compared dump after every family invalidation, hand edits,
  ideation writes, and a departed scope (`catch_up_behavior.rs`, `catch_up_domain_coverage.rs`,
  `relation_rows_behavior.rs`, all green); departed scopes lose their rows; `ProjectionFamily::Edges` gone, `ALL`
  is 18, the global unit only updates its digest row; scope-locality guard green.
- **Migration 021.** Drops edges table/indexes/digest row and both `superseded_by` cache columns, creates
  `relations` + both indexes; any applied migration routes catch-up to a full rebuild
  (`catch_up.rs:57-59`, `a_schema_move_routes_catch_up_to_a_full_rebuild` green).
- **The conversion (ran file 05 myself against a copy of main's state).** Edge counts in: 614 = 76 references, 19
  refines_into, 0 depends_on, 0 supersedes, 1 contradicts, 98 needs, 96 resolves, 1 spawns, 323 produces, all
  scope `default`. Report matches the PR's converter report line for line: 2 needs-only pairs unioned into
  `requirement_ids`, 3 field-only citations (`src_annotation_format_spec`), the one supersession carried to the
  newer record (`res_state_is_jsonl_in_git.supersedes += res_convex_chosen_as_backend_over_custom_web`), the
  minted topic/question with the deterministic ids, `explored`/`open`, no `resolution_id`, superseded
  `res_rule_is_the_function` left. Family deltas: edges 614→0, questions 9→10, topics 5→6, everything else
  unchanged; 731 records rewritten. Rerun idempotent (byte-identical); rerun on the already-converted state a
  no-op for content. Output is byte-identical to the branch's committed `.provenance/state` except the three K.8
  record edits, which sit on top as K.7→K.8 sequencing requires. Version rewrite reaches every family, the
  manifest, threads, and nested landing records (nested `schema_version: 2` verified).
- **Frozen audit digests.** Recomputed both with the recipe (sort by id, serde-serialized row bytes + newline,
  SHA-256) from main's and the branch's rows: terminal proposals `f3438033…86f843` → `eb8f351c…462843`;
  disposition audit `8f25c3f3…c90c9cfb` → `995184bc…23b69b`. Both match the new constants
  (`legacy_audit.rs:19-22`) and the PR body.
- **Check, gaps, validator, merge gate.** Planted dangling targets per class (single, list, via-struct, links) are
  refused by `check` and reported by gaps with the plan's wording (`"<relation> points at missing <kind> <id>"`),
  including `domain_id`; thread parent pass intact. Empty required list refused by `check` and `export`
  (empirical). Requirement `refines` cycle refused by `check` and `export` (empirical). Merge gate deserializes
  all seven owner families as their types and enforces required lists (`merge/validation.rs:114-138,182-194`);
  its named tests green. `OrphanRule`/`OrphanResolution` deletions justified: writer refuses (create + last-entry
  clear), validator refuses, merge gate refuses, `check` refuses, import goes through `check` — no gap category
  disappeared without both a writer and a validator refusal in place.
- **Wire.** Protocol 6 rejects a version 5 request ("request names protocol version 5; this engine speaks 6");
  neighbors/trace filters take relation names and reject the old edge type names ("unknown relation
  `refines_into`", probed for `needs` too); `Neighbor.relation`/`direction` carry the walked relation, direction
  partitions correct; `SDK_PROTOCOL_VERSION = 6`. Export carries no edges family (18 keys, probed); import's edge
  branch and `ScopeExport.edges` gone; a v1 export is refused on its `edges` key. Graph reference v2: issued on
  the converted fixture state (`schema_version: 2`, `grf1_`/`git1_` ids) and the reference verification suite is
  green. TypeScript: `EdgeType`/`Edge` deleted, `Neighbor.relation`, `relations` filters, `STATE_SCHEMA_VERSION`,
  declaration fields (`supersedes`, `refines`, `depends_on`, `spawned_by`, `resolution_ids`) all present; TS suite
  green here (runtime 86/86, types, packed).
- **Typed declarations.** `reconcile/references.rs` implements the plan's rule exactly (named field authoritative,
  omitted field untouched, `source_refs` append preserved, canonical `spawned_by`/`resolution_ids` must exist,
  acyclicity enforced); `typed_references.rs` pins "an omitted refines is untouched", round-trip, and refusal
  cases — green.
- **The six mutations, replayed here.** (1) drop `#[relation]` on `Rule.resolution_ids` → compile fails with the
  exact declared message; (2) skip the rules kind in the loader → `materialize_derives_one_row_per_declared_reference`
  red; (3) `Rule.requirement_ids` optional → the three named tests red (writer's own check green — see blocking
  item 1); (4) backfill a `refines` as `depends_on` in the converted fixture →
  `cut_binary_over_converted_state_matches_the_oracle` red; (5) drop the topic table from
  `declared_relations()` → `every_owner_kind_appears_once_in_the_declared_tables` red; (6) invert the
  requirement's `cites` flow → `flow_follows_each_declared_flow_and_skips_none_relations` red. All restored after
  each run.
- **Suite state at head.** `cargo test --workspace --all-features --no-fail-fast`: 2 failures, both the
  pre-existing tests of item 7, failing identically on `be9b52c`; everything else green including the parity
  harness and trybuild. `provenance check` clean on the converted live state. TS suite green.
