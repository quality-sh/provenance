# Adversarial review: draft position on relation shapes (GLM)

- Reviewer: GLM adversarial architecture review, dispatched by the owner
- Date: 2026-09-03
- Reviewed: `docs/proposals/q_relations_first_class_or_properties_draft_position.md` at
  `a68ffc9`, on `origin/1wh-shape-tournament` at `bf5f9c8`, with the relation map
  (`docs/research/2026-09-03-relation-ontology-by-authoring-act.md`), the three stance
  artifacts, the owner's reaction on the question thread, and the code at `bf5f9c8`.
- Method: code reading with file:line evidence, probes over `.provenance/state`, and
  `cargo test -p provenance-store` as a green baseline (280 tests). Nothing committed
  by the reviewer except this report.

## Verdict

**ACCEPT WITH AMENDMENTS.** The direction is right and the evidence holds: the 612
edge rows carry no author, date, status, or label; two double-written relations have
drifted; the act-relations built later (bindings, reviews, assertions, dispositions)
already do what the draft wants. No canonical edge, one owner per fact, reverse
lookups derived, one declaration per record kind — keep all of it. The ownership
table is wrong in three rows and internally inconsistent in a fourth, the migration
order is backwards, and the draft is silent on the one change that destroys data if
skipped (record version). Amendments, in product words:

1. `produces` becomes two lists on the rule: `requirement_ids` (required after
   backfill) and `resolution_ids` (optional). Not a singular field: 14 rules have
   more than one requirement producer today, 4 have more than one resolution
   producer.
2. `resolves` becomes a list `requirement_ids` on the resolution: one live
   resolution resolves 3 requirements.
3. `supersedes` becomes a `supersedes` list on the newer record, for source,
   resolution, and requirement (requirement gains the field; it has none today).
   `superseded_by` retires when the shard does. No one-release dual form.
4. `spawns` becomes an optional `spawned_by` field on the spawned requirement, not
   a list on the resolution.
5. `refines_into` becomes an optional singular `refines` field on the child
   requirement. Drop "required" and drop the root marker; a root is a requirement
   with no `refines`.
6. `needs` is dropped, and the backfill is `needs ∪ resolves` with a printed report
   of the 2 disagreeing pairs so their disposal is recorded.
7. `contradicts` is decided now, not left open, between (a) a question that names
   both requirements and is settled by its resolution or by either side being
   superseded, or (b) keeping the one symmetric row until an authoring act exists.
   Never a "review": that word is taken by ADR 0007.
8. Before any field lands, add a version the old binaries refuse: either a
   per-family record version check or a global bump with a one-shot rewriter. State
   in the position that today's shipped binaries silently strip unknown fields when
   they rewrite a shard.
9. Rule 3 is restated: reverse lookups are derived everywhere; the projection
   materializes them for served reads; the canonical readers (gaps, prime, check,
   wiki, plan, review fan-out, merge) derive them from fields in memory.
10. Migration order: add fields and backfill while writing both, with adoption
    equality (ADR 0008) moved to fields in the same step; then flip readers to
    fields under a parity harness; then stop writing edges; then delete the shard.
11. Every field that replaces an edge type gets its clear/remove command in the same
    step that stops writing edges, because `edges delete` dies with the shard.
12. The same change retires the `rule_prov_edge_endpoint_table` rule record and
    updates the `rule_prov_relation_vocabulary_closed` description text.
13. The declaration is the existing `RelationKind` const table extended per record
    kind, tied to the structs by a derive attribute in `provenance-macros` or by a
    struct-walking completeness test — picked, not left as "one declaration".
14. Fix rule 1's example: `source_refs` is not "optional and may be empty" while the
    `MissingSourceRefs` gap exists. Decide per field whether requiredness lives in
    the type or in the gap report.

## Findings, most severe first

