# Adversarial review: relation shapes cut plan, revision 1 (GLM-5.3-flash)

- workflowd run agent-run/54d5814cee0a9ac2 (commit 0daa6e5), dispatched from Claude Code session https://claude.ai/code/session_01NQ2v24jQRt4G9Yzu2LFMcc; reviewed d97560a

---

# Adversarial review: relation shapes cut plan (GLM)

- Reviewer: GLM adversarial plan review, dispatched by the owner
- Date: 2026-09-03
- Reviewed: `docs/plans/2026-09-03-relation-shapes-cut.md` at `d97560a` on `1wh-cut-plan`, against
  `res_relations_are_fields_or_action_records` and `res_catch_up_hashes_scopes_no_journal`
  (`.provenance/state/scopes/default/resolutions/res.jsonl`), gist `c07c1e3` (files 04, 06, 07),
  and the code at `d97560a` (= `ce891fe` code plus the plan and one question record)
- Method: code reading with file:line evidence, probes over `.provenance/state`, and a
  reproduction of the section B macro spike in a scratch crate (`/tmp/opencode/relspike`;
  nothing committed by the reviewer except this report)

## VERDICT: REJECT — do not implement `d97560a`; execute the revision 2 already on this branch

The plan is faithful to the resolution on every shape and scope decision, no compatibility
hedge survives reading, and every count I recomputed reproduces exactly (614 rows:
76/19/0/1/0/98/96/1/323; 0 labels, 0 dangling, all scope `default`; 14/4/3 multi-producer
cases; 49 roots; 3 field-only citations, all `src_annotation_format_spec`; 2 needs-only
pairs, both from `req_rust_requirements_as_code_authoring`; 1 `superseded_by`; 0 rules and
0 resolutions lack producers). But five defects would each produce wrong code or an
unmeetable acceptance gate if an implementer executed this revision as written: the parity
harness contradicts sections E and F (finding 1), the traversal-direction rule blinds
`impact` (finding 2), the commit sequence breaks its own green-suite promise (finding 3),
the converter fights the global version guard (finding 4), and the declaration grammar
cannot express bare `StableId` keys or `links` (findings 5-6). A revision 2 addressing
exactly this defect set already exists on this branch (`ce495d5`, re-reviewed approve-with-
amendments at `746d459`); my independent verification confirms its diagnosis. Land that, not
this commit, plus the residuals in the prior-review section below.

## Findings, most severe first

1. **The differential harness (I) cannot be green as written, so "Nothing else may differ"
   (plan:341) is hollow.** F says a record's neighbors are "every declared relation in both
   directions" (plan:234-235) and E moves `impact` to "related_nodes over `RecordFront`"
   (plan:200-201); today `neighbors`/`trace` walk edge rows only (`walk.rs:11-19,64-96,98-147`).
   After the cut the walk output gains every field-derived relation with no edge-row
   counterpart: 67 `domain_id` rows, 79 citation rows, 5 topic, 3 boundary, 9+9+4 question
   rows, 1 `contradicts`, and the G.9 `supersedes` row; `needs` (98) disappears. The
   expected-delta file (plan:339-341) names only prime's 3 field-only citations and the
   contradiction question. No uniform reading of "compare as sets of (owner, relation,
   target) triples" (plan:337-338) equates those sets. Owner sides also flip for
   `references`/`refines_into`/`spawns`/`produces` and name-mapping does not repair
   direction. One saving property the plan does not claim: the `needs ∪ resolves` union makes
   the folded needs set exactly equal the post-union `requirement_ids` set (98 = 96+2), so
   the needs fold is parity-sound — the field-derived additions are not.

2. **Traversal direction is undefined once storage inverts; `impact` from a Source returns
   nothing as specified.** E: "`queries/impact.rs:27,34-58` walks the same way, out-direction
   only" (plan:216), repeated in the W3 text (plan:368-370). Out-only works today because
   `references` runs source→requirement (`queries/impact.rs:34-36` walks `edge.from_id ==
   origin`; `cache/impact.rs:81-83,98-105` define Downstream=`to_id`). Post-cut a Source's
   only out-relation is `supersedes` (C row 83; 0 rows), and from a Requirement no
   out-relation reaches a Rule (`requirement_ids` lives on the rule; `produces` inverted).
   No `RelationDecl` field carries a flow orientation, so "the declaration drives every
   derivation" cannot repair this. The harness snapshots `impact per requirement and source`
   (plan:334) and would go red with no correct resolution available.

