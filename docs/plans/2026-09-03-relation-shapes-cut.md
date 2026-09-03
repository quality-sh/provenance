# Relation shapes cut: implementation plan

Bead `provenance-1wh.1`. Decision: `res_relations_are_fields_or_action_records` (binding).
Read at `ce891fe`; every file:line below was counted at that commit. Owner rulings: one cut,
no older-binary compatibility, product words only, `depends_on` stays as an optional list on
the dependent requirement (ruled 2026-09-03).

## A. Scope and non-goals

The cut removes the canonical edge. Each of the nine edge types becomes a field on the record
that makes the claim, or a question. One declaration per record kind names its reference
fields; the writers, the validator, the gap policy, the check, the merge gate, and the
projection derive from it. Reverse lookups are derived: in memory from record fields for
canonical readers, from a derived `relations` table in `provenance.db` for served reads.
Existing state is converted once by a throwaway subcommand. The schema version and the SDK
protocol version move. The edges shard, the edge commands, the endpoint table, the same-fact
dedupe, and `EdgeType` are deleted in the same PR.

Non-goals: serving neighbors, trace, or impact from SQLite (W3, bead 1wh.2); freshness
policy knobs and rollout order (W5, bead 1wh.3); any SaaS storage shape; the rendered
graph-change view; restating any requirement other than the anchor.

## B. The declaration mechanism

Spiked at the scratchpad (`spike/relmacro`): a `#[derive(Relations)]` on the record struct
with a `#[relation(target = Kind, required, name = "...", via = field)]` attribute per
reference field. `syn` 2 reads the field type (`Option<StableId>` is single, `Vec<StableId>`
is a list, `Vec<SourceReference>` with `via = source_id` is a list through a struct). The
derive emits, per struct: a `const RELATIONS: [RelationDecl; N]` (owner kind, name, target
kind, list, required) and an `impl RelationOwner` with `fn references(&self) ->
Vec<(&'static str, &StableId)>`. From those two, one generic function each gives: the
reverse scan (filter records whose `references()` names the id), the derived-table rows
(owner id, relation, target kind, target id), the existence check for `check`, the dangling
gap, and the merge gate's field check. The spike ran and printed all of these.

What the derive cannot do:
- See another struct. The whole vocabulary is a hand-written concatenation
  `declared_relations() = [&Source::RELATIONS, &Requirement::RELATIONS, ...]` (5 kinds). A
  test asserts each `RelationOwner` kind appears exactly once.
- Enforce a required list at deserialize time. `serde` builds an empty `Vec`. Requiredness is
  read from the table and enforced in the writer, in `check`, and in the merge gate.
- Catch a reference-typed field that carries no attribute. A serde-walking completeness test
  stays: serialize a full fixture of each kind and assert every key ending in `_id`/`_ids`,
  plus `refines`, `supersedes`, `depends_on`, `spawned_by`, `contradicts`, is declared or on
  the allow list (`scope_id`, `origin_thread`, `origin_message`, `declared_by`, `thread_id`).
- Write the TypeScript types, the JSON schema artifact, or the docs table.

The const-table alternative keeps five hand mirrors per field (struct field, table row,
`RecordFront` arm, SQL column list, check arm); `graph_records.rs:39-54` already forgot
`source_refs`, and both reviews name this as the W2 cost. Cost of the derive: `syn`, `quote`,
`proc-macro2` become dependencies of `provenance-macros` (all three are in `Cargo.lock`
already through `serde_derive`; `Cargo.toml` of the crate has no dependencies today), plus a
`trybuild` suite (already used by `crates/provenance-sdk/tests/compile_fail.rs`).

Recommendation: the derive. `RelationKind` (enum, `relations.rs:49-71`) and
`RelationDerivation` (`relations.rs:33-40`) are deleted. The vocabulary is data: `RelationDecl`
rows in struct order, kinds in node-rank order. `declared_relations()` (`relations.rs:26-29`)
keeps the `#[rule("rule_prov_relation_vocabulary_closed")]` anchor and returns the
concatenation. `RelationSource`, `related_nodes`, `RelationDirection`, `RelationEndpoint`
(`front.rs:17-92`) stay; `RecordFront` (`front.rs:121-131`) loses `edges` and its three
per-kind arm functions (`front.rs:170-323`, `338-372`) become one generic walk over
`RelationOwner`. `drop_duality_echoes` and `same_fact_as` (`front.rs:94-118`,
`relations.rs:215-239`) are deleted. The scanner matches the word `rule` (`parser.rs:184`), so
`#[relation(...)]` is invisible to it.