1. **Singular fields cannot hold live data.** The table (draft:37-39) says `produces`
   is "a required requirement field on the rule, plus an optional resolution field"
   and `resolves` "a required field on the resolution". Probe over
   `.provenance/state/edges/edges-00.jsonl`: 14 rules have ≥2 requirement producers
   (`rule_prov_edge_endpoint_table` has 4), 4 rules have ≥2 resolution producers, and
   `res_dispositions_sole_authority` resolves 3 requirements. The writer emits one
   `produces` edge per producer (`crates/provenance-store/src/state_store/rule_writers.rs:153-172`),
   so the rows are the fact. Lists, or the backfill is lossy.

2. **The `supersedes` owner is wrong, and unwritable.** The draft (draft:35) puts it
   on "the existing superseded_by field on the older record". Three errors in one
   cell. (a) `Requirement` has no `superseded_by` field at all
   (`crates/provenance-core/src/model/artifacts.rs:284-318`); requirement
   supersession has no home except the edge. (b) The only writers that set
   `superseded_by` set it on the record being created — `sources create`
   (`crates/provenance-store/src/state_store/writers.rs:22-45`) and
   `resolutions create --superseded-by`
   (`crates/provenance-cli/src/cli/policy.rs:43`,
   `rule_writers.rs:26,55`) — and there is no update command for sources or
   resolutions (`cli/knowledge.rs:6-13`, `cli/policy.rs:6-51`), so the older record
   cannot be edited to receive the field. The record authored when the claim is made
   is the successor. (c) "any to any" misstates the endpoint table: a `Supersedes`
   edge is requirement→requirement only
   (`crates/provenance-core/src/edge_validation.rs:27-29`). The one live data point
   (`res_convex_chosen_as_backend_over_custom_web`, status `draft`, points at the
   approved `res_state_is_jsonl_in_git`) shows the field's direction is already
   muddled; the migration is the moment to fix it, not to codify it.

3. **The `spawns` owner contradicts the draft's own rule 1 and the owner's reaction.**
   The landing fan-out is: the resolution lands, then it "spawns a Requirement"
   (`docs/shaping.md:110-122`) — the requirement is the line being authored. Putting
   "an optional list on the resolution" (draft:38) edits an approved record, and no
   command edits a resolution (`cli/policy.rs:6-51`). The one live row
   (`res_ste_local_dictionary_acquisition` → `req_ste_local_dictionary_acquisition`)
   matches child-side authorship. The owner's reaction on the thread
   (`msg_1781475924576`) says maintenance should be "a byproduct of authoring"; the
   resolution-owned choice forces post-authoring edits.

4. **Version silence is silent data loss.** Every write is a read first and rewrites
   the whole shard, "dropping any unrecognised field on the way out"
   (`crates/provenance-store/src/jsonl.rs:67-76`, `state_store/readers.rs:72-77`).
   The only guard is a `schema_version` different from this build's
   (`readers.rs:86-100`), and that constant is global
   (`crates/provenance-core`, `SUPPORTED_SCHEMA_VERSION`), not per family. v0.1.0 and
   v0.2.1 both ship this path (verified in `git show v0.1.0:v0.2.1:.../jsonl.rs`). A
   teammate on 0.1 or 0.2 running `requirements fog set` on a migrated shard deletes
   every new relation field in that file. The draft's Costs (draft:59-70) and Open
   (draft:72-78) never mention record version. Note the mechanism cost: because the
   guard is global, "bump the version on requirements, resolutions, and rules" is a
   guard redesign (per-family versions), not a flag.

5. **"Reverse lookups are a query over the projection" is false where it matters
   most: a canonical write path.** Raising requirement reviews — the ADR 0007 apply
   step that writes review records — reads `produces` edge rows from the store:
   `state_store/requirement_reviews.rs:132-149` (`rule_ids_for_requirement`), called
   from `state_store/typed_specs.rs:252` and from plan preview
   (`operations/plan/evidence.rs:79-84`). The gap report and prime also read
   canonical state, not the database (`cache/gaps/state_adapter.rs:8-17`,
   `handlers/gaps.rs:7`, `cache/prime.rs:126-147`), and the wiki joins edge rows in
   memory (`wiki/assemble/context.rs:25-47`, `traversal.rs:11-18,94-99`,
   `evidence.rs:101`, `pages/rule.rs:15,29`, `pages/resolution.rs:18,33`). Meanwhile
   `RecordFront` — the in-memory derivation the draft would extend — has zero
   consumers outside `provenance-core` (grep over all crates). The draft's rule 3
   (draft:18-20) and its W4 sentence (draft:67-68) defer exactly the machinery the
   canonical path needs on day one.