3. **K.3/K.4 break the "each commit builds and passes the workspace suite" promise
   (plan:384).** K.3 lands writers that "write fields only and stop writing edges"
   (plan:390-391) while readers and adoption still read edges: adoption equality compares
   declaration against edge rows (`typed_specs/adoption/relationships.rs:59-148`; edges
   loaded at `typed_specs.rs:284-287`), so every `sdk apply` adoption test is red; write-
   then-gap tests (`cache/tests/frontier_behavior.rs`, `gap_rule_behavior.rs`, wiki assemble
   tests) are red for the same reason. Section I's "49 Rust ... test files ... each
   rewritten" (plan:359-360) is assigned no step. Fix: double-write at K.3, or fold K.3/K.4.

4. **The converter cannot read v1 through the store once the constant is 2, and the harness
   cannot validate a v2 fixture while it is 1.** The guard refuses `version !=
   SUPPORTED_SCHEMA_VERSION` in both directions (`readers.rs:143-155`; manifest via
   `state_store.rs:100`). G's converter runs through the store and then "run[s] `check`; run[s]
   `materialize`" (plan:295-296); K.7 flips the constant and converts in one commit
   (plan:396-398) — at 2 the store refuses the v1 live state the converter must load; at 1
   (its K.3 landing, plan:390) the harness's "green against the converted fixture" (K.4,
   plan:393) fails `check`/`materialize` on v2 records. Fix: the converter reads raw JSON
   below the guard, writes the constant, and K sequences the flip.

5. **Section B's declaration grammar is contradictory about bare keys and kind count.** B
   says the concatenation is "(5 kinds)" (plan:38) and the attribute grammar covers only
   `Option<StableId>`/`Vec<StableId>`/`Vec<T> via` (plan:27-28). C's table carries seven
   kinds with reference fields (requirement, resolution, rule, source, question, plus
   "as today" topic/boundary rows, plan:85-86), and question's `topic_id`/`requirement_id`
   are bare `StableId` (`shaping.rs:146-149`) — undeclarable under B's grammar, yet E makes
   dangling and `check` "one generic pass over `declared_relations()`" (plan:192,224-225),
   which requires them declared and replacing `add_topic_refs`/`add_question_refs`
   (`dangling.rs:74-139`). Meanwhile I lists "a `required` single" as a trybuild compile-fail
   (plan:358-359), contradicting a legal required single. Pick: seven kinds, bare = required
   single, trybuild case removed.

6. **`links[]` cannot be declared by the attribute as designed.** `ArtifactLink` carries a
   per-entry `target_type` (`shaping.rs:105-110`); `#[relation(target = Kind)]` names one
   kind. `RelationKind::TopicLinks`/`QuestionLinks` die with the enum (plan:55-56) with no
   replacement stated, and `check_artifact_links` (`check/references.rs:39`) is unmentioned.
   State that links stay hand-written/hand-checked outside the declaration.

7. **The version bump's blast radius is understated.** F names five TS literals (plan:255-256)
   and misses the four production `SchemaVersion(1)` writers: `model/manifest.rs:36` (the
   `init` default — a v2 binary would refuse the manifest it just wrote), `scope.rs:70`,
   `threads.rs:60`, `handlers/rules.rs:145`. Risk 9 (plan:439-440) covers state only. Amend:
   production sites use `SUPPORTED_SCHEMA_VERSION`; test literals get a grep gate.

8. **Section J edits documents that are not in the repository, and K never schedules it.**
   No W3/W5 plan exists under `docs/` at `d97560a` (`docs/plans/` holds only this plan).
   J cites "plan lines 21, 39, 47-48, 107, 273-440" (plan:377) with no path or branch, and
   K.1-K.9 contain no step that applies J. Name the target (or move the rewrite to beads
   1wh.2/1wh.3) and give it a step.

9. **Risk 8's error message names a command K.9 deletes** (plan:437-438 vs plan:400-402),
   and the refusal is not "the schema version guard": `ScopeExport` is `deny_unknown_fields`
   (`export.rs:8`), so a v1 export dies on the `edges` key with a serde message. Fix both.

10. **The citation's relation name is never stated.** The wire `relation: String` (plan:229-230),
    the harness's name map (plan:336-337), the `relations` table, and `docs/cli.md` all need
    it; C's `source_refs` row (plan:74) names none — nor do the "as today" rows
    (`domain_id` → `requirement_in_domain`?). Enumerate the post-cut vocabulary; it is the
    exact surface the wire, the table, and the W3 filters share.