## C. Record shapes after the cut

Names use `_id`/`_ids` where the field is a plain reference and the product word where the
relation has one. Every list is sorted by id on write and deduplicated.

| Kind | Field | Target | Required | Shape | Replaces |
|---|---|---|---|---|---|
| requirement | `domain_id` | domain | optional | single | unchanged (`artifacts.rs:302-303`) |
| requirement | `source_refs[].source_id` (+`clause`) | source | optional | list | citation stays here; `references` edge dropped |
| requirement | `refines` | requirement | optional | single | `refines_into` edge, inverted onto the child |
| requirement | `depends_on` | requirement | optional | list | `depends_on` edge (0 rows; starts empty) |
| requirement | `supersedes` | requirement | optional | list | `supersedes` edge (0 rows) |
| requirement | `spawned_by` | resolution | optional | single | `spawns` edge, inverted onto the requirement |
| resolution | `requirement_ids` | requirement | required, non-empty | list | `resolves` edge; `needs` dropped |
| resolution | `supersedes` | resolution | optional | list | `superseded_by` field, inverted |
| rule | `requirement_ids` | requirement | required, non-empty | list | `produces` edge from a requirement |
| rule | `resolution_ids` | resolution | optional | list | `produces` edge from a resolution |
| source | `supersedes` | source | optional | list | `superseded_by` field, inverted |
| question | `contradicts` | requirement | optional | single | `contradicts` edge (with `requirement_id` as the other side) |
| question | `topic_id`, `requirement_id`, `resolution_id`, `links[]` | as today | as today | as today | unchanged (`shaping.rs:142-169`) |
| topic, boundary | `requirement_id`, `links[]`, `source_ref` | as today | as today | as today | unchanged |

Lists were forced by the data: 14 rules have two or more requirement producers, 4 have two
or more resolution producers, `res_dispositions_sole_authority` resolves 3 requirements.
Singles were allowed by it: 19 `refines_into` rows, 19 distinct children; 1 `spawns` row.

`superseded_by` today (Source `artifacts.rs:255-260`, Resolution `artifacts.rs:381-386`) sits
on the older record and is set by `sources create --superseded-by` and `resolutions create
--superseded-by` (`knowledge.rs:28-29`, `policy.rs:42-43`), that is, on the record being
created. Its direction inverts: the newer record holds `supersedes: [older]`, and
`superseded_by` is deleted from both structs, both inputs (`inputs.rs:23,116`), both create
commands, `graph_records.rs:28-32,146-153`, `dangling.rs:38-72`, and the wiki pages
(`pages/source.rs:26-29`, `pages/resolution.rs:47-53`, which now derive it by reverse scan).
`CreateProposalCardInput.superseded_by` (`inputs.rs:317`) is a proposal field and is untouched.

A root requirement is a requirement whose `refines` is absent. There is no root marker.
49 of 68 requirements are roots today.

A contradiction is a question: `topic_id` and `requirement_id` name one side, `contradicts`
names the other. It is settled when its `resolution_id` is set, or when either requirement
lists the other in `supersedes`. The unordered pair (`requirement_id`, `contradicts`) is the
identity the gap policy dedupes on.

## D. Commands and SDK calls that change

Owner flag names the owner kind; the target flag is `--target-id` (an existing product word,
`contributions create --target-id`). `add` and `set` refuse a target that does not exist in
the scope. `clear` on a list names the entry; `clear` on a single names nothing. `clear` on a
required list refuses to remove the last entry.

Added (CLI, each backed by one `StateStore` method of the same name):
- `requirements refines set|clear --requirement-id`, `requirements depends-on add|clear`,
  `requirements supersedes add|clear`, `requirements spawned-by set|clear`.
- `requirements source-ref clear --requirement-id --source-id` beside the existing `add`
  (`knowledge.rs:172-188`, add only today); this subsumes "source-ref remove".