6. **"Stop double writes" as step one breaks readers and adoption.** Stopping the
   edge half of the citation and needs/resolves writes while every reader still
   walks edges (finding 5's list plus `cache/health.rs:81-99`,
   `cache/impact.rs:44-60`, `cache/traceability.rs:40-90`, `operations/plan.rs:131`,
   `operations/queries/walk.rs:11-96`) leaves answers frozen at the last edge
   written. Worse, adoption breaks immediately: `adopt_unowned` equality compares the
   declaration's desired relations against edge rows
   (`state_store/typed_specs/adoption/relationships.rs:60-140`; the requirement is in
   `docs/state-format.md:14-15`), so an apply that stops writing edges makes adoption
   refuse its own records. The draft's three steps (draft:69-70) also collapse three
   reconciler decisions — stop emitting edges
   (`typed_specs/relationships.rs:8-37`), stop deleting stale managed rows
   (`relationships.rs:68-145`), and move adoption — into one phrase.

7. **`contradicts` as "a review" collides with ADR 0007, and the open question is
   load-bearing.** A requirement review record is machine-raised by `sdk apply`, its
   identity is the exact statement restatement, it carries `field/before/after`, it
   has no actor and no evidence, and a verification run clears it
   (`docs/adr/0007-requirement-changes-put-evidence-up-for-review.md:26-35`;
   `docs/state-format.md:55-63`; `state_store/requirement_reviews.rs:164-180`). A
   contradicts pair is symmetric — the gap policy dedups it as an unordered pair
   (`cache/gaps/contradiction.rs:17,60-66`) — resolves by a supersedes edge or a
   shared resolution, not a test run (`contradiction.rs:33-58`), and the one live
   row's author and date are unrecoverable (edge rows carry none). None of that fits
   the review record. "Review" is also simply a taken word.

8. **The eraser dies first.** For five of the nine types — refines_into, depends_on,
   contradicts, supersedes, spawns — `edges delete` is the only way to remove a
   mistaken claim (`crates/provenance-cli/src/handlers/edges.rs:52-63`;
   `writers.rs:221-230`), and `requirements source-ref` has add but no remove
   (`cli/knowledge.rs:173-186`). The draft replaces the storage of all five and
   schedules no clear or remove path for any of them (draft:64-66 mentions creation
   only). Deleting the shard deletes the product's only relation eraser before a
   replacement exists.

9. **The wire, pinned graph, and export all carry the edge family.** `EdgeType` is
   named in 49 Rust files (274 references); `Neighbor.edge_type` is on every
   neighbors/trace response (`crates/provenance-core/src/protocol/node.rs:107-114`;
   `packages/provenance/src/protocol.ts:246-249`); `edge_types` filters are request
   fields (`protocol/query.rs:78,97`; `protocol.ts:256`); the pinned graph export
   holds `edges` as a fixed family under `deny_unknown_fields`
   (`graph_reference/projection.rs:31-48,120`); export and import read and write it
   (`handlers/export.rs:27`, `handlers/import/scope_writer.rs:52,111-126`). Retiring
   the family is an SDK protocol bump (`protocol.rs:25`, currently 5) plus a
   graph-reference schema change, and every scope's projection digest moves
   (`docs/cache.md`; `graph_reference.rs:85,337`). The draft's Costs (draft:61-63)
   names "a protocol change" and stops there.

10. **The typed declaration surface cannot express five of the nine, and resolutions
    not at all.** `TypedSpecInput` declares sources, requirements, and rules only
    (`crates/provenance-core/src/protocol/typed_spec.rs:20-32`); there is no
    resolution declaration, so `resolves` and `spawns` have no declaration home.
    `TypedRequirementInput` has no slot for `refines`, `supersedes`, or `spawned_by`
    (`typed_spec.rs:66-75`), and `deny_unknown_fields` (`typed_spec.rs:19`) makes
    every addition a wire break for old engines. `TypedRuleInput.requirement(s)` is
    optional (`typed_spec.rs:85-88`), which contradicts the draft's "required
    requirement field on the rule". The apply path itself moves: today it writes
    edges (`typed_specs/relationships.rs:8-37`) and stores no producer on the rule
    (`typed_specs/reconcile.rs:250-274`).