11. **Reconcile semantics for the new declaration fields are unspecified.** D specifies only
    `desired_rule` (plan:147-148; `typed_specs/reconcile.rs:250-274`). Whether a re-applied
    spec that omits `refines` clears a CLI-set value is undecided (`desired_requirement`,
    `reconcile.rs:142-174`); adoption risk 6 (plan:431-434) covers equality, not reconcile.
    `TypedSourceInput` gaining `supersedes` (C row 83) is unaddressed. Default to
    present-is-authoritative, absent-untouched, and say where it lands (reconcile.rs is at
    476 lines — name the split).

12. **Deletion/reader-list omissions.** `CreateEdgeInput` (`inputs.rs:54-61`),
    `StateStore::list_edges`/`closed_edges` (`state_store.rs:167-168,244-245`),
    `graph_records::load_edges` (`graph_records.rs:182-195`),
    `cache/tests/projection_digest_sensitivity.rs:29`. Also: the aggregate validator that
    `materialize` and direct writes run reads ideation families only
    (`ideation_batches.rs:143-185`), so L1's required-list refusal has no home there unless
    reads are added — name the refusal homes precisely (check, merge gate, aggregate validator).

13. **The deslop rule is overbroad** (plan:411-413): "no `edge` word left in production code,
    comments, or docs" would rewrite `lineage_validation.rs:164-165,288` (graph-theory "back
    edge" in proposal lineage), historical ADRs, and SDK parity notes. Exempt non-relation
    uses; add the doc sites H misses (`docs/shaping.md:233-256`, `docs/typescript-sdk-poc.md:8`).

14. **Spike mechanics the plan under-specifies** (from my reproduction): a module-level
    `const RELATIONS` collides when two structs share a module — it must be an associated
    const (`Requirement::RELATIONS`); and a generic function cannot reach the const through
    `RelationOwner` as specified — the trait must carry the table
    (`fn relations() -> &'static [RelationDecl]`). B's "From those two, one generic function
    each gives..." (plan:31-34) is not implementable as literally written.

15. **Minor.** (a) The relation-map test "runs the authoring command" per row (plan:344-347),
    but map rows 13 (question_refines — derived) and 30 (duplicate_of — no path) have none.
    (b) G step 8 never states the minted topic's required `requirement_id` (`shaping.rs:129-130`).
    (c) The new dangling text (plan:193) differs structurally from today's
    (`dangling.rs:196-206` carries the edge row id) and is never exercised — the fixture has
    0 dangling rows. (d) Idempotence holds only for completed runs; a crash between mint and
    shard-delete double-mints on rerun — say the recovery is git reset. (e) Plan:356 says
    "the four catch-up equivalence suites"; five `cache/tests/catch_up_*.rs` files exist.

## Answers

1. **Fidelity.** The position holds sentence by sentence: no canonical edge (A); field or
   action record (C); required/optional by type (C, enforcement at writer/check/merge gate
   per L1 — the only reading serde permits); reverse lookups derived, projection for served
   reads, canonical readers from fields (A, E); declaration drives writer/validator/gap/
   projection (B, D, E — with findings 5, 6, 10, 14 showing where the driving is still
   hand-wired); rule lists ✓; resolution list + `spawned_by` ✓; requirement
   `refines`/`depends_on`/`supersedes` ✓; root = no `refines` (plan:101-102, 49/68) ✓;
   source/resolution `supersedes` on the newer record with `superseded_by` deleted and no
   dual form (plan:92-98) ✓; citation stays ✓; contradicts as a question settled by
   resolution or supersession (plan:104-107) ✓ — dropping today's shared-resolution settle
   path (`contradiction.rs:49-57`) is required by the shape and stated; needs dropped with
   the union backfill and report (plan:275-279) ✓; add+clear per replaced reference ✓; one
   cut, convert once, bump both versions, delete shard and types, retire the endpoint-table
   rule, build the declaration ✓; raw JSONL read only by code — untouched, no violation. No
   compatibility hedge anywhere; old references and v1 exports are refused outright
   (plan:240-241, 243-244, 437-438). Amendments: Fable A1-A5, A7, A8, A10 are in; A6 is
   superseded by the resolution's question shape (with GLM's correction — a dedicated
   `contradicts` field, not `links`); A9's order is violated inside the PR (finding 3). GLM
   1-9, 11-13 are in; GLM 10 is the same violation; GLM 14 is answered by table C
   (`MissingSourceRefs`/`MissingDomainId` stay gaps; `OrphanRule`/`OrphanResolution` die
   with the required lists).

