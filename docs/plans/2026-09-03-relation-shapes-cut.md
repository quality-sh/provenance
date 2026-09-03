# Relation shapes cut: implementation plan (revision 2)

Bead `provenance-1wh.1`. Decision: `res_relations_are_fields_or_action_records` (binding). Read at `ce891fe`; every
file:line below was counted there. Revision 2 answers findings 1-14 of the review at
`docs/reviews/2026-09-03-relation-shapes-cut-plan-fable-review.md` (branch `1wh-cut-plan`). Owner rulings: one cut,
no older-binary compatibility, product words only, `depends_on` stays an optional list on the dependent requirement.

## A. Scope and non-goals

The cut removes the canonical edge. Each of the nine edge types becomes a field on the record that makes the claim,
or a question. One declaration per record kind names its reference fields, their target kind, requiredness, and
flow; the writers, the validator, the gap policy, the check, the merge gate, the traversals, and the projection
derive from it. Reverse lookups are derived: in memory from record fields for canonical readers, from a derived
`relations` table in `provenance.db` for served reads. Existing state is converted once by a throwaway subcommand.
The schema version and the SDK protocol version move. The edges shard, the edge commands, the endpoint table, the
same-fact dedupe, and `EdgeType` go in the same PR.

Non-goals: serving neighbors, trace, or impact from SQLite (W3, bead 1wh.2); freshness policy knobs and rollout
order (W5, bead 1wh.3); any SaaS storage shape; the rendered graph-change view; restating other requirements.

## B. The declaration mechanism

Spiked (scratchpad `spike/relmacro`, reproduced by the reviewer): `#[derive(Relations)]` on the record struct, one
`#[relation(target = Kind, flow = ..., required, name = "...", via = field)]` per reference field. `syn` 2 reads the
field type: bare `StableId` is a required single, `Option<StableId>` an optional single, `Vec<StableId>` a list
(required only with the flag), `Vec<T>`/`Option<T>` with `via = field` a reference through a struct. `flow` is
`target_upstream` (the target is upstream of the owner), `target_downstream`, or `none`. The derive emits per struct
a `const RELATIONS: [RelationDecl; N]` (owner kind, name, target kind, list, required, flow) and `impl
RelationOwner` with `fn references(&self) -> Vec<(&'static str, &StableId)>`. One generic function each then gives
the reverse scan, the derived-table rows, the existence check, the dangling gap, the empty-required-list refusal,
and the directed walk (section E). The derive refuses at compile time a `StableId`-typed field (bare, `Option`, or
`Vec`) with no `#[relation]`: the primary guard.

Seven kinds carry the derive: source, requirement, resolution, rule, topic, question, boundary. The derive cannot:
- See another struct. `declared_relations()` (`relations.rs:26-29`, keeps its `#[rule]` anchor) is a hand-written
  concatenation of the seven tables; a test with a hand list of the seven owner kinds asserts each table is present
  exactly once.
- Enforce a required list at deserialize time: `serde` builds an empty `Vec`. Requiredness is enforced from the
  table by the writer, the aggregate validator, `check`, and the merge gate.
- Declare `links[]` on topic and question: `ArtifactLink` (`shaping.rs:105-110`) carries a per-entry `target_type`.
  Links stay outside the declaration: written through `validate_artifact_links` (`artifact_links.rs:9`), checked by
  `check_artifact_links` (`references.rs:39`), walked by one hand-written contribution to `related_nodes` named
  `links`, which replaces `TopicLinks`/`QuestionLinks`. A serde-walking test over a full fixture guards what the
  compile check cannot see (`ArtifactLink.target_id`, `ThreadParent`).
- Write the TypeScript types, the JSON schema artifact, or the docs table.

The const-table alternative keeps five hand mirrors per field (struct field, table row, `RecordFront` arm, SQL
column list, check arm); `graph_records.rs:39-54` already forgot `source_refs`. Cost of the derive: `syn`, `quote`,
`proc-macro2` in `provenance-macros` (in `Cargo.lock` through `serde_derive`; the crate has no dependencies today)
and a `trybuild` suite (pattern: `crates/provenance-sdk/tests/compile_fail.rs`).

Recommendation: the derive. `RelationKind` (`relations.rs:49-71`), `RelationDerivation` (33-40), `same_fact_as`
(215-239), and `drop_duality_echoes` (`front.rs:94-118`) are deleted. `RelationSource`, `related_nodes`,
`RelationDirection`, `RelationEndpoint` (`front.rs:17-92`) stay; `RecordFront` (121-131) loses `edges`, and its
per-kind arms (170-323, 338-372) become one generic walk over `RelationOwner` plus the `links` function. The scanner
matches the word `rule` (`parser.rs:184`); `#[relation(...)]` is invisible to it.