11. **Merge checking loses its only typed reference check, and two rule records go
    stale.** `ShardFamily` recognizes Edges, Requirements, Rules, and landings only
    (`merge/validation.rs:34-47`); sources, resolutions, boundaries, topics, and
    questions merge unchecked. Deleting the shard deletes `validate_merged_edges`
    (`validation.rs:102-113,182-194`) — and the endpoint table behind it — so the new
    fields must land on checked families, and resolutions gain `requirement_ids`
    unchecked unless `ShardFamily` grows. Separately, the rule record
    `rule_prov_edge_endpoint_table` (severity critical, 4 requirement producers,
    `.provenance/state/scopes/default/rules/rule.jsonl:63`) is anchored by `#[rule]`
    on the function being deleted (`edge_validation.rs:20`), so coverage with
    `--validate-rules` reports a critical unimplemented rule unless the record is
    retired in the same change (`docs/shaping.md:280-289`);
    `rule_prov_relation_vocabulary_closed`'s description becomes false text
    (`rule.jsonl:64`).

12. **Requiredness is a gap-policy decision the draft skipped, and its own example
    contradicts the gap report.** Rule 1 says a requirement's citations are
    "optional and may be empty" (draft:11-13) while `MissingSourceRefs` flags
    requirements with none (`cache/gaps/frontier.rs:19-26`; `graph_query.rs:255-284`;
    6 live). The same split exists for `MissingDomainId` (67/68 set),
    `OrphanRule` (`RuleProducer::REQUIRED`, `graph_query.rs:20`), and
    `OrphanResolution` (`frontier.rs:50-61`). "The record's type says whether the
    field is required or optional" (draft:10-11) is therefore not a restatement of
    today — it is a second, unnamed decision about which enforced absences move from
    the gap report into the type, with backfill-order consequences (a serde-required
    field cannot be added before its backfill).

## Answers

**1. The ownership rule, row by row.**

- *references → requirement, existing citation list*: correct. The clause is only
  there, the field side is the superset (79 vs 76; 3 field-only pairs, all
  `src_annotation_format_spec`, 0 edge-only). One precision fix: the draft's
  parenthetical "requirement to source" (draft:32) inverts the stored row — the edge
  runs source→requirement (`edge_validation.rs:25`) while the field lives on the
  requirement. The duality is also a direction inversion; say so, because
  `same_fact_as` and `drop_duality_echoes` (`relations.rs:215-239`,
  `front.rs:94-118`) exist to paper over exactly that.
- *refines_into → child requirement*: right owner; the child is the line being
  authored. Singular is safe (19 rows, 19 children, no child with two parents).
  "Required parent field, absent only on a root" (draft:33) is an optional field
  wearing a required label: 49 of 68 requirements are roots today, so absence is the
  majority case, and a "root" marker has no word in the product. Make it optional,
  singular, at-most-one.
- *depends_on → dependent requirement*: defensible and empty (0 rows). Do not let a
  0-row kind drive wire surface; decide when an authoring act exists.
- *supersedes → older record*: wrong. See finding 2. The successor owns it; the
  proposal case (an immutable record whose replacement is a verdict) already routes
  through the disposition by ADR 0001 and is untouched.
- *needs → dropped*: right call, wrong proof. It does not mirror `resolves`: 2 of 97
  pairs have no `resolves` twin (both from
  `req_rust_requirements_as_code_authoring`). Drop it, but report the 2 pairs and
  record their disposal.
- *resolves → resolution*: right owner (authored by `resolutions create
  --requirement-id`), wrong cardinality (one resolution resolves 3 requirements
  today). List.
- *spawns → resolution*: wrong. See finding 3. The spawned requirement owns
  `spawned_by`.