- `rules requirement add|clear --rule-id`, `rules resolution add|clear --rule-id`.
- `resolutions requirement add|clear --resolution-id`, `resolutions supersedes add|clear`.
- `sources supersedes add|clear --source-id`.
- `questions contradicts set|clear --id`.
- `requirements create` gains `--refines`, `--spawned-by`, repeatable `--depends-on` and
  `--supersedes`. `questions create` gains `--contradicts`.
- `provenance convert-edges` (section G), dev build only, deleted after the state lands.

Removed: `edges create|list|delete` (`cli/graph.rs:5-43`, `handlers/edges.rs`, 57 lines,
`cli.rs:123-126`, `handlers/mod.rs:15,117-118`); `--superseded-by` on `sources create` and
`resolutions create`; `check`'s edge pass (`handlers/check/edges.rs`, 45 lines, called at
`check.rs:145`).

Changed:
- `rules create --requirement-id` becomes required and repeatable; `--resolution-id`
  repeatable (`policy.rs:75-80`, `handlers/rules.rs:96-97`, `CreateRuleInput` `inputs.rs:126-127`
  become `Vec<StableId>`). `write_rule` (`rule_writers.rs:93-174`) writes the lists on the
  rule and no edge.
- `resolutions create --requirement-id` becomes required and repeatable
  (`policy.rs:16-17`, `handlers/resolutions.rs:40`, `inputs.rs:105`). `write_resolution`
  (`rule_writers.rs:10-87`) writes `requirement_ids` and no `needs`/`resolves` edge.
- `sources create --supersedes` and `resolutions create --supersedes`, repeatable, checked
  for existence (today `create_source` `writers.rs:11-55` checks nothing).
- `requirements source-ref add` returns the requirement, not an `Edge`
  (`writers.rs:131-187`, `handlers/requirements.rs:52-60`).
- `questions create --contradicts`: writes the field; the topic and the requirement are as today.
- `sdk apply`: `desired_rule` (`reconcile.rs:250-274`) writes `requirement_ids` from
  `TypedRuleInput.requirement(s)`; the edge write (`typed_specs/relationships.rs:8-37`) and
  stale-edge delete (`68-145`) become a field reconcile on the rule and the requirement;
  adoption equality (`adoption/relationships.rs`, 213 lines) compares `source_refs` and
  `requirement_ids` fields; `CurrentTypedState.edges` (`typed_specs.rs:33,284-287`) goes.
- `sdk query neighbors|trace`: section F.

## E. The derived relation table

SQLite, migration `021_relations_table.sql`: drop `edges` and its four indexes
(`002:22-33`, `005:1-2`); create

```
relations(scope_id, owner_type, owner_id, relation, target_type, target_id,
          PRIMARY KEY (scope_id, owner_type, owner_id, relation, target_id))
idx_relations_out (scope_id, owner_type, owner_id, relation)
idx_relations_in  (scope_id, target_type, target_id, relation)
```

`ProjectionFamily::Edges` (`projection_families.rs:30,53,77,93-96,105-106,140-142`) is
deleted; `ALL` becomes 18 rows; `is_scoped` goes. `relations` is not a family and has no
digest row: every row derives from a family that already has one. The loader is one generic
function over `RelationOwner` (section B), run per scope after `load_scope` for every owner
kind with reference fields. Catch-up: `rederive_scope` (`catch_up.rs:205-225`) deletes and
reloads the scope's `relations` rows for each owner kind whose family digest moved;
`remove_departed_scopes` (`148-177`) deletes the scope's rows; the `Unit::Global` arm of
`apply_unit_change` (`181-201`) only updates the unit digest row, since no family derives from
the manifest or the dictionary (`docs/cache.md` table); `Unit::Global` stays.
`materialize.rs:70`, `family_rows.rs:43-45,87`, and `projection_digest.rs:17-18` lose their
edge branches. The guard test `scope_locality_guard.rs:132-147` becomes "relations rows derive
from the scope's directory only".