## C. Record shapes after the cut

Lists are sorted by id on write and deduplicated. `refines`, `depends_on`, and `supersedes` refuse a cycle at write.

| Kind | Field | Target | Required | Shape | Flow | Replaces |
|---|---|---|---|---|---|---|
| requirement | `domain_id` | domain | optional | single | none | unchanged (`artifacts.rs:302-303`) |
| requirement | `source_refs[].source_id` (+`clause`), name `cites` | source | optional | list | target_upstream | `references` edge dropped |
| requirement | `refines` | requirement | optional | single | target_upstream | `refines_into` edge, inverted onto the child |
| requirement | `depends_on` | requirement | optional | list | target_downstream | `depends_on` edge (0 rows) |
| requirement | `supersedes` | requirement | optional | list | target_downstream | `supersedes` edge (0 rows) |
| requirement | `spawned_by` | resolution | optional | single | target_upstream | `spawns` edge, inverted |
| resolution | `requirement_ids` | requirement | required, non-empty | list | target_upstream | `resolves`; `needs` dropped |
| resolution | `supersedes` | resolution | optional | list | target_downstream | `superseded_by`, inverted |
| rule | `requirement_ids` | requirement | required, non-empty | list | target_upstream | `produces` from a requirement |
| rule | `resolution_ids` | resolution | optional | list | target_upstream | `produces` from a resolution |
| source | `supersedes` | source | optional | list | target_downstream | `superseded_by`, inverted |
| topic | `requirement_id` | requirement | required | single | none | unchanged (`shaping.rs:129-130`) |
| boundary | `requirement_id`; `source_ref.source_id` | requirement; source | required; optional | single | none | unchanged (`shaping.rs:117-121`) |
| question | `topic_id`, `requirement_id`; `resolution_id` | topic, requirement; resolution | required; optional | single | none | unchanged (`shaping.rs:146-168`) |
| question | `contradicts` | requirement | optional | single | none | `contradicts` edge, other side in `requirement_id` |

Lists were forced by the data: 14 rules have two or more requirement producers, 4 have two or more resolution
producers, `res_dispositions_sole_authority` resolves 3 requirements. Singles were allowed by it: 19 `refines_into`
rows, 19 distinct children; 1 `spawns` row.

`superseded_by` today (Source `artifacts.rs:255-260`, Resolution 381-386) sits on the older record but is set on the
record being created (`--superseded-by` at `knowledge.rs:28-29`, `policy.rs:42-43`). Its direction inverts: the
newer record holds `supersedes: [older]`; `superseded_by` is deleted from both structs, both inputs
(`inputs.rs:23,116`), both commands, `graph_records.rs:28-32,146-153`, `dangling.rs:38-72`, and the wiki pages
(`pages/source.rs:26-29`, `pages/resolution.rs:47-53`, which derive it by reverse scan).
`CreateProposalCardInput.superseded_by` (`inputs.rs:317`) is a proposal field and is untouched.

A root requirement is a requirement whose `refines` is absent; 49 of 68 are roots today.

A contradiction is a question: `topic_id` and `requirement_id` name one side, `contradicts` the other. It is settled
when `resolution_id` is set or either requirement lists the other in `supersedes`. Today's shared-resolution settle
path (`contradiction.rs:49-57`) is dropped on purpose: the question's `resolution_id` is the record of that
settlement. The unordered pair (`requirement_id`, `contradicts`) is the gap identity.

## D. Commands and SDK calls that change

The owner flag names the owner kind; the target flag is `--target-id` (an existing word, `contributions create
--target-id`). `add` and `set` refuse a missing target and a cycle. `clear` on a list names the entry; on a single
it names nothing; on a required list it refuses the last entry ("a rule needs one requirement").

Added (CLI, each backed by one `StateStore` method of the same name):
- `requirements refines set|clear --requirement-id`, `requirements depends-on add|clear`, `requirements supersedes
  add|clear`, `requirements spawned-by set|clear`, `requirements source-ref clear --requirement-id --source-id`
  beside the existing `add` (`knowledge.rs:172-188`; subsumes "remove").
- `rules requirement add|clear --rule-id`, `rules resolution add|clear --rule-id`, `resolutions requirement
  add|clear --resolution-id`, `resolutions supersedes add|clear`, `sources supersedes add|clear --source-id`,
  `questions contradicts set|clear --id`.
- `requirements create` gains `--refines`, `--spawned-by`, repeatable `--depends-on` and `--supersedes`; `questions
  create` gains `--contradicts`. `provenance convert-edges` (section G), dev build only.

Removed: `edges create|list|delete` (`cli/graph.rs:5-43`, `handlers/edges.rs`, 57 lines, `cli.rs:123-126`,
`handlers/mod.rs:15,117-118`); `--superseded-by` on `sources create` and `resolutions create`; `check`'s edge pass
(`handlers/check/edges.rs`, 45 lines, `check.rs:145`).