- *produces → rule*: right owner (`rules create` authors it), wrong cardinality
  twice (finding 1). Lists. Note the ordering cost: `rules create` already requires
  the resolution to exist when `--resolution-id` is given
  (`rule_writers.rs:118-125`), but a resolution producer added after the rule exists
  needs an update path that does not exist today (`cli/policy.rs:56+` has only
  Create) — under the draft that becomes a rule-field edit, which is the honest
  equivalent of today's raw `edges create` and should be said.
- *contradicts → the review*: wrong word, undecided shape. See finding 7 and answer
  2.

A root requirement is a requirement with no `refines`. Nothing breaks today (49 of
68 already are), and the wiki's parent/sibling/lineage joins
(`wiki/assemble/traversal.rs:11-99,102-124`) read the same fact either way.

**2. Contradicts.** The review record does not fit (finding 7). The two shapes that
do, in product words: (a) a question — the frontier already lists open questions
beside contradiction pairs (`docs/shaping.md:70-72`), a question already carries
status and is settled by a resolution, and the gap policy already treats a shared
resolution as settling the pair (`contradiction.rs:49-57`). Costs the draft must
price if it takes this path: a question requires `topic_id` and `requirement_id`,
so it needs a second requirement pointer and a topic for the backfilled row, and
"settled because one side superseded the other" (`contradiction.rs:33-47`) must be
derived as an answer, since a question's `resolution_id` cannot express it; also
`is_resolved` never checks the resolution's status, so a rejected resolution counts
as settling today — that bug becomes a question-shape decision. (b) Keep the one
symmetric row as its own record kind named after the relation — a contradicts
record — with actor and evidence fields, cleared by pointing at the resolution that
settled it. Both are defensible; "review" is not, and leaving the Open question open
(draft:74) blocks the migration step that touches the one live row.

**3. What breaks, concretely.**

- *SDK wire*: `EdgeType` (49 files), `Neighbor.edge_type`, `edge_types` filters,
  `SDK_PROTOCOL_VERSION` 5→6 (`protocol.rs:25`; `packages/provenance/src/engine.ts:35-37`
  pins it). Finding 9.
- *Typed declarations and apply*: `TypedSpecInput` gains fields under
  `deny_unknown_fields` (wire break); needs a resolution declaration kind and
  requirement fields for `refines`/`supersedes`/`spawned_by`; `desired_rule` must
  write `requirement_ids` into the rule record (`typed_specs/reconcile.rs:250-274`);
  the edge emit/delete halves of `relationships.rs:8-145` move to field comparison;
  adoption equality moves to fields (`adoption/relationships.rs:60-140`). Finding 10.
- *Merge validator*: `ShardFamily` loses Edges and its only typed reference check;
  should gain Resolutions at minimum; the endpoint table and its exhaustion tests
  (`edge_validation.rs:89-176`) retire. Finding 11.
- *Gap policy*: today's edge-fed gaps — `MissingSourceRefs`, `NoResolvingDecision`,
  `NoProducedRules` (`frontier.rs:7-48`), `OrphanResolution` (`frontier.rs:50-61`),
  `OrphanRule` (`frontier.rs:65-79`), `UnreferencedSource` (`frontier.rs:81-92`),
  `UnresolvedContradictsPair` (`contradiction.rs:8-31`), dangling edge endpoints
  (`dangling.rs:159-212`), and the retired-resolution derivation over `resolves`
  edges (`state_adapter.rs:49-66`) — all become field reads. Nothing is lost if the
  flip is ordered, everything is lost if it is not.
- *Wiki*: parent, sibling, lineage, evidence, rule attribution, resolution and spawn
  pages all join edge rows today (`wiki/assemble/traversal.rs`, `evidence.rs:101`,
  `pages/rule.rs:15,29,54`, `pages/requirement.rs:50`, `pages/resolution.rs:18,33`).
- *Trace/impact*: `provenance impact`, traceability, prime, and the SDK
  neighbors/trace/impact walk edge rows only, in both directions
  (`cache/impact.rs:44-60`, `cache/traceability.rs:40-90`, `cache/prime.rs:126-147`,
  `operations/queries/walk.rs:11-96`, `operations/queries/impact.rs:27-58`) — today
  they are blind to field-derived relations (a boundary or topic never appears in a
  walk); after the move they must read fields, which is a capability fix and the
  biggest single rewrite in the change.