Readers that move (edge rows to in-memory field scans unless noted):
- gaps `graph_query.rs` (285 lines): `edge_exists` (70-85), `resolution_resolves_any_requirement`
  (146-156), `missing_rule_producers` (236-244), `RuleProducer` (8-32), `GapGraph.edges` (42)
  deleted; `resolving_resolutions` (130-144) reads `resolution.requirement_ids`; the four
  produced/producing joins (158-232) read the rule lists; `requirement_has_valid_source`
  (255-267) and `source_is_referenced` (269-284) read `source_refs` only.
- gaps `frontier.rs` (135): the seven kinds at 7-48, 81-135 keep their text. `OrphanResolution`
  (50-61) and `OrphanRule` (65-79) are deleted: the type requires the list, and `check`
  refuses an empty one from a hand edit. A question with `contradicts` set is excluded from
  `OpenQuestion`.
- gaps `contradiction.rs` (66): iterates questions with `contradicts`; `is_resolved` (33-58)
  reads `resolution_id` and both `supersedes` lists. The gap kind and text are unchanged.
- gaps `dangling.rs` (225): `add_edge_refs` and `add_edge_endpoint_gap` (159-211) and
  `edge_type_word` (213-225) are deleted; one generic pass over `declared_relations()` reports
  a dangling target per field, with the text `"<relation> points at missing <kind> <id>"`.
  `add_source_refs`/`add_resolution_refs` (38-72) fold into that pass.
- gaps `state_adapter.rs` (165): `GraphRecords.edges` (29, 44, 94-97) goes; retired
  resolutions (49-66) derive from `requirement_ids` all retired.
- `prime.rs` (149): `RequirementGraphView.edges` (12) becomes `relations: Vec<RelationRow>`
  (owner kind, owner id, relation, target kind, target id) from the in-memory walk;
  `get_requirement_graph_locked` (116-149) sources come from `source_refs`.
- `impact.rs` (126): walks `related_nodes` over `RecordFront`; `follow_indirect` (48-58)
  excludes `refines`, `depends_on`, `contradicts`, `supersedes`, `spawned_by` by name.
- `traceability.rs` (115): `TraceabilityView.edges` (17) becomes `relations` rows for the
  rule's `requirement_ids`, `resolution_ids`, the resolutions' `requirement_ids`, and the
  requirements' `source_refs`.
- `health.rs` (272): `graph_evidence_locked` (73, 88-99) reads `source_refs` only;
  `coverage_health_locked` (181-212) reads `source_refs`, `resolution.requirement_ids`,
  `rule.requirement_ids`; `orphan_rules_locked` (247-272) reports `missing: ["source"]` only.
- wiki assemble: `context.rs:25-53` deleted; `traversal.rs:9-22,77-108`, `discovery.rs:333-341`,
  `pages/requirement.rs:44-56` read `refines`; `evidence.rs:87-115` drops the edge branch (its
  `label` was never set, 0 of 614 rows); `pages/resolution.rs:12-41` reads `requirement_ids`
  and scans `spawned_by`; `pages/rule.rs:9-36,47-66` and `pages/source.rs:9-24` read the rule
  lists and `source_refs`; `assemble.rs:67` and `ScopeExport.edges` (`export.rs:27,59-63`) go.
- `operations/queries/walk.rs` (185): `scoped_edges` (11-19), `steps` (30-58), `edge_rank`
  (173-185) go; `neighbors` (64-96) and `trace` (98-147) walk `related_nodes` over
  `RecordFront` built from `records::load`. Order: node rank, id, then declaration order, then
  direction. `queries/impact.rs:27,34-58` walks the same way, out-direction only.
- `operations/plan.rs:131-139` and `requirement_reviews.rs:132-149`
  (`rule_ids_for_requirement`, called at `typed_specs.rs:252`) scan `rule.requirement_ids`.
- adoption equality (ADR 0008): `adoption/relationships.rs:59-148` compares fields.
- merge gate `merge/validation.rs` (328): `ShardFamily::Edges` (36, 62-63, 103) and
  `validate_merged_edges` (182-194) go; `ShardFamily` gains `Sources`, `Resolutions`,
  `Questions`, `Topics`, `Boundaries`, each deserialized as its type, and every recognized
  family runs the required-list check from the declaration table.