Changed:
- `rules create --requirement-id` required and repeatable, `--resolution-id` repeatable (`policy.rs:75-80`,
  `handlers/rules.rs:96-97`, `inputs.rs:126-127` become `Vec<StableId>`); `write_rule` (`rule_writers.rs:93-174`)
  writes the lists, no edge. `resolutions create --requirement-id` required and repeatable (`policy.rs:16-17`,
  `handlers/resolutions.rs:40`, `inputs.rs:105`); `write_resolution` (`rule_writers.rs:10-87`) writes
  `requirement_ids`, no `needs`/`resolves` edge.
- `sources create --supersedes`, `resolutions create --supersedes`, repeatable, existence checked (`create_source`
  `writers.rs:11-55` checks nothing today).
- `requirements source-ref add` returns the requirement, not an `Edge` (`writers.rs:131-187`,
  `handlers/requirements.rs:52-60`).
- `sdk apply`: `desired_rule` (`reconcile.rs:250-274`) writes `requirement_ids` and `resolution_ids`;
  `desired_requirement` (142-174) writes `refines`, `spawned_by`, `depends_on`, `supersedes`. A declaration field is
  authoritative when present and untouched when absent, so a CLI-set `refines` survives a spec that does not name
  one; `source_refs` keeps today's append (`reconcile.rs:202-208`). The edge write
  (`typed_specs/relationships.rs:8-37`) and stale-edge delete (68-145) go; adoption equality
  (`adoption/relationships.rs:59-148`) compares the `source_refs`, `requirement_ids`, `resolution_ids` fields;
  `CurrentTypedState.edges` (`typed_specs.rs:33,284-287`) goes. `reconcile.rs` (476 lines) splits first into
  `reconcile/{sources,requirements,rules,changes}.rs`; the field reconcile lands in `reconcile/references.rs`.
- `sdk query neighbors|trace`: section F.

## E. The derived relation table and the readers

SQLite, migration `021_relations_table.sql`: drop `edges` and its four indexes (`002:22-33`, `005:1-2`); create

```
relations(scope_id, owner_type, owner_id, relation, target_type, target_id,
          PRIMARY KEY (scope_id, owner_type, owner_id, relation, target_id))
idx_relations_out (scope_id, owner_type, owner_id, relation)
idx_relations_in  (scope_id, target_type, target_id, relation)
```

`ProjectionFamily::Edges` (`projection_families.rs:30,53,77,93-96,105-106,140-142`) is deleted; `ALL` becomes 18
rows; `is_scoped` goes. `relations` is not a family and has no digest row: every row derives from one owner record's
fields with no join. The loader is one generic function over `RelationOwner` plus the `links` function, run per
scope after `load_scope` for the seven owner kinds. Catch-up: `rederive_scope` (`catch_up.rs:205-225`) deletes and
reloads the scope's `relations` rows for each owner kind whose family digest moved; `remove_departed_scopes`
(148-177) deletes the scope's rows explicitly; the `Unit::Global` arm of `apply_unit_change` (181-201) only updates
the unit digest row, since no family derives from the manifest or the dictionary (`docs/cache.md` table);
`Unit::Global` stays. `load_edges` (`graph_records.rs:182-196`), `materialize.rs:70`, `family_rows.rs:43-45,87`,
`projection_digest.rs:17-18` lose their edge branches; `scope_locality_guard.rs:132-147` becomes a scope-only
assertion; the `edges` fixture at `projection_digest_sensitivity.rs:29` becomes a record with relation fields.

Directed walks derive from `flow`. Downstream from a record is out over `target_downstream` relations and in over
`target_upstream` ones; upstream is the mirror; `none` relations are never followed by impact or traceability and by
trace and neighbors only under `direction`. Today `queries/impact.rs:34-58` follows `from_id` only and
`cache/impact.rs:81-83` defines downstream as `to_id`; after the cut a source has no out field, so an "out only"
walk answers nothing. A source reaches its requirements through `cites` (in), a requirement its rules through
`requirement_ids` (in) and its resolutions likewise.

Readers that move (edge rows to in-memory field scans unless noted):
- gaps `graph_query.rs` (285 lines): `edge_exists` (70-85), `resolution_resolves_any_requirement` (146-156),
  `missing_rule_producers` (236-244), `RuleProducer` (8-32), `GapGraph.edges` (42) deleted; `resolving_resolutions`
  (130-144) reads `resolution.requirement_ids`; the four produced/producing joins (158-232) read the rule lists;
  `requirement_has_valid_source` (255-267) and `source_is_referenced` (269-284) read `source_refs` only.