2. **The declaration spike.** Reproduced in a scratch crate (syn 2, quote, proc-macro2 from
   the lock's cached versions; two structs shaped like section C's `Requirement` and `Rule`).
   It compiles and emits exactly what B claims: per-kind `RELATIONS` tables (owner kind,
   name, target kind, list, required) and `references()` flat scans, from which one generic
   function each produced the reverse scan, the derived-table rows, the dangling report, and
   the empty-required-list refusal. `syn` reads the field types as claimed (`Option` single,
   `Vec` list, `Vec<Struct>` with `via`). What the macro cannot see: any other struct (the
   vocabulary assembly is hand-written); requiredness at deserialize (confirmed: an empty
   `requirement_ids` deserializes without complaint); a reference-typed field with no
   attribute (silently skipped). The guards narrow rather than close: the concatenation test
   as specified catches duplicates only, never an omitted kind (my spike's owner-set assert
   is exactly the weak form); the serde-walking test is fixture-quality-dependent
   (`skip_serializing_if` hides unpopulated fields) and its recursion into nested structs and
   its kind scope (five vs seven) are unspecified. Two mechanical corrections fall out:
   associated const, trait carries the table (finding 14).

3. **The shapes (C).** Every row checks against the resolution and map rows 1-9, 15-21:
   names, target kinds, required/optional, single/list. Lists are forced by the data (14/4/3
   — recounted); singles are safe (19 rows, 19 children; 1 spawn). `superseded_by` becomes
   `supersedes` on the newer record and is deleted from both structs, both `--superseded-by`
   flags (`cli/knowledge.rs:28-29`, `cli/policy.rs:42-43`), both inputs
   (`inputs.rs:23,116`), both SQL inserts (`graph_records.rs:28-32,146-153`),
   `dangling.rs:38-72`, and the wiki pages — `CreateProposalCardInput.superseded_by`
   (`inputs.rs:317`) correctly untouched. Root = no `refines`, no marker. `depends_on`
   optional list on the dependent requirement per the owner ruling. Missing: the citation
   relation name (finding 10), bare-key and `links` declarations (findings 5-6),
   `TypedSourceInput.supersedes` (finding 11).

4. **Commands (D).** Complete and in product words: every replaced reference has add (set
   for singles) and clear — `refines set|clear`, `depends-on add|clear`,
   `requirements/resolutions/sources supersedes add|clear`, `spawned-by set|clear`,
   `source-ref clear`, `rules requirement/resolution add|clear`,
   `resolutions requirement add|clear`, `questions contradicts set|clear` — filling the
   eraser hole (`edges delete` was the only removal path for five types;
   `source-ref` had no remove, `cli/knowledge.rs:172-188`). Removals verified
   (`cli/graph.rs`, `handlers/edges.rs` 57 lines, `check/edges.rs` 45 lines called at
   `check.rs:145`, `cli.rs:123-126`, `handlers/mod.rs:15,117-118`). Nothing still needs
   `edges create`: every map row 2-5 and 8 act now has a command or create flag, including
   the resolution-producer-added-later case (`rules resolution add`). Unspecified: cycle
   refusal for `refines`/`depends_on`/`supersedes` (wiki lineage walks parents,
   `traversal.rs:77-108`) — state the default.

5. **The derived table and readers (E).** I grepped every consumer of edge rows (88 Rust
   files, 3 TS) against sections D, E, F, H, K: all production consumers appear with a
   destination except finding 12's omissions. All section E file:line anchors verified as
   cited (graph_query 285, frontier 135, contradiction 66, dangling 225, state_adapter 165,
   prime 149, impact 126, traceability 115, health 272, walk 185, merge/validation 328,
   plan.rs:131-139, requirement_reviews.rs:132-149). Per-scope load is correct: each
   `relations` row derives from exactly one owner record with no join, no cross-scope rows
   exist, `rederive_scope`/`remove_departed_scopes`/`Unit::Global` handling
   (`catch_up.rs:148-225`) is consistent with `res_catch_up_hashes_scopes_no_journal` (18
   scoped families; the global unit shrinks to manifest+dictionary; the locality guard at
   `scope_locality_guard.rs:132-147` becomes scope-only). Catch-up equivalence holds if the
   suites compare `relations` table content — it has no digest row, so digests cannot catch
   a skipped owner kind; the plan's mutation target implies content comparison, say it.