- `check`: `check/edges.rs` goes; `check/scope/core.rs` runs one generic existence pass over
  `declared_relations()` against `CheckIndex` and refuses an empty required list.

## F. Wire and formats

- SDK protocol 6 (`protocol.rs:25`, `engine.ts:17,35-38`). `Neighbor.edge_type`
  (`node.rs:107-113`, `protocol.ts:246-250`) becomes `relation: String` (the declaration
  name); `NeighborsQuery.edge_types` and `TraceQuery.edge_types` (`query.rs:78,97`,
  `protocol.ts:256` and the trace request) become `relations: Vec<String>`, an unknown name
  refused. `EdgeType` (`graph.rs:62-99`, `protocol.ts:186-195`, `index.ts:122`) and `Edge`
  (`graph.rs:101-135`) are deleted. `Direction` stays. A record's neighbors are every
  declared relation in both directions; `docs/cli.md:138-146` is rewritten to say so.
- Graph reference v2: `GraphExport` (`projection.rs:29-48`) loses `edges` (47),
  `load_projection` loses `closed_edges` (89), `validate_schema_versions` loses the edge arm
  (120); `GraphCounts.edges` (`graph_reference.rs:85,337`) goes; the JSON schema artifact
  (`schema/artifacts/graph_reference.rs:49,64,231-236`) drops `edges` and `edge`; `grf1_` and
  `git1_` id formats stay. Every scope's graph digest moves; the reference issued before the
  cut cannot verify after it, which the owner accepted.
- Export/import: `ScopeExport.edges` (`export.rs:27,59-63,94-132`) and the edge branch of
  import (`import.rs:39-42,52`, `import/scope_writer.rs:52,111-189`) go. Import of a v1
  export is refused by the schema version guard.
- Typed declarations: `RequirementDeclaration` (`protocol.ts:34-40`) gains optional `refines`,
  `spawned_by`, `depends_on[]`, `supersedes[]`; `RuleDeclaration` (42-52) gains
  `resolution_ids[]` and needs one of `requirement`/`requirements`. `TypedRequirementInput`
  and `TypedRuleInput` (`typed_spec.rs:64-96`) mirror this; the engine refuses a rule with no
  requirement (`typed_specs.rs:333-358`). No resolution declaration exists, so a resolution's
  `requirement_ids` stays CLI-authored. The two TS tests reading `edges-00.jsonl`
  (`bound-spec.test.ts:395-400`, `fluent-spec.test.ts:451-456`) read `rule.jsonl` instead.
- State schema version: `SUPPORTED_SCHEMA_VERSION` (`aggregate_validation.rs:19`) becomes
  `SchemaVersion(2)`. The guard is global (`readers.rs:85-100`), so the conversion rewrites
  every record in every family to 2, nested landing records and the manifest included. The
  five TS literals `schema_version: 1` (`protocol.ts:60`, `spec.ts:326`, `fluent-spec.ts:392`,
  `bound-materialize.ts:166`, `registry.ts:49`) collapse into one `STATE_SCHEMA_VERSION = 2`.

## G. The conversion

`provenance convert-edges --repo . [--dry-run]`, behind `--features dogfood`, holding the
publication lock, one pass, exact per type, idempotent (a second run finds no edges directory
and no `superseded_by` and changes nothing). Counted at `ce891fe`: 614 rows (76 `references`,
19 `refines_into`, 0 `depends_on`, 1 `contradicts`, 0 `supersedes`, 98 `needs`, 96 `resolves`,
1 `spawns`, 323 `produces`); the decision's 612 was measured at `bf5f9c8`, before
`res_relations_are_fields_or_action_records` landed with its own pair. All rows are scope
`default`, 0 carry a label, 0 dangle.

Per type:
1. `references` (source -> requirement): assert the pair is in `requirement.source_refs`; add
   with `clause: None` when absent (0 today). Report the field-only pairs (3 today, all
   `src_annotation_format_spec`) as "field is the complete side".
2. `refines_into` (parent -> child): `child.refines = parent`; refuse if a child would get two.
3. `depends_on`, `supersedes` (requirement): `from.depends_on += to`, `from.supersedes += to`
   (0 rows; the code path is still written and tested on a fixture).