- gaps `frontier.rs` (135): the seven kinds at 7-48, 81-135 keep their text. `OrphanResolution` (50-61) and
  `OrphanRule` (65-79) are deleted: the type requires the list, and the validator, `check`, and the merge gate
  refuse an empty one. A question with `contradicts` set is excluded from `OpenQuestion`.
- gaps `contradiction.rs` (66): iterates questions with `contradicts`; `is_resolved` (33-58) reads `resolution_id`
  and both `supersedes` lists. Gap kind and text unchanged.
- gaps `dangling.rs` (225): the edge passes (159-225) deleted; one generic pass over `declared_relations()` reports
  a dangling target per field as `"<relation> points at missing <kind> <id>"`; the source, resolution, topic, and
  question passes (38-139) fold into it; links and thread parents stay hand-written.
- gaps `state_adapter.rs` (165): `GraphRecords.edges` (29, 44, 94-97) goes; retired resolutions (49-66) derive from
  `requirement_ids` all retired.
- `prime.rs` (149): `RequirementGraphView.edges` (12) becomes `relations: Vec<RelationRow>` (owner kind, owner id,
  relation, target kind, target id); `get_requirement_graph_locked` (116-149) reads `source_refs`.
- `impact.rs` (126): the directed walk above; `follow_indirect` (48-58) excludes `refines`, `depends_on`,
  `contradicts`, `supersedes`, `spawned_by` by name.
- `traceability.rs` (115): upstream walk from the rule over `requirement_ids`, `resolution_ids`, the resolutions'
  `requirement_ids`, `cites`; `edges` (17) becomes `relations`.
- `health.rs` (272): `graph_evidence_locked` (73, 88-99) reads `source_refs` only; `coverage_health_locked`
  (181-212) reads the three lists; `orphan_rules_locked` (247-272) reports `missing: ["source"]` only.
- wiki assemble: `context.rs:25-53` deleted; `traversal.rs:9-22,77-108`, `discovery.rs:333-341`,
  `pages/requirement.rs:44-56` read `refines`; `evidence.rs:87-115` drops the edge branch (its `label` was never
  set, 0 of 614 rows); `pages/resolution.rs:12-41` reads `requirement_ids` and scans `spawned_by`;
  `pages/rule.rs:9-36,47-66` and `pages/source.rs:9-24` read the rule lists and `source_refs`; `assemble.rs:67` and
  `ScopeExport.edges` (`export.rs:27,59-63`) go.
- `operations/queries/walk.rs` (185): `scoped_edges` (11-19), `steps` (30-58), `edge_rank` (173-185) go; `neighbors`
  (64-96) and `trace` (98-147) walk `related_nodes` over a `RecordFront` from `records::load`. Order: node rank, id,
  declaration order, direction. `queries/impact.rs:27,34-58` uses the downstream walk.
- `operations/plan.rs:131-139` and `requirement_reviews.rs:132-149` (called at `typed_specs.rs:252`) scan
  `rule.requirement_ids`.
- merge gate `merge/validation.rs` (328): `ShardFamily::Edges` (36, 62-63, 103) and `validate_merged_edges`
  (182-194) go; `ShardFamily` gains `Sources`, `Resolutions`, `Questions`, `Topics`, `Boundaries`, each deserialized
  as its type, and every recognized family runs the required-list check from the table.
- aggregate validator `validate_ideation_scope` (`ideation_batches.rs:126`; run by materialize, direct writes,
  import, and `check` per `docs/cache.md`): refuses an empty required list and a cycle.
- `check`: `check/edges.rs` goes; the question, topic, and boundary key checks at `check/scope/core.rs:274-332`
  become one generic pass over `declared_relations()` against `CheckIndex`; links and origin checks stay.
- store plumbing deleted: `CreateEdgeInput` (`inputs.rs:54-61`), `list_edges`/`closed_edges`
  (`state_store.rs:167-169,244-246`), the edge writers (`writers.rs:189-320`), the edge readers
  (`readers.rs:357-428`), `shards::edges_path`, `layout.edges_dir`.

## F. Wire and formats

- SDK protocol 6 (`protocol.rs:25`, `engine.ts:17,35-38`). `Neighbor.edge_type` (`node.rs:107-113`,
  `protocol.ts:246-250`) becomes `relation: String` (the declaration name; `cites` for the citation, `links` for
  artifact links); `NeighborsQuery.edge_types` and `TraceQuery.edge_types` (`query.rs:78,97`, `protocol.ts:256` and
  the trace request) become `relations: Vec<String>`, an unknown name refused. `EdgeType` (`graph.rs:62-99`,
  `protocol.ts:186-195`, `index.ts:122`) and `Edge` (`graph.rs:101-135`) are deleted. `Direction` stays. Neighbors
  are every declared relation both ways (`docs/cli.md:138-146`).