- *W4 vocabulary*: `RelationKind` keeps the read layer; nine `EdgeRow` derivations
  become fields; `Needs`, `References`, and (under amendment 7a) `Contradicts`
  variants go; `same_fact_as` and `drop_duality_echoes` are deleted. The post-move
  vocabulary is never enumerated in the draft — the exact place the "declaration is
  itself a list" risk lands.
- *Scanner and coverage*: nothing reads edges; the exposure is the two rule records
  (finding 11).
- *W3/W5*: the projection's `edges` table and its four indexes
  (`migrations/001:16-17`, `002:22-33`, `005:1-2`) are loaded from `list_edges()`
  (`cache/materialize/graph_records.rs:182-190`) and the global hash unit includes
  edge shards (`docs/cache.md`). The reverse-lookup materialization needs a new
  derived table (and `source_refs` has no column today, `graph_records.rs:46-52`, so
  the served front is blind to the 3 unmirrored pairs until it exists). The catch-up
  resolution already books a derivation version for reader-logic changes without
  byte changes (`res_catch_up_hashes_scopes_no_journal`); this migration is its
  first customer and the draft's "lands in W3" (draft:67-68) does not name it.

**4. Migration.**

Losslessness per type, from the probe: references (field side superset, lossless);
refines_into (19→19 singular fields, lossless); produces (323 rows → lists,
lossless); resolves (95 → lists, lossless); needs (95 mirrored + 2 needs-only that
must be reported and disposed); spawns (1, lossless onto the child); depends_on and
supersedes (0 rows — but requirement supersession gains its first home, and the
field's direction flips to the successor); contradicts (1 row → a record whose
actor, date, and evidence are unrecoverable and must be recorded as unknown, plus a
topic if it becomes a question).

- *Newer binary on a 0.1 repository*: reads fine (unknown fields ignored,
  `readers.rs` Fields::Open) until step 3 deletes the shard; until then the edge
  readers keep working. Import of a v1 export must map `edges` to fields or refuse
  it explicitly (`handlers/import/scope_writer.rs:111-126`).
- *Older binary on a migrated repository*: the destructive case (finding 4). A 0.1
  binary strips the new fields from any requirement, resolution, or rule shard it
  rewrites, and its writers keep writing edge rows the new readers ignore — so a
  citation it creates exists only as an edge and vanishes from field-fed gaps. The
  version guard stops this only if it fires, which today it does not.
- *Schema version*: required. The guard is global
  (`SUPPORTED_SCHEMA_VERSION`), so either add a per-family version check or bump the
  global version with a one-shot rewriter; both are code, not config. The draft must
  choose and say so.