4. `resolves` (resolution -> requirement): `resolution.requirement_ids += requirement`.
5. `needs` (requirement -> resolution): assert the mirrored `resolves` exists; report each
   pair without one and add it to `requirement_ids` anyway (the union). 2 today, both
   `req_rust_requirements_as_code_authoring` -> `res_sdk_engine_from_package_manager`,
   `res_typed_facade_owns_construction`.
6. `spawns` (resolution -> requirement): `requirement.spawned_by = resolution`; refuse two.
7. `produces` (requirement -> rule): `rule.requirement_ids += requirement`; (resolution ->
   rule): `rule.resolution_ids += resolution`.
8. `contradicts` (requirement -> requirement): mint topic
   `topic_contradiction_<from>` (status `explored`, so no `UnexploredTopic` gap), question
   `q_contradiction_<from>_<to>` with `topic_id`, `requirement_id = from`, `contradicts = to`,
   method `grill`, status `open`, text "Requirement <from> contradicts requirement <to>. One of
   them must be restated or superseded.", no `resolution_id` (no shared resolution today:
   `req_state_merges_without_humans` is resolved by `res_state_is_jsonl_in_git`,
   `req_edge_writes_validated` by nothing). Report that the author and date are unknown.
9. `superseded_by` fields: for each `X.superseded_by = Y`, `Y.supersedes += X`, delete the
   field. 1 today: `res_state_is_jsonl_in_git.supersedes = [res_convex_chosen_as_backend_over_custom_web]`.
   Report `res_rule_is_the_function` (status `superseded`, no successor) and leave it.

Then: assert every rule and every resolution has a non-empty `requirement_ids` (true today: 0
rules and 0 resolutions lack one); rewrite `schema_version` to 2 in every family; delete
`.provenance/state/edges/`; run `check`; run `materialize`. Print a report with one line per
type, count in, count written, and each item from steps 1, 5, 8, 9. The subcommand is deleted
in the last commit of the PR after the converted state is committed.

## H. Records to retire or rewrite

- `rule_prov_edge_endpoint_table` (`rule.jsonl`, severity critical, 4 requirement producers):
  status `archived`, `retired: true`; its `#[rule]` anchor (`edge_validation.rs:14`) and six
  `#[verifies]` sites (`edge_validation.rs:97-166`) go with the file. `coverage
  --validate-rules` must not report it.
- `rule_prov_relation_vocabulary_closed`: statement "Each reference field on a canonical
  record carries one relation declaration, and every reverse lookup, validator check, gap,
  and projection row derives from that declaration." The description names the derive, the
  `declared_relations()` anchor, the concatenation test, and the completeness test.