- Graph reference v2: `GraphExport` (`projection.rs:29-48`) loses `edges` (47), `load_projection` loses
  `closed_edges` (89), `validate_schema_versions` its edge arm (120); `GraphCounts.edges`
  (`graph_reference.rs:85,337`) goes; the JSON schema artifact (`schema/artifacts/graph_reference.rs:49,64,231-236`)
  drops `edges` and `edge`; `grf1_` and `git1_` formats stay. Every scope's graph digest moves; a reference issued
  before the cut does not verify after it (owner accepted).
- Export/import: `ScopeExport.edges` (`export.rs:27,59-63,94-132`) and the edge branch of import
  (`import.rs:39-42,52`, `import/scope_writer.rs:52,111-189`) go. A v1 export is refused by `deny_unknown_fields`
  (`export.rs:8`) on its `edges` key with serde's message; no message names the converter.
- Typed declarations: `RequirementDeclaration` (`protocol.ts:34-40`) gains optional `refines`, `supersedes[]`,
  `depends_on[]` (keys of the same spec) and `spawned_by` (a resolution id); `RuleDeclaration` (42-52) gains
  `resolution_ids[]` and needs one of `requirement`/`requirements`. `TypedRequirementInput` and `TypedRuleInput`
  (`typed_spec.rs:64-96`) mirror this; the engine refuses a rule with no requirement (`typed_specs.rs:333-358`). No
  resolution declaration exists, so a resolution's `requirement_ids` stays CLI-authored. The TS tests reading
  `edges-00.jsonl` (`bound-spec.test.ts:395-400`, `fluent-spec.test.ts:451-456`) read `rule.jsonl` instead.
- State schema version: `SUPPORTED_SCHEMA_VERSION` (`aggregate_validation.rs:19`) becomes `SchemaVersion(2)`. The
  guard is global and exact (`readers.rs:85-100,143-155`; `state_store.rs:100` for the manifest), so the conversion
  rewrites every record in every family, the manifest, and the nested landing records. Production literals become
  the constant: `manifest.rs:36` (the `init` default), `scope.rs:70`, `threads.rs:60`, `handlers/rules.rs:145`; the
  TS literals (`protocol.ts:60`, `spec.ts:326`, `fluent-spec.ts:392`, `bound-materialize.ts:166`, `registry.ts:49`)
  collapse into one `STATE_SCHEMA_VERSION = 2`. 83 test files carry a literal 1 (`merge/validation.rs:202`,
  `lifecycle.rs:273-295`, `fixtures_scale.rs:42-107` among them); the deslop grep gate allows it only in
  `cli_record_schema_versions.rs`, which writes a foreign version on purpose.
- Frozen legacy audit: the 76 `promotion_decisions.jsonl` rows and the shipped terminal proposal rows pass the guard
  (`read_legacy_dispositions`, `readers.rs:206-217`, goes through `record_from_line`) and are rewritten. Their
  frozen digests serialize the whole record (`legacy_audit.rs:50-60`), so `SHIPPED_TERMINAL_PROPOSAL_DIGEST_V1` and
  `SHIPPED_DISPOSITION_AUDIT_DIGEST_V1` (`legacy_audit.rs:19-21`) are recomputed over the rewritten rows in the same
  commit, before and after values in the PR body. Exempting legacy rows from the guard is rejected: it covers all.

## G. The conversion

`provenance convert-edges --repo . [--dry-run] [--versions-only]`, behind `--features dogfood`, holding the
publication lock. It reads raw JSON lines below the version guard (its own line reader; `jsonl.rs` atomic writer for
output) and writes `SUPPORTED_SCHEMA_VERSION` into every record (never a literal), so it runs at K.3/K.4 with the
constant at 1 and at K.7 with it at 2. Every list step is a set union and step 9 deletes the field, so a rerun after
a crash changes nothing more; a rerun on converted state finds no edges directory and no `superseded_by` and is a
no-op. Counted at `ce891fe`: 614 rows (76 `references`, 19 `refines_into`, 0 `depends_on`, 1 `contradicts`, 0
`supersedes`, 98 `needs`, 96 `resolves`, 1 `spawns`, 323 `produces`); the decision's 612 was measured at `bf5f9c8`,
before `res_relations_are_fields_or_action_records` landed with its pair. All rows are scope `default`, unlabeled,
none dangling.

Per type:
1. `references` (source -> requirement): assert the pair is in `requirement.source_refs`; add with `clause: None`
   when absent (0 today). Report the 3 field-only pairs (all `src_annotation_format_spec`).
2. `refines_into` (parent -> child): `child.refines = parent`; refuse a second parent.
3. `depends_on`, `supersedes` (requirement): `from.depends_on += to`, `from.supersedes += to` (0 rows; the path is
   written and tested on a fixture).