- *Can each step merge alone?* The reordered steps can: fields+backfill+write-both
  merges against branches still writing edges (readers see both, as
  `cache/health.rs:81-99` and `graph_query.rs:255-284` already union them);
  flipping readers under a parity harness is the one step that must be atomic in
  review, because it changes every answer's inputs at once; stopping edge writes is
  safe once readers ignore them; deleting the shard is cleanup. In the draft's
  order, step 1 freezes every edge-fed answer (finding 5's list) and breaks
  adoption (`adoption/relationships.rs`), so no.

**5. Reverse lookups as materialization.** Yes — canonical state must answer reverse
questions without the projection, and one canonical *write* path already does:
review raising reads which rules a requirement produces
(`requirement_reviews.rs:132-149` via `typed_specs.rs:252`), as does plan preview
(`operations/plan/evidence.rs:79-84`). The gap report, prime, and the wiki compute
from records under the publication lock (`state_adapter.rs:8-17`, `prime.rs:126-147`,
`wiki/assemble.rs:72`), and `provenance check` needs per-field existence checks on
canonical state (`handlers/check/edges.rs:13-44` becomes per-field). SQLite
materialization is therefore not enough: it serves the SDK query surface, while the
canonical path needs the same joins from fields in memory — which is what
`RecordFront` exists for and what nobody consumes yet. Restate rule 3 (amendment 9).

**6. The declaration.** The repo already has the closest thing: the `RelationKind`
const table with derivation, endpoints, and duality, anchored by a rule
(`crates/provenance-core/src/model/relations.rs:26-266`). A macro and a schema file
are both new concepts; a derive attribute in `provenance-macros` (which already
hosts `#[rule]`) tying a struct field to its relation entry, with a struct-walking
completeness test as the fallback, is the extension that fits. Where the "a
declaration is itself a list" risk lands here, concretely — five hand mirrors per
reference field: the struct field (serde), the `RelationKind` entry, the
`RecordFront` match arms (`front.rs:170-323`), the SQL INSERT column lists
(`graph_records.rs`, already the file that forgot `source_refs`), and the TS unions
(`protocol.ts:186-195`) — plus the docs derivation table. The draft's Costs do not
count them; the amendment names the mechanism so the mirrors can be generated or
guarded instead of remembered.

**7. SaaS.** The rule holds, with one reclassification. Citations: a child table
keyed by the owner (`requirement_source_refs`); two writers citing one source from
two requirements never contend, two writers editing one requirement's citations
contend on that requirement — the same contention the JSONL merge has today, so no
regression. Dependencies: the dependent owns the row; peers never contend unless
they edit the same owner. Contradictions: the ownership rule survives only because
contradicts is reclassified as a record of its own (a question row or a contradicts
row with a unique unordered-pair constraint) — that is a relation-shaped table, but
it is a record family with an actor and a lifecycle, not the edge shard back.
After-the-fact assertions (a resolution producer added to an existing rule) force an
update to the owner's row — legitimate, but it needs an update act per owner kind,
and none exists today for rules or resolutions (`cli/policy.rs:6-51,56+`). No peer
relation forces the global edge table back. The one thing fields cannot hold is a
cross-scope reference; none exists (all 612 rows are scope `default`, and writers
check endpoints within one scope, `writers.rs:215-216`).

**8. Do not water down under pushback.** (a) No canonical edge and no "keep the
edges shard for compatibility" — the shard is in 45 of 71 state commits, all of them
also touching a record shard; it is a hotspot, not an asset. (b) The act-relations
stay their own record families with `declared_by` and `retired` — the edges-first
artifact wanted to fold them into an edge envelope; refuse that. (c) One owner per
fact, including retiring `needs` rather than keeping it as a second copy of
`resolves`. (d) The citation clause stays on the requirement, and the 3 unmirrored
pairs get fixed by the migration rather than preserved as a second storage form.
(e) The version guard (amendment 8) is not negotiable for 0.1-interop convenience;
stripping is the failure mode it exists to prevent. (f) The measured drift — 79/76
citations, 97/95 needs/resolves, 0 labels in 612 rows — stays in the position as the
evidence; it is what separates this from taste.

## Prior review (Fable 5.1)

**Agree**, verified independently: findings 1-8 and 10-12 in substance — singular
fields lose live rows (same probe numbers: 14/4/3), the strip-on-write path and
shipped versions, the `supersedes` owner and the missing requirement field, the
`spawns` owner, the vacuity of "required parent, absent only on a root" (49/68
roots), `needs` not being a mirror (same 2 pairs), the ADR 0007 collision, the
canonical readers list, `RecordFront`'s zero production consumers, the wire and
export surfaces, prime missing the 3 field-only citations today, and the
declaration-mechanism choice. Their amendment list A1, A3, A4, A5, A7, A8, A9, A10
is right. Their migration order (add fields + backfill + write both, flip readers
under parity, stop writes, delete shard) is right and my amendment 10 adopts it.

**Disagree:**

- *A2's "superseded_by readable one release"* re-creates, deliberately, the exact
  duality this migration exists to kill — for a field with 1 live value in 93
  resolutions and 0 in 52 sources. Flip it at the backfill step in the same commit;
  a one-release dual form buys no interop (old binaries strip the new field
  regardless) and costs a second union in every supersession reader.
- *A6's contradicts-as-question mechanism* is not implementable as written. "Its
  `links` name both requirements" loses the unordered-pair identity the gap policy
  depends on (`contradiction.rs:17,60-66` — links are freeform artifact links, and
  two links cannot express "these two sides are one pair"); a question also requires
  `topic_id`, which the backfilled row does not have; and the supersedes-settle path
  (`contradiction.rs:33-47`) has no home in a question's `resolution_id`. The word
  is right; the shape needs the second requirement pointer and the derived settle
  rule, and their own answer 2 concedes the cost the amendment omits.
- *Their finding 12 (drop `depends_on` or move it to questions)*: moving relation
  kinds into the migration bundles a product-behavior change (blocking is a
  shaping-loop concept with no storage today, `shaping.md:63`) into a storage
  change. Keep the row owned by the dependent requirement with 0 rows, or drop it
  — but decide it separately from the migration.
- *Citation hygiene*: several of their file:line anchors do not exist in this
  checkout (e.g. `rule_writers.rs:347,376,439-446` — the file is 175 lines here;
  `gaps/graph_query.rs:331-485` — 285 lines). The substance checks out; the anchors
  are from another revision, which matters in a review whose whole currency is
  file:line evidence.

**What it missed:**

- *The eraser problem*: `edges delete` is the only removal path for five relation
  types and `source-ref` has no remove; the migration deletes the product's only
  relation eraser before replacements exist (finding 8, amendment 11).
- *The two relation rule records*: `rule_prov_edge_endpoint_table` is a critical,
  active canonical rule anchored on the function being deleted, with 4 requirement
  producers; retiring it is a governance act in the same change, and
  `rule_prov_relation_vocabulary_closed`'s description becomes false (finding 11).
  Their "scanner and coverage: clean" line is true for edges and misses this.
- *The version guard is global*: "bump schema_version on requirements, resolutions,
  and rules" (their A7) is a per-family guard redesign or a full-repo version
  migration, because `SUPPORTED_SCHEMA_VERSION` is one constant checked for every
  family (`readers.rs:86-100`). Finding 4.
- *Adoption sequencing*: ADR 0008 equality compares edges
  (`adoption/relationships.rs:60-140`), so "stop double writes" as any early step
  breaks `adopt_unowned` before the readers flip. Their finding 8 lists the readers
  but not this writer-side dependency; their answer 3 mentions the move without
  sequencing it.
- *The draft's internal contradiction on requiredness*: "optional and may be empty"
  for citations versus the `MissingSourceRefs` gap (finding 12). Neither review
  should let rule 1's example stand as written.
- *The derivation version booking*: the catch-up resolution explicitly defers a
  derivation version for reader-logic changes without byte changes; this migration
  changes reader logic everywhere and the draft's W3 sentence should name it
  (finding 12's last item).

## Checked, clean

- Draft's measured claims reproduce at `bf5f9c8`: 612 edge rows, 0 labels; 79
  `source_refs` vs 76 `references` edges with 3 field-only pairs (all
  `src_annotation_format_spec`) and 0 edge-only; 97 `needs` vs 95 `resolves` with 2
  needs-only pairs (both from `req_rust_requirements_as_code_authoring`); 19
  `refines_into` rows, 19 distinct children, one parent each; 1 `spawns`, 1
  `contradicts`, 0 `supersedes`, 0 `depends_on`; 45 state commits touch the edges
  shard (now out of 71 — the draft's "of 67" is stale, not wrong).
- All 612 edges are scope `default` with no dangling endpoints.
- Bindings, requirement reviews, dispositions, assertions, thread parents, and
  ideation targets are already owner-side or own-family records with identity and
  lifecycle; the migration does not touch them.
- Scanner and coverage read no edge rows anywhere.
- `write_resolution` writes needs and resolves in one act (`rule_writers.rs:68-85`);
  the reconciler's stale-row deletion is references/produces only
  (`relationships.rs:110-140`).
- Baseline: `cargo test -p provenance-store` green (280 tests) before any probe; no
  probe was committed.
- The owner's two constraints on the thread (relations recomputable from records;
  act-relations stay first-class) are consistent with the direction; the draft
  honors the first and is silent on what the second implies for `contradicts`.