- Anchor `req_implement_a_normalized_knowledge_graph_d` (its statement names "nine typed
  edges checked at write time"): new requirement `req_relations_are_record_fields`,
  `supersedes: [req_implement_a_normalized_knowledge_graph_d]`, same domain and `source_refs`,
  statement drafted with the provenance-grounded-writing skill and the checker: "The graph is
  four canonical record kinds, Source, Requirement, Resolution, and Rule, and a relation
  between records is a reference field on the record that makes the claim or the record of
  the action that asserted it." The old requirement and its topic stay as they are. This is
  the first use of `requirements create --supersedes`.
- `docs/state-format.md` (120 lines): line 7 (supersession fields), 15 (adoption
  relationships), 23 ("historical edges"), 115-120 (graph reference families) rewritten;
  version 2 stated; the reference-field table from section C added.
- `docs/cli.md` (471): line 22 example, 138-146 (neighbors/trace), 336-341 (merge gate),
  384 (exact export families), 462 (edge commands), 466-467 (traceability), and the create
  flags at 471 rewritten; the new commands from section D listed.
- `docs/cache.md`: the catch-up section loses "the edges shard" and "A changed global unit
  reloads the edges table whole"; the derivation table loses the `edges` row and gains a
  `relations (derived, no digest row)` row.
- `docs/shaping.md:63` ("blocking edges") becomes "depends_on"; line 71 ("no source edge")
  becomes "no source reference".

## I. Test strategy

Differential harness (`crates/provenance-cli/tests/relation_cut_parity.rs`, over a fixture
copy of `.provenance/state` at `ce891fe`): before the readers move, snapshot `gaps`, `prime`,
`wiki` (assembled model), `traceability` per rule, `impact` per requirement and source,
`health`, `orphans`, `sdk query neighbors` and `trace` per record, all as JSON. After
conversion, assert byte-identical output after one normalization: edge-type names map to
relation names, and the `edges` arrays of prime, traceability, and neighbors compare as sets
of (owner, relation, target) triples. Expected deltas live in one file and are asserted
exactly: prime's requirement graph for the 3 field-only citations gains
`src_annotation_format_spec`; the contradiction question adds nothing (`open` keeps the pair
gap, the `explored` topic adds no gap). Nothing else may differ.

Relation map: `crates/provenance-cli/tests/relation_map.rs` carries the gist's 39 rows as
data (relation, owner kind, authoring command, clear command or "immutable"). Per row it runs
the authoring command on a fresh repository, reads the record back, asserts the reference,
runs the clear command where one is named, and asserts it is gone. The `X`-class rows use
their existing commands.

Mutation targets, each run as a deliberate break that must turn a test red:
drop one reverse scan (remove one `#[relation]` attribute: completeness test);
skip one owner kind in the derived-table loader (catch-up equivalence test);
make `Rule.requirement_ids` optional (writer, `check`, and merge gate tests);
backfill one edge type wrong (the harness);
delete one row from `declared_relations()` (the concatenation test).

Unchanged: the four catch-up equivalence suites (`cache/tests/catch_up_*.rs`); the edge cases
in `unit_digest_behavior.rs:76-176` and `catch_up_domain_coverage.rs:132-176` become
`relations` cases. `trybuild` for the derive: a bad field type, an unknown key, a `required`
single, a missing `target`. 49 Rust and 2 TS test files mention edges today; each is
rewritten to the field it now means, none deleted without a replacement.

## J. Revised W3 and W5 text

W3, per-operation mapping, item 2 becomes: "`neighbors`, `trace` — walk the derived
`relations` table (`idx_relations_out`, `idx_relations_in`) instead of the in-memory
`RecordFront`. Filters name relations, not edge types (protocol 6, inherited from the
relation shapes cut). Trace gains a resume token as before. Ordering: node rank, canonical
id, declaration order, direction." Item 3 (`impact`): "traversal over the `relations` table,
out direction only; the indirect filter names `refines`, `depends_on`, `contradicts`,
`supersedes`, `spawned_by`." Stamp table rows `neighbors`, `trace`, `impact`: attested fields
unchanged; "edges + nodes" becomes "relations + nodes". The prerequisite paragraph stays.
Protocol flag: "Version is 6 after the relation shapes cut; W3 adds no further bump."
`SqlFront`: "reads `relations` per scope; a global edges family no longer exists."

W5 landing order, item 2: "W2 equivalence suite green; catch-up is the default freshness
step. No journal." Item 3: "Relation shapes cut (bead 1wh.1) merges; the vocabulary is the
declaration table." Delete every journal sentence (plan lines 21, 39, 47-48, 107, 273-440).
Knobs: delete `cache.catchup_journal`; keep `read.freshness_policy`, `read.visit_budget`,
`read.scan_budget`. File-growth gates: drop `publication/journal.rs` and
`materialize/sweep.rs`. q82f handoff item 6: "Protocol version confirmed at 6."

## K. Sequencing inside the one PR

Branch `1wh-relation-shapes-cut`. Each commit builds and passes the workspace suite.
1. Harness "before" snapshots, produced by the `main` binary over the fixture state and
   committed as files. `provenance-macros`: `syn`, `quote`, `proc-macro2`; the `Relations`
   derive; trybuild.
2. Fields on the structs with the derive, `declared_relations()` as the concatenation, the
   completeness test. Old readers still read edges; new fields are empty; version stays 1.
3. Writers and commands (section D) write fields only and stop writing edges; conversion
   subcommand; the relation-map test.
4. Readers move (section E): gaps, prime, impact, traceability, health, wiki, walk, plan,
   reviews, adoption, merge gate, check. Harness green against the converted fixture.
5. Projection: migration 021, the derived-table loader, catch-up changes, family table at 18.
6. Wire: protocol 6, graph reference v2, TypeScript types and tests.
7. `SUPPORTED_SCHEMA_VERSION` to 2 and the TS constant; convert the live state; commit
   `.provenance/state` (edges directory gone, every record at version 2, the topic and
   question from section G step 8, the resolution `supersedes`).
8. Records and docs (section H): the two rules, the new anchor requirement, four docs.
9. Deletions: `EdgeType`, `Edge`, `edge_validation.rs`, `RelationKind`, the edges commands,
   `check/edges.rs`, `read_edge_shards` and `edge_shard_paths` (`readers.rs:357-428`),
   `shards::edges_path`, `layout.edges_dir`, the conversion subcommand.

500-line cap (`AGENTS.md:20`), split by responsibility before growth: the `add`/`clear`
writers go in a new `state_store/reference_writers.rs`, not `writers.rs` (321); the new
subcommands go in a new `cli/references.rs`, not `cli/knowledge.rs` (188); field comparison
lands in `adoption/relationships.rs` (213), so `typed_specs/reconcile.rs` (476) and
`typed_specs/adoption.rs` (483) do not grow; `graph_reference.rs` (422) shrinks. New test
files stay under 300 lines each.

Before ready: a deslop pass (no `edge` word left in production code, comments, or docs except
the conversion report and this plan), the five mutation runs from section I recorded in the
PR body, `cargo clippy --all-targets --all-features`, `cargo test --workspace`, the TS suite,
and `provenance check` clean on the converted state. Then the three adversarial reviews.

## L. Risks and open points

1. Required lists and hand edits. An empty `requirement_ids` deserializes. Default: `check`
   and the merge gate refuse it; `OrphanRule` and `OrphanResolution` gaps are deleted, not
   kept as a second guard. `clear` on the last entry refuses: "a rule needs one requirement".
2. Contradiction topic. A question needs a topic. Default: the conversion mints one
   `explored` topic per contradiction (1 today); `questions create --contradicts` on a live
   repository uses the topic the caller names.
3. Contradiction settled by a rejected resolution. `is_resolved` never checked status
   (`contradiction.rs:49-57`). Default: keep that behavior; leave the fix to W3.
4. `depends_on`: ruled optional list on the dependent requirement, 0 rows, no gap reads it.
   Default: impact treats it as indirect, like today's edge.
5. Neighbors order. Today: edge rank then direction (`walk.rs:149-156,173-185`). Default:
   declaration order (struct field order within the owner kind, kinds in node rank) then
   direction; the harness compares sets, and the W3 cursor freezes the new order.
6. Adoption equality and the fields a declaration cannot name (`refines`, `supersedes`,
   `spawned_by`, `depends_on` set by the CLI). Default: adoption compares only `source_refs`,
   `requirement_ids`, `resolution_ids`; the rest is "richer canonical metadata"
   (`docs/state-format.md:16-17`).
7. Typed declaration keys versus ids. Default: `refines` and `supersedes` take a key of the
   same spec; `spawned_by` takes a resolution id (resolutions have no declaration).
8. Old graph references and v1 exports. Default: a reference issued before the cut does not
   verify; a v1 export is refused by the version guard with a message naming `convert-edges`.
9. The schema version rewrite touches every file under `.provenance/state`. Default: one
   commit, step 7, reviewed by count (records in equals records out per family).
10. The conversion subcommand. Default: `handlers/convert_edges.rs` behind `--features
    dogfood`, its logic a test-visible function the harness calls, deleted in step 9.
11. Reverse scan cost. In-memory scans are O(records) per lookup, as `fk`/`embedded` are
    today (`front.rs:376-446`). Default: one `BTreeMap<(kind, id), Vec<(owner, relation)>>`
    index per `RecordFront`, built on first use; W3 replaces it with SQL for served reads.
12. `source-ref clear` of a citation the typed reconciler manages. Default: allowed; the next
    `sdk apply` restores it, as today's reconciler appends (`reconcile.rs:202-208`).
13. Threads and ideation records reference requirements too (map rows 22-34). Default: they
    stay out of `relations` in this cut and enter after the W3 dangling-target prerequisite;
    the declaration table lists them as `X`-class rows without a loader.