4. `resolves` (resolution -> requirement): `resolution.requirement_ids += requirement`.
5. `needs` (requirement -> resolution): assert the mirrored `resolves` exists; report each pair without one and add
   it to `requirement_ids` anyway (the union). 2 today, both `req_rust_requirements_as_code_authoring` ->
   `res_sdk_engine_from_package_manager`, `res_typed_facade_owns_construction`.
6. `spawns` (resolution -> requirement): `requirement.spawned_by = resolution`; refuse two.
7. `produces` (requirement -> rule): `rule.requirement_ids += requirement`; (resolution -> rule):
   `rule.resolution_ids += resolution`.
8. `contradicts` (requirement -> requirement): mint topic `topic_contradiction_<from>` (status `explored`, so no
   `UnexploredTopic` gap) and question `q_contradiction_<from>_<to>` with `topic_id`, `requirement_id = from`,
   `contradicts = to`, method `grill`, status `open`, text "Requirement <from> contradicts requirement <to>. One of
   them must be restated or superseded.", no `resolution_id` (`req_state_merges_without_humans` is resolved by
   `res_state_is_jsonl_in_git`, `req_edge_writes_validated` by nothing). Report the unknown author and date.
9. `superseded_by` fields: for each `X.superseded_by = Y`, `Y.supersedes += X`, delete the field. 1 today:
   `res_state_is_jsonl_in_git.supersedes = [res_convex_chosen_as_backend_over_custom_web]`. Report
   `res_rule_is_the_function` (status `superseded`, no successor) and leave it.

Then: assert every rule and resolution has a non-empty `requirement_ids` (true today) and no cycle; rewrite
`schema_version` in every family; delete `.provenance/state/edges/`; print one line per type (count in, count
written) and each item from steps 1, 5, 8, 9. `--versions-only` runs only the version rewrite. `check` and
`materialize` run after it as separate commands. The subcommand is deleted in K.9.

## H. Records to retire or rewrite

- `rule_prov_edge_endpoint_table` (`rule.jsonl`, severity critical, 4 requirement producers): status `archived`,
  `retired: true`; its `#[rule]` anchor (`edge_validation.rs:14`) and six `#[verifies]` sites (97-166) go with the
  file; `coverage --validate-rules` must not report it.
- `rule_prov_relation_vocabulary_closed`: statement "Each reference field on a canonical record carries one relation
  declaration, and every reverse lookup, validator check, gap, walk, and projection row derives from that
  declaration." The description names the derive, the compile-time guard, the anchor, and the two tests.
- Anchor `req_implement_a_normalized_knowledge_graph_d` (its statement names "nine typed edges checked at write
  time"): new requirement `req_relations_are_record_fields` with `supersedes` naming it, same domain and
  `source_refs`, statement drafted with the provenance-grounded-writing skill and the checker: "The graph is four
  canonical record kinds, Source, Requirement, Resolution, and Rule, and a relation between records is a reference
  field on the record that makes the claim or the record of the action that asserted it." The old requirement and
  its topic stay. First use of `requirements create --supersedes`.
- `docs/state-format.md` (120 lines): lines 7, 15, 23, 115-120 rewritten; version 2 stated; the section C table
  added. `docs/cli.md` (471): lines 22, 138-146, 336-341, 384, 462, 466-467, 471 rewritten; the section D commands
  listed. `docs/cache.md`: the catch-up section loses "the edges shard" and "A changed global unit reloads the edges
  table whole"; the derivation table loses `edges` and gains `relations (derived, no digest row)`.
- `docs/shaping.md:63,71` reworded; the `validate_edge_endpoint` example block (233-256) is replaced by a
  `#[relation]` example. `docs/typescript-sdk-poc.md:8` reworded.

## I. Test strategy

Differential harness (`crates/provenance-cli/tests/relation_cut_parity.rs`, over a fixture copy of
`.provenance/state` at `ce891fe`): "before" snapshots of `gaps`, `prime`, `wiki` (assembled model), `traceability`
per rule, `impact` per requirement and source, `health`, `orphans`, `sdk query neighbors` and `trace` per record,
all JSON from the `main` binary, committed as files. "After" runs on the converted fixture. Normalization is this
table, applied to every edge-shaped row in the "before" files:

| Edge type | Relation | Owner flip | Rows |
|---|---|---|---|
| references | cites | yes (owner becomes the requirement) | 76 |
| refines_into | refines | yes (owner becomes the child) | 19 |
| depends_on | depends_on | no | 0 |
| contradicts | contradicts (on the question) | replaced, see below | 1 |
| supersedes | supersedes | no | 0 |
| needs | dropped | dropped | 98 |
| resolves | requirement_ids | no | 96 |
| spawns | spawned_by | yes (owner becomes the requirement) | 1 |
| produces | requirement_ids / resolution_ids | yes (owner becomes the rule) | 183 / 140 |

A flipped row also flips `direction` in a neighbor. Rows compare as sets of (owner, relation, target). The
expected-diff file then lists, by count, what is new because the walk reads every declared relation: 67 requirements
gain a `domain_id` neighbor and 8 domains the reverse; 9 questions gain `topic_id`, `requirement_id`, and (3)
`resolution_id` neighbors, 5 topics and 3 boundaries gain `requirement_id` neighbors, with the reverse rows on their
targets; prime's requirement graph for the 3 field-only citations gains `src_annotation_format_spec`; the
contradiction pair loses its direct adjacency (1 row each way) and each side gains the question as a neighbor; the
minted topic and question add their own records to neighbors and trace; gaps are unchanged (the pair gap stays, the
`explored` topic adds none). After normalization and the listed deltas every output is byte-identical; any other
difference fails the test.

Relation map: `crates/provenance-cli/tests/relation_map.rs` carries the gist's 39 rows as data (relation, owner
kind, authoring command, clear command or "immutable"). Per row it runs the authoring command on a fresh repository,
reads the record back, asserts the reference, runs the clear command where named, asserts it gone.