6. **Wire (F).** Enumeration verified anchor by anchor: protocol 6 (`protocol.rs:25`,
   `engine.ts:17,35-38`), `Neighbor.edge_type`→`relation` (`node.rs:107-113`,
   `protocol.ts:246-250`), `edge_types`→`relations` on both queries (`query.rs:78,97`,
   `protocol.ts:256`), `EdgeType`/`Edge` deleted (`graph.rs:62-135`, `protocol.ts:186-195`,
   `index.ts:122`), graph reference v2 (`projection.rs:29-48,89,120`,
   `graph_reference.rs:85,337`, schema artifact `:49,64,231-236`), export/import
   (`export.rs:27,59-63,94-132`, `import.rs:40,52`, `scope_writer.rs:52,111-189`). The typed
   declarations gain exactly the four requirement fields plus `resolution_ids` and the
   required rule list. Gaps: the engine refusal of a rule with no requirement is NEW
   behavior pinned to a stale anchor (`typed_specs.rs:333-358` is the generic version check —
   say it is new and where it lands); `TypedSourceInput.supersedes` unaddressed; the five TS
   literals are not the whole version story (finding 7); reconcile semantics (finding 11).

7. **The conversion (G).** I counted the rows myself at `d97560a`: 614 total, per type
   76/19/0/1/0/98/96/1/323 — exact match; 0 labels, 0 dangling, all scope `default`; 3
   field-only citations (all `src_annotation_format_spec`); the 2 needs-only pairs match; 0
   edge-only pairs; 19 refines rows over 19 distinct children; 1 spawn; 0 rules and 0 of 94
   resolutions lack producers. Lossless per type given the union for needs (the union makes
   the folded set exact). Idempotent for completed runs; deterministic ids are valid
   (`StableId` checks charset only, `ids.rs:22-25`; the question id is 73 chars — no cap).
   An already-converted repository is a clean no-op. The version rewrite touches every
   record in every family including those with no relation fields (ideation, threads,
   messages, bindings, reviews — rewrite only), the manifest and nested landing versions
   included. Mechanism defect: finding 4 (converter vs guard); crash-mid-run caveat
   (finding 15d).

8. **Tests (I).** The harness is not byte-identical-capable as specified (finding 1): the
   expected-diff file understates the deltas, so the implementer either fails it or stuffs
   real changes into it — the exact failure a differential gate exists to prevent. The
   relation-map test is executable except map rows 13/30 (finding 15a). The mutation targets
   are the right five (attribute drop, loader skip, optional required list, wrong backfill,
   concatenation row) with the caveat that the concatenation target needs a hand list of
   expected kinds to catch omissions. Untested but required by the resolution: `clear` on
   the last entry of a required list refuses (stated in D/L1, absent from I); the merge
   gate's new family checks; the new dangling wording; contradiction settled-by-supersession.

9. **W3/W5 (J), sequencing (K), 500-line, risks (L).** J's targets are unlocatable from the
   repo and unscheduled in K (finding 8). Sequencing: K.3/K.4 not green (finding 3); K.7
   cannot run as ordered (finding 4). File splits are named for writers (`state_store/
   reference_writers.rs`), CLI (`cli/references.rs`), and adoption; sizes verified
   (`writers.rs` 321, `cli/knowledge.rs` 188, `reconcile.rs` 476, `typed_specs/adoption.rs`
   483, `graph_reference.rs` 422) — but no split is named for `reconcile.rs` if it gains
   requirement-field reconcile (finding 11). Every risk L1-L13 carries a default except the
   risk-8 message defect (finding 9).

10. **Vocabulary.** No invented or agent-flavored term reaches a command, flag, field, doc,
    or error message in this plan. `relation` on the wire and in gap text is sanctioned by
    the resolution's own wording. Two watch items: "X-class rows" (plan:346-347, 449-450) is
    tournament-internal shorthand — keep it out of the declaration table and docs; the
    surviving risk-8 message names the deleted `convert-edges` (finding 9). The gap text
    `"<relation> points at missing <kind> <id>"` replaces today's varied per-family texts
    (`dangling.rs:60-65,81-85,104-107`) — a permitted wording change, but an output change
    the fixture never exercises (finding 15c).

## Prior review (Fable, `b8ce42f` against this same commit; re-review `746d459` of revision 2)