Named tests: `clear` on the last entry of a required list refuses (rule and resolution); `add`/`set` of a `refines`,
`depends_on`, or `supersedes` cycle refuses; the merge gate refuses an empty `requirement_ids` on a resolutions
shard and a duplicate id on a sources shard; the aggregate validator refuses an empty required list.

Mutation targets, each a deliberate break that must turn a named test red: drop one `#[relation]` attribute
(compile-time guard, trybuild); skip one owner kind in the derived-table loader (catch-up equivalence); make
`Rule.requirement_ids` optional (writer, validator, `check`, merge gate); backfill one edge type wrong (harness);
delete one table from `declared_relations()` (hand-list test); invert one relation's `flow` (harness on impact and
traceability).

Unchanged: the four catch-up equivalence suites (`cache/tests/catch_up_*.rs`); the edge cases in
`unit_digest_behavior.rs:76-176` and `catch_up_domain_coverage.rs:132-176` become `relations` cases. `trybuild` for
the derive: a bad field type, an unknown key, a missing `target`, a `StableId` field with no attribute. 49 Rust and
2 TS test files mention edges today; each is rewritten to the field it now means.

## J. Revised W3 and W5 text

The W3/W5 text is `docs/research/2026-08-27-qrspi-1wh-query-uniformity-plan.md` on branch
`opencode/provenance-20260827T223718Z-87cc1ac4`, not in the tree on `main`. The edits below land as text on the
beads (`provenance-1wh.2` for W3, `provenance-1wh.3` for W5), not as a PR against that branch.

W3, per-operation mapping, item 2: "`neighbors`, `trace` walk the derived `relations` table (`idx_relations_out`,
`idx_relations_in`). Filters name relations, not edge types (protocol 6, inherited from the relation shapes cut).
Trace gains a resume token as before. Ordering: node rank, canonical id, declaration order, direction." Item 3:
"`impact` walks downstream over the `relations` table, direction derived from each declaration's `flow`; the
indirect filter names `refines`, `depends_on`, `contradicts`, `supersedes`, `spawned_by`." Stamp rows `neighbors`,
`trace`, `impact`: attested fields unchanged; "edges + nodes" becomes "relations + nodes". Protocol flag: "Version
is 6 after the cut; W3 adds no further bump." `SqlFront`: "reads `relations` per scope; a global edges family no
longer exists."

W5 landing order, item 2: "W2 equivalence suite green; catch-up is the default freshness step. No journal." Item 3:
"Relation shapes cut (bead 1wh.1) merges; the vocabulary is the declaration table." Delete every journal sentence
(plan lines 21, 39, 47-48, 107, 273-440). Knobs: delete `cache.catchup_journal`; keep the three `read.*` knobs.
File-growth gates: drop `publication/journal.rs` and `materialize/sweep.rs`. q82f handoff item 6: "Protocol version
confirmed at 6."

## K. Sequencing inside the one PR

Branch `1wh-relation-shapes-cut`. Each commit builds and passes the workspace suite; how it stays green is stated
per commit. The version constant stays 1 until commit 7.
1. Harness "before" snapshots from the `main` binary, committed as files. `provenance-macros`: `syn`, `quote`,
   `proc-macro2`; the `Relations` derive; trybuild. Green: additive.
2. Fields on the seven structs with the derive, `declared_relations()` as the concatenation, the hand-list and
   serde-walking tests. New fields are empty and skipped on serialize, so every fixture round-trips and the old
   readers still read edges. Green: additive.
3. Writers and commands (section D) write the fields and still write the edges; the conversion subcommand (writing
   version 1 through the constant); the relation-map test. Green: readers and adoption still see the edges they
   compare against; writers double-write.
4. Readers move (section E) with adoption equality and the merge gate in the same commit; the harness runs the
   converter over the fixture and goes green. Green: every reader reads fields the writers already fill.
5. Writers stop writing edges; `add_edge` and its callers go; edge-write tests become field tests. Green: no reader
   reads edges now.
6. Projection: migration 021, the derived-table loader, catch-up changes, family table at 18. Wire: protocol 6,
   graph reference v2, TypeScript types and tests.
7. `SUPPORTED_SCHEMA_VERSION` to 2, the four production literals, the TS constant, the two frozen audit digests; run
   the converter on the live state, then `--versions-only` as a no-op check, then `check` and `materialize`; commit
   `.provenance/state` (edges directory gone, every record at 2, the topic and question from G.8, the resolution
   `supersedes`).
8. Records and docs (section H).
9. Deletions: `EdgeType`, `Edge`, `edge_validation.rs`, `RelationKind`, the edges commands, `check/edges.rs`, the
   store plumbing listed in E, the conversion subcommand.

500-line cap (`AGENTS.md:20`), split by responsibility before growth: the `add`/`clear` writers go in a new
`state_store/reference_writers.rs`, not `writers.rs` (321); the new subcommands in a new `cli/references.rs`, not
`cli/knowledge.rs` (188); `reconcile.rs` (476) splits as in section D before any field lands; field comparison lands
in `adoption/relationships.rs` (213), so `typed_specs/adoption.rs` (483) does not grow; `graph_reference.rs` (422)
shrinks. New test files stay under 300 lines.

Before ready: the deslop pass (no `edge` in the relation sense left in production code, comments, or docs; ADRs, the
graph-theory use in `lineage_validation.rs`, and `PARITY.md` are exempt; the version-literal grep gate from section
F), the six mutation runs from section I recorded in the PR body, `cargo clippy --all-targets --all-features`,
`cargo test --workspace`, the TS suite, `provenance check` clean on the converted state, then the three reviews.

## L. Risks and open points

1. Required lists and hand edits. An empty `requirement_ids` deserializes. Default: the aggregate validator,
   `check`, and the merge gate refuse it; `OrphanRule` and `OrphanResolution` gaps are deleted, not kept.
2. Contradiction topic. A question needs a topic. Default: the conversion mints one `explored` topic per
   contradiction (1 today); `questions create --contradicts` on a live repository uses the caller's topic.
3. Contradiction settled by a rejected resolution. `is_resolved` never checked status (`contradiction.rs:49-57`).
   Default: keep that behavior; leave the fix to W3.
4. `depends_on`: ruled optional list on the dependent requirement, 0 rows, no gap reads it. Default:
   `target_downstream`, treated as indirect by impact, like today's edge.
5. Neighbors order. Today: edge rank then direction (`walk.rs:149-156,173-185`). Default: declaration order (struct
   field order within the owner kind, kinds in node rank) then direction; the harness compares sets; the W3 cursor
   freezes the new order.
6. Adoption equality and the fields a declaration can also set (`refines`, `supersedes`, `spawned_by`,
   `depends_on`). Default: adoption compares only `source_refs`, `requirement_ids`, `resolution_ids`; the rest is
   "richer canonical metadata" (`docs/state-format.md:16-17`) under the section D reconcile rule.
7. Old graph references and v1 exports. Default: a reference issued before the cut does not verify; a v1 export is
   refused on its `edges` key (section F).
8. The version rewrite touches every file under `.provenance/state` and moves two frozen digests. Default: one
   commit, step 7, reviewed by count (records in equals records out per family) and by the digest pair.
9. The conversion subcommand. Default: `handlers/convert_edges.rs` behind `--features dogfood`, its logic a
   test-visible function the harness calls, deleted in step 9.
10. Reverse scan cost. In-memory scans are O(records) per lookup, as `fk`/`embedded` are today (`front.rs:376-446`).
    Default: one `BTreeMap<(kind, id), Vec<(owner, relation)>>` index per `RecordFront`, built on first use; W3
    replaces it with SQL for served reads.
11. `source-ref clear` of a citation the typed reconciler manages. Default: allowed; the next `sdk apply` restores
    it, as today's reconciler appends (`reconcile.rs:202-208`).
12. Threads and ideation records reference requirements too (map rows 22-34). Default: out of `relations` in this
    cut, in after the W3 dangling-target prerequisite; the table lists them as `X`-class rows, no loader.
13. Cycle refusal on `refines`, `depends_on`, `supersedes`. Default: the writer walks the field chain and refuses a
    path back to the owner; `check` and the aggregate validator report one found in state. Today's edges had no such
    guard.