The branch moved while this review ran: `1wh-cut-plan` now carries the Fable reject of
`d97560a` (`b8ce42f`), plan revision 2 (`ce495d5`), and the Fable re-review approving
revision 2 with five amendments (`746d459`). I read them after completing my independent
verification, as instructed.

**Agree** (independently derived before reading, verified against the code): its findings 1,
2, 3, 5, 6, 8, 9, 10, 11, 12, 13, 14 are the same defects I found by the same evidence, and
its finding 4 (converter vs global guard) is one I missed — I verified it myself
(`readers.rs:147-155` refuses `version != SUPPORTED` in both directions; `state_store.rs:100`
extends the guard to the manifest) and fold it in as my finding 4. Its spike reproduction
agrees with mine on capability and on what the derive cannot see; its checked-clean list
reproduces the same counts.

**Disagree:** its checked-clean line that the new gap text "matches today's wording
(`dangling.rs:80-85`)" is wrong — today's text carries the edge row id and a direction word
(`dangling.rs:196-206`); the plan's text is a real wording change (parity-safe only because
the fixture has 0 dangling rows). Precision note on its finding 2(a): the `produces` owner
flip changes prime/traceability triples but not neighbor sets (the queried record keys the
neighbor triple); the normalization table it prescribes handles both regardless.

**What it missed:** the two spike mechanics (associated const; the trait must carry the
table — finding 14); the relation-map rows with no authoring command (15a); the minted
topic's required `requirement_id` (15b); the new dangling wording never being exercised
(15c); the crash-mid-run idempotence caveat (15d); `TypedSourceInput.supersedes` as part of
the typed-declaration gap; "four" vs five catch-up suites (15e).

**On revision 2:** I spot-checked its section B and the re-review's status table. It adopts
the flow orientation, seven kinds, the bare-key rule, the compile-time guard for undeclared
`StableId` fields, the `links` exemption, the harness normalization table, the K reorder,
and the converter-below-guard mechanism. The re-review's five amendments are precise; I
confirmed their factual basis (4 questions with `resolution_id` at `ce891fe`; the boundary
`source_ref` neighbor; the G.9 `supersedes` row being new adjacency). Execute `ce495d5` with
those five amendments; add the residuals above (findings 14-15 apply to revision 2's design
unchanged where they concern the spike mechanics, the map test, and the topic id).

## Checked, clean

- Every count in the plan reproduces at `d97560a` (614 by type; 14/4/3; 19/19; 1; 49/68
  roots; 3 field-only citations; 2 needs-only pairs; 1 `superseded_by` on
  `res_convex_chosen_as_backend_over_custom_web`; `res_rule_is_the_function` superseded with
  no successor; 0 dangling; 0 labels; all scope `default`; 0 of 167 rules / 94 resolutions
  lack producers).
- ~60 spot-checked file:line anchors in sections B, C, D, E, F, G, H, K all exist at
  `d97560a` as cited, including all thirteen line-count claims.
- `SUPPORTED_SCHEMA_VERSION` is one global constant (`aggregate_validation.rs:19`); the guard
  refuses both older and newer versions (`readers.rs:143-155`), and covers the manifest
  (`state_store.rs:100`) and nested landing records.
- The contradiction question's parity story holds: excluding a `contradicts` question from
  `OpenQuestion` (`frontier.rs:94-116`) while keeping the pair gap avoids double reporting;
  `explored` raises no `UnexploredTopic` (`frontier.rs:118-135`); `grill` is a valid
  `ResolutionMethod`; `q_contradiction_<from>_<to>` is a valid `StableId`.
- `projection_families.rs` anchors (30, 53, 77, 93-96, 105-106, 140-142), `catch_up.rs`
  anchors (148-177, 181-201, 205-225), and the five `catch_up_*.rs` suites exist.
- `provenance-macros/Cargo.toml` has no dependencies today; `syn`/`quote`/`proc-macro2` are
  in `Cargo.lock` via `serde_derive`; trybuild is already used by
  `provenance-sdk/tests/compile_fail.rs`.
- The scanner matches the word `rule` and reads no edge rows; `provenance-sdk` the same.
- Merge gate anchors (`validation.rs:36,62-63,103,182-194`) and `ShardFamily`'s current
  coverage (Edges/Requirements/Rules/landings) are as E describes.
- The scanner-invisibility claim for `#[relation(...)]` is consistent with
  `provenance-macros` emitting no `rule` keys.
- The spike crate lived in `/tmp/opencode/relspike` and was never committed.
