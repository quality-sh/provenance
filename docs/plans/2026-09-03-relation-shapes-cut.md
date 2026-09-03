# Relation shapes cut: implementation plan (revision 5)

Bead `provenance-1wh.1`. Decision: `res_relations_are_fields_or_action_records` (binding). Read at `ce891fe`; every file:line
below was counted there. Revision 5 answers both reviews (Fable 5.1, GLM-5.3-flash; branch `1wh-cut-plan`) in full. Owner
rulings: one cut, no older-binary compatibility, product words only, `depends_on` stays optional on the dependent requirement.

## A. Scope and non-goals

The cut removes the canonical edge. Each of the nine edge types becomes a field on the record that makes the claim, or a
question. One declaration per record kind names its reference fields, their target kind, requiredness, and flow; writers,
validator, gap policy, check, merge gate, traversals, and projection derive from it. Reverse lookups are derived: in memory from
fields for canonical readers, from a derived `relations` table in `provenance.db` for served reads. Existing state is converted
once by a throwaway subcommand. The schema and SDK protocol versions move. The edges shard, the edge commands, the endpoint
table, the same-fact dedupe, and `EdgeType` go in the same PR.

Non-goals: serving neighbors, trace, or impact from SQLite (W3, bead 1wh.2); freshness policy knobs and rollout order (W5, bead
1wh.3); any SaaS storage shape; the rendered graph-change view; restating other requirements.

## B. The declaration mechanism

Spiked (scratchpad `spike/relmacro`, reproduced by both reviewers): `#[derive(Relations)]` on the record struct, one
`#[relation(target = Kind, flow = ..., required, name = "...", via = field)]` per reference field. `syn` 2 reads the field type:
bare `StableId` is a required single, `Option<StableId>` an optional single, `Vec<StableId>` a list (required only with the
flag), `Vec<T>`/`Option<T>` with `via = field` a reference through a struct. `flow` is `target_upstream` (the target is upstream
of the owner), `target_downstream`, or `none`. The derive emits an associated const `Kind::RELATIONS: [RelationDecl; N]` (owner
kind, name, target kind, list, required, flow; a module-level const would collide where two kinds share a module) and `impl
RelationOwner`, whose trait carries the table: `fn relations() -> &'static [RelationDecl]`, `fn id(&self) -> &StableId`, `fn
references(&self) -> Vec<(&'static str, &StableId)>`. Every generic function (reverse scan, derived-table rows, existence check,
dangling gap, empty-required-list refusal, directed walk; section E) reaches the table through the trait. The derive refuses at
compile time a `StableId`-typed field (bare, `Option`, or `Vec`) with no `#[relation]`: the primary guard. The field named `id`
is exempt with no attribute: it is the owner key the derive locates for `RelationOwner::id()` (all seven:
`artifacts.rs:227,287,358,407`, `shaping.rs:116,128,145`). A field that points outside the table carries `#[relation(none)]` so
the exemption is visible at the field: `origin_thread` and `origin_message` on the four graph kinds
(`artifacts.rs:266,272,311,317,394,400,438,444`; threads and messages, L12). No other name list.

Seven kinds carry the derive: source, requirement, resolution, rule, topic, question, boundary. The derive cannot:
- See another struct. `declared_relations()` (`relations.rs:26-29`, keeps its `#[rule]` anchor) is a hand-written concatenation
  of the seven tables; a test with a hand list of the seven owner kinds asserts each table appears once.
- Enforce a required list at deserialize time: `serde` builds an empty `Vec`. Requiredness is enforced from the table by the
  writer, the graph validator (section E), `check`, and the merge gate.
- Declare `links[]` (topic, question): `ArtifactLink` (`shaping.rs:105-110`) carries a per-entry `target_type`. Links stay
  outside it: written through `validate_artifact_links` (`artifact_links.rs:9`), checked by `check_artifact_links`
  (`references.rs:39`), walked by one hand-written `related_nodes` contribution named `links`, replacing
  `TopicLinks`/`QuestionLinks`.
- See references not typed `StableId`. A serde-walking test guards those: per kind (seven) it serializes a fixture, recurses
  into nested objects and arrays (`SourceReference`, `ArtifactLink`, `ThreadParent`), collects every key ending `_id`/`_ids`
  plus the declared names at any depth, and asserts the set equals the declaration table plus the allow list. An empty fixture
  field is hidden by `skip_serializing_if`, missing from the set, and fails the test: fixture completeness is asserted.
- Write the TypeScript types, the JSON schema artifact, or the docs table.

The const-table alternative keeps five hand mirrors per field (struct, table row, `RecordFront` arm, SQL columns, check arm);
`graph_records.rs:39-54` already forgot `source_refs`. Cost of the derive: `syn`, `quote`, `proc-macro2` in `provenance-macros`
(in `Cargo.lock` via `serde_derive`; the crate has no dependencies today) and a `trybuild` suite (pattern:
`crates/provenance-sdk/tests/compile_fail.rs`). Recommendation: the derive. `RelationKind` (`relations.rs:49-71`),
`RelationDerivation` (33-40), `same_fact_as` (215-239), `drop_duality_echoes` (`front.rs:94-118`) are deleted. `RelationSource`,
`related_nodes`, `RelationDirection`, `RelationEndpoint` (`front.rs:17-92`) stay; `RecordFront` (121-131) loses `edges`, and its
per-kind arms (170-323, 338-372) become one generic walk over `RelationOwner` plus the `links` function. The scanner matches the
word `rule` (`parser.rs:184`); `#[relation(...)]` is invisible to it.

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
| source | `supersedes` (the typed declaration gains it too) | source | optional | list | target_downstream | `superseded_by`, inverted |
| topic | `requirement_id` | requirement | required | single | none | unchanged (`shaping.rs:129-130`) |
| boundary | `requirement_id`; `source_ref.source_id`, name `cites` | requirement; source | required; optional | single | none | unchanged (`shaping.rs:117-121`) |
| question | `topic_id`, `requirement_id`; `resolution_id` | topic, requirement; resolution | required; optional | single | none | unchanged (`shaping.rs:146-168`) |
| question | `contradicts` | requirement | optional | single | none | `contradicts` edge, other side in `requirement_id` |

Relation names (the one vocabulary of `Neighbor.relation`, the `relations` filter and table, `docs/cli.md`, and the W3 filters):
`domain_id`, `cites` (requirement `source_refs`, boundary `source_ref`), `refines`, `depends_on`, `supersedes` (requirement,
resolution, source), `spawned_by`, `requirement_ids` (rule, resolution), `resolution_ids`, `requirement_id` (topic, boundary,
question), `topic_id`, `resolution_id`, `contradicts`, `links` (topic, question): 13 names over 20 declarations. A shared name
filters every owner kind that carries it; the row's owner kind tells them apart. Data forced the lists: 14 rules have 2+
requirement producers, 4 have 2+ resolution producers, one resolution resolves 3 requirements. It allowed the singles: 19
`refines_into` rows, 19 distinct children; 1 `spawns` row.

`superseded_by` today (Source `artifacts.rs:255-260`, Resolution 381-386) sits on the older record but is set on the record
being created (`--superseded-by` at `knowledge.rs:28-29`, `policy.rs:42-43`). Its direction inverts: the newer record holds
`supersedes: [older]`. The struct fields, both flags, and every reader of the field go together at K.4 (file list there); the
converter carries the one live value to `supersedes` at K.7 from raw JSON (L15). The wiki "Superseded by" line and the source
page's superseded badge stay (`wiki/model.rs:273,351`; `render/pages/source.rs:73-87`, `render/pages/resolution.rs:136-137`),
fed by the reverse scan over `supersedes` (`assemble/pages/source.rs:26-29`, `assemble/pages/resolution.rs:47-53`). After G.9
the value sits on `res_state_is_jsonl_in_git.supersedes`; the `res_convex_chosen_as_backend_over_custom_web` page still says
"Superseded by res_state_is_jsonl_in_git" and the `res_state_is_jsonl_in_git` page gains no line; the wiki snapshot is
unchanged. `CreateProposalCardInput.superseded_by` (`inputs.rs:317`) is untouched. A root requirement is a requirement whose
`refines` is absent; 49 of 68 are roots.

A contradiction is a question: `topic_id` and `requirement_id` name one side, `contradicts` the other. It is settled when
`resolution_id` is set or either requirement lists the other in `supersedes`. Today's shared-resolution settle path
(`contradiction.rs:49-57`) is dropped on purpose: the question's `resolution_id` records that settlement. The unordered pair
(`requirement_id`, `contradicts`) is the gap identity.

## D. Commands and SDK calls that change

The owner flag names the owner kind; the target flag is `--target-id` (an existing word, `contributions create --target-id`).
`add` and `set` refuse a missing target and a cycle. `clear` on a list names the entry; on a single it names nothing; on a
required list it refuses the last entry ("a rule needs one requirement").

Added (CLI, each backed by one `StateStore` method of the same name):
- `requirements refines set|clear --requirement-id`, `requirements depends-on add|clear`, `requirements supersedes add|clear`,
  `requirements spawned-by set|clear`, `requirements source-ref clear --requirement-id --source-id` beside the existing `add`
  (`knowledge.rs:172-188`; subsumes "remove").
- `rules requirement|resolution add|clear --rule-id`, `resolutions requirement|supersedes add|clear --resolution-id`, `sources
  supersedes add|clear --source-id`, `questions contradicts set|clear --id`.
- `requirements create` gains `--refines`, `--spawned-by`, repeatable `--depends-on` and `--supersedes`; `questions create`
  gains `--contradicts`. `provenance convert-edges` (section G), dev build only.

Removed: `edges create|list|delete` (`cli/graph.rs:5-43`, `handlers/edges.rs`, 57 lines, `cli.rs:123-126`,
`handlers/mod.rs:15,117-118`); `--superseded-by` on `sources create` and `resolutions create`; `check`'s edge pass
(`handlers/check/edges.rs`, 45 lines, `check.rs:145`).

Changed (`sdk query neighbors|trace`: section F):
- `rules create --requirement-id` required and repeatable, `--resolution-id` repeatable (`policy.rs:75-80`,
  `handlers/rules.rs:96-97`, `inputs.rs:126-127` become `Vec`); `resolutions create --requirement-id` likewise
  (`policy.rs:16-17`, `handlers/resolutions.rs:40`, `inputs.rs:105`); `write_rule` (`rule_writers.rs:93-174`) writes the lists
  and `write_resolution` (`rule_writers.rs:10-87`) writes `requirement_ids`; neither writes an edge.
- `sources create --supersedes`, `resolutions create --supersedes`, repeatable, existence checked (`create_source`,
  `writers.rs:11-55`, checks nothing today); `requirements source-ref add` returns the requirement, not an `Edge`
  (`writers.rs:131-187`, `handlers/requirements.rs:52-60`).
- `sdk apply`: `desired_rule` (`reconcile.rs:250-274`) writes `requirement_ids` and `resolution_ids`; `desired_requirement`
  (142-174) writes `refines`, `spawned_by`, `depends_on`, `supersedes`; `desired_source` (45-70) writes `supersedes`. A
  declaration field is authoritative when present and untouched when absent, so a CLI-set `refines` survives a spec that does
  not name one; `source_refs` keeps today's append (`reconcile.rs:202-208`). The edge write
  (`typed_specs/relationships.rs:8-37`) and stale-edge delete (68-145) go; adoption equality
  (`adoption/relationships.rs:59-148`) compares the `source_refs`, `requirement_ids`, `resolution_ids` fields;
  `CurrentTypedState.edges` (`typed_specs.rs:33,284-287`) goes. `reconcile.rs` (476 lines) splits first into
  `reconcile/{sources,requirements,rules,changes}.rs`; the field reconcile lands in `reconcile/references.rs`.

## E. The derived relation table and the readers

SQLite, migration `021_relations_table.sql`: drop `edges` and its four indexes (`002:22-33`, `005:1-2`); create

```
relations(scope_id, owner_type, owner_id, relation, target_type, target_id,
          PRIMARY KEY (scope_id, owner_type, owner_id, relation, target_id))
idx_relations_out (scope_id, owner_type, owner_id, relation)
idx_relations_in  (scope_id, target_type, target_id, relation)
```

`ProjectionFamily::Edges` (`projection_families.rs:30,53,77,93-96,105-106,140-142`) is deleted; `ALL` becomes 18 rows;
`is_scoped` goes. `relations` is not a family and has no digest row: every row derives from one owner record, no join. The
loader is one generic function over `RelationOwner` plus the `links` function, run per scope after `load_scope`. Catch-up:
`rederive_scope` (`catch_up.rs:205-225`) deletes and reloads the scope's `relations` rows for each owner kind whose family
digest moved; `remove_departed_scopes` (148-177) deletes the scope's rows explicitly; the `Unit::Global` arm of
`apply_unit_change` (181-201) only updates the unit digest row, since no family derives from the manifest or the dictionary
(`docs/cache.md` table); `Unit::Global` stays. No digest row can catch a skipped owner kind, so the catch-up equivalence suites
compare the `relations` table content (full dump in primary-key order) between catch-up and rebuild, as for the 18 family
tables. `load_edges` (`graph_records.rs:182-196`), `materialize.rs:70`, `family_rows.rs:43-45,87`, `projection_digest.rs:17-18`
lose their edge branches; `scope_locality_guard.rs:132-147` becomes a scope-only assertion; the `edges` fixture at
`projection_digest_sensitivity.rs:29` becomes a record with relation fields.

Walks derive from `flow`: downstream is out over `target_downstream` and in over `target_upstream`; upstream is the mirror.
`none` relations are never followed by impact or traceability; trace and neighbors follow them as `direction` admits. Today
`queries/impact.rs:34-58` follows `from_id` only and `cache/impact.rs:81-83` reads downstream as `to_id`; after the cut a source
has no out relation with rows (its `supersedes` is `target_downstream`, empty today), so "out only" is empty. A source reaches
requirements through `cites` (in), a requirement its rules and resolutions through `requirement_ids` (in). Readers that move
(edge rows to in-memory field scans unless noted):
- gaps `graph_query.rs` (285 lines): `edge_exists` (70-85), `resolution_resolves_any_requirement` (146-156),
  `missing_rule_producers` (236-244), `RuleProducer` (8-32), `GapGraph.edges` (42) deleted; `resolving_resolutions` (130-144)
  reads `resolution.requirement_ids`; the four produced/producing joins (158-232) read the rule lists;
  `requirement_has_valid_source` (255-267) and `source_is_referenced` (269-284) read `source_refs` only.
- gaps `frontier.rs` (135): the seven kinds at 7-48, 81-135 keep their text. `OrphanResolution` (50-61) and `OrphanRule` (65-79)
  are deleted: the type requires the list, and the graph validator, `check`, and the merge gate refuse an empty one. A question
  with `contradicts` set is excluded from `OpenQuestion`. `contradiction.rs` (66): iterates questions with `contradicts`;
  `is_resolved` (33-58) reads `resolution_id` and both `supersedes` lists; kind and text unchanged.
- gaps `dangling.rs` (225): the edge passes (159-225) deleted; one generic pass over `declared_relations()` reports a dangling
  target per field as `"<relation> points at missing <kind> <id>"`; the source, resolution, topic, and question passes (38-139)
  fold into it; links and thread parents stay hand-written. `state_adapter.rs` (165): `GraphRecords.edges` (29, 44, 94-97) goes;
  retired resolutions (49-66) derive from every `requirement_ids` entry retired. `prime.rs` (149): `RequirementGraphView.edges`
  (12) becomes `relations: Vec<RelationRow>` (owner kind, owner id, relation, target kind, target id);
  `get_requirement_graph_locked` (116-149) reads `source_refs`.
- `impact.rs` (126): the directed walk above; `follow_indirect` (48-58) excludes `refines`, `depends_on`, `contradicts`,
  `supersedes`, `spawned_by` by name. `traceability.rs` (115): upstream walk from the rule over `requirement_ids`,
  `resolution_ids`, the resolutions' `requirement_ids`, `cites`; `edges` (17) becomes `relations`.
- `health.rs` (272): `graph_evidence_locked` (73, 88-99) reads `source_refs` only; `coverage_health_locked` (181-212) reads the
  three lists; `orphan_rules_locked` (247-272) reports `missing: ["source"]` only.
- wiki assemble: `context.rs:25-53` goes; `traversal.rs:9-22,77-108`, `discovery.rs:333-341`, `pages/requirement.rs:44-56` read
  `refines`; `evidence.rs:87-115` drops its edge branch (`label` never set, 0 of 614); `pages/resolution.rs:12-41` reads
  `requirement_ids`, scans `spawned_by`; `pages/rule.rs:9-36,47-66` and `pages/source.rs:9-24` read the lists and `source_refs`;
  `assemble.rs:67` and `ScopeExport.edges` (`export.rs:27,59-63`) go.
- `operations/queries/walk.rs` (185): `scoped_edges` (11-19), `steps` (30-58), `edge_rank` (173-185) go; `neighbors` (64-96) and
  `trace` (98-147) walk `related_nodes` over a `RecordFront` from `records::load`. Order: node rank, id, declaration order,
  direction. `queries/impact.rs:27,34-58` uses the downstream walk.
- `operations/plan.rs:131-139`, `requirement_reviews.rs:132-149` (`typed_specs.rs:252`) scan the rule lists. Merge gate
  `merge/validation.rs` (328): `ShardFamily::Edges` (36, 62-63, 103) and `validate_merged_edges` (182-194) go; `ShardFamily`
  gains `Sources`, `Resolutions`, `Questions`, `Topics`, `Boundaries`, each deserialized as its type, and every recognized
  family runs the required-list check from the table.
- graph validator: `validate_ideation_scope_snapshot` (`ideation_batches.rs:143-185`) reads only the ideation families, so it
  cannot see rules, resolutions, or requirements. A sibling `validate_graph_scope` in a new `state_store/graph_validation.rs`
  loads the scope's seven kinds (same directory; locality guard holds), refuses an empty required list and a
  `refines`/`depends_on`/`supersedes` cycle in state, and is called beside the ideation validator at its five call sites
  (`state_store.rs:303`, `materialize.rs:52`, `catch_up.rs:64`, `handlers/export.rs:46`, `handlers/dispositions.rs:80`) and from
  `check`. Adding family reads to the ideation validator is rejected: two subjects, one file.
- `check`: `check/edges.rs` goes; the question, topic, and boundary key checks at `check/scope/core.rs:274-332` become one
  generic pass over `declared_relations()` against `CheckIndex`; links and origin checks stay. Store plumbing deleted:
  `CreateEdgeInput` (`inputs.rs:54-61`), `list_edges`/`closed_edges` (`state_store.rs:167-169,244-246`), the edge writers
  (`writers.rs:189-320`), the edge readers (`readers.rs:357-428`), `shards::edges_path`, `layout.edges_dir`.

## F. Wire and formats

- SDK protocol 6 (`protocol.rs:25`, `engine.ts:17,35-38`). `Neighbor.edge_type` (`node.rs:107-113`, `protocol.ts:246-250`)
  becomes `relation: String` (a section C name); `NeighborsQuery.edge_types` and `TraceQuery.edge_types` (`query.rs:78,97`,
  `protocol.ts:256` and the trace request) become `relations: Vec<String>`, an unknown name refused. `EdgeType`
  (`graph.rs:62-99`, `protocol.ts:186-195`, `index.ts:122`) and `Edge` (`graph.rs:101-135`) are deleted. `Direction` stays;
  neighbors are every declared relation both ways (`docs/cli.md:138-146`).
- Graph reference v2: `GraphExport` (`projection.rs:29-48`) loses `edges` (47), `load_projection` loses `closed_edges` (89),
  `validate_schema_versions` its edge arm (120); `GraphCounts.edges` (`graph_reference.rs:85,337`) goes. The JSON schema
  artifact (`schema/artifacts/graph_reference.rs`) drops the `edges` family (49, 64) and the `edge` record (231-236); its source
  (130-144) and resolution (188-199) records drop `superseded_by` (142, 197) and gain `supersedes` (id array); requirement
  (149-159) gains `refines`, `spawned_by`, `depends_on`, `supersedes`; resolution gains required `requirement_ids`; rule (200-)
  gains required `requirement_ids` and `resolution_ids`; question (177-187) gains `contradicts`. `grf1_`/`git1_` formats stay.
  Every graph digest moves; pre-cut references stop verifying (owner accepted). Export/import: `ScopeExport.edges`
  (`export.rs:27,59-63,94-132`) and import's edge branch (`import.rs:39-42,52`, `import/scope_writer.rs:52,111-189`) go. A v1
  export is refused by `deny_unknown_fields` (`export.rs:8`) on its `edges` key, serde's message; none names the converter.
- Typed declarations: `RequirementDeclaration` (`protocol.ts:34-40`) gains optional `refines`, `supersedes[]`, `depends_on[]`
  (keys of the same spec) and `spawned_by` (a resolution id); `SourceDeclaration` (25-32) gains `supersedes[]` (keys);
  `RuleDeclaration` (42-52) gains `resolution_ids[]`. The three `Typed*Input`s (`typed_spec.rs:50-96`) mirror this. A rule with
  no requirement is already refused by `normalize_rule_relationships` (`authoring/checks.rs:85-117`, "must refine at least one
  requirement"); the single refusal site on the typed path; the graph validator re-checks the written record. No resolution
  declaration exists; a resolution's `requirement_ids` stays CLI-authored. The TS tests reading `edges-00.jsonl`
  (`bound-spec.test.ts:395-400`, `fluent-spec.test.ts:451-456`) read `rule.jsonl`.
- State schema version: `SUPPORTED_SCHEMA_VERSION` (`aggregate_validation.rs:19`) becomes `SchemaVersion(2)`. The guard is
  global and exact (`readers.rs:85-100,143-155`; `state_store.rs:100` for the manifest), so the conversion rewrites every record
  in every family, the manifest, and nested landing records. Production literals become it: `manifest.rs:36` (`init`'s default),
  `scope.rs:70`, `threads.rs:60`, `handlers/rules.rs:145`; the TS literals (`protocol.ts:60`, `spec.ts:326`,
  `fluent-spec.ts:392`, `bound-materialize.ts:166`, `registry.ts:49`) collapse into `STATE_SCHEMA_VERSION = 2`. 83 test files
  carry a literal 1 (e.g. `merge/validation.rs:202`, `lifecycle.rs:273-295`, `fixtures_scale.rs:42-107`); the deslop grep gate
  allows it only in `cli_record_schema_versions.rs`, which writes a foreign version on purpose.
- Frozen legacy audit: the 76 `promotion_decisions.jsonl` rows and the shipped terminal proposal rows pass the guard
  (`read_legacy_dispositions`, `readers.rs:206-217`, goes through `record_from_line`) and are rewritten. Their frozen digests
  serialize the whole record (`legacy_audit.rs:50-60`), so `SHIPPED_TERMINAL_PROPOSAL_DIGEST_V1` and
  `SHIPPED_DISPOSITION_AUDIT_DIGEST_V1` (`legacy_audit.rs:19-21`) are recomputed over the rewritten rows in the same commit,
  before and after values in the PR body. Exempting legacy rows from the guard is rejected: it covers all.

## G. The conversion

`provenance convert-edges --repo . [--dry-run] [--versions-only]`, behind `--features dogfood`, holding the publication lock. It
reads raw JSON lines below the version guard (its own line reader; `jsonl.rs` atomic writer for output) and writes
`SUPPORTED_SCHEMA_VERSION` into every record (never a literal), so it runs at K.3/K.4 with the constant at 1 and at K.7 with it
at 2. Every step is idempotent: list steps are set unions, the step 8 mint is keyed by its deterministic ids and skips an
existing topic or question, step 9 deletes the field, the shard is deleted last; a rerun after a crash redoes nothing already
done, and a rerun on converted state changes nothing. Recovery from a half-run is `git checkout -- .provenance/state` and a
rerun. Counted at `ce891fe`: 614 rows (76 `references`, 19 `refines_into`, 0 `depends_on`, 1 `contradicts`, 0 `supersedes`, 98
`needs`, 96 `resolves`, 1 `spawns`, 323 `produces`); the decision's 612 predates `res_relations_are_fields_or_action_records`
and its pair. All rows are scope `default`, unlabeled, none dangling. Per type:
1. `references` (source -> requirement): assert the pair is in `requirement.source_refs`; add with `clause: None` when absent (0
   today). Report the 3 field-only pairs (all `src_annotation_format_spec`).
2. `refines_into` (parent -> child): `child.refines = parent`; refuse a second parent.
3. `depends_on`, `supersedes`: `from.depends_on += to`, `from.supersedes += to` (0 rows; fixture-tested).
4. `resolves` (resolution -> requirement): `resolution.requirement_ids += requirement`.
5. `needs` (requirement -> resolution): assert the mirrored `resolves` exists; report each pair without one and add it to
   `requirement_ids` anyway (the union). 2 today, both `req_rust_requirements_as_code_authoring` ->
   `res_sdk_engine_from_package_manager`, `res_typed_facade_owns_construction`.
6. `spawns` (resolution -> requirement): `requirement.spawned_by = resolution`; refuse two.
7. `produces`: `rule.requirement_ids += requirement` or `rule.resolution_ids += resolution`, by producer kind.
8. `contradicts` (requirement -> requirement): mint topic `topic_contradiction_<from>` with `requirement_id = from`
   (`shaping.rs:129-130`), status `explored` (no gap), and question `q_contradiction_<from>_<to>` with that `topic_id`,
   `requirement_id = from`, `contradicts = to`, method `grill`, status `open`, text "Requirement <from> contradicts requirement
   <to>. One of them must be restated or superseded.", no `resolution_id` (`req_state_merges_without_humans` is resolved by
   `res_state_is_jsonl_in_git`; `req_edge_writes_validated` by none). Report the unknown author and date.
9. `superseded_by` fields: for each `X.superseded_by = Y`, `Y.supersedes += X`, delete the field. 1 today:
   `res_state_is_jsonl_in_git.supersedes = [res_convex_chosen_as_backend_over_custom_web]`. Report `res_rule_is_the_function`
   (status `superseded`, no successor) and leave it.

Then: assert every rule and resolution has `requirement_ids` (true today) and no cycle; rewrite `schema_version` in every
family; delete `.provenance/state/edges/`; print one line per type (in, written) and each item from steps 1, 5, 8, 9.
`--versions-only` runs only the version rewrite. `check` and `materialize` follow separately; the subcommand goes in K.9.

## H. Records to retire or rewrite

- `rule_prov_edge_endpoint_table` (severity critical, 4 requirement producers): status `archived`, `retired: true`; its
  `#[rule]` anchor (`edge_validation.rs:14`) and six `#[verifies]` sites (97-166) go with the file; `coverage --validate-rules`
  must not report it. `rule_prov_relation_vocabulary_closed`: statement "Each reference field on a canonical record carries one
  relation declaration, and every reverse lookup, validator check, gap, walk, and projection row derives from that declaration."
  The description names the derive, the compile-time guard, the anchor, and both tests.
- Anchor `req_implement_a_normalized_knowledge_graph_d` (its statement names "nine typed edges checked at write time"): new
  requirement `req_relations_are_record_fields` with `supersedes` naming it, same domain and `source_refs`, statement drafted
  with the grounded-writing skill and the checker: "The graph is four canonical record kinds, Source, Requirement, Resolution,
  and Rule, and a relation between records is a reference field on the record that makes the claim or the record of the action
  that asserted it." The old requirement and its topic stay. First use of `requirements create --supersedes`.
- `docs/state-format.md` (120 lines): lines 7, 15, 23, 115-120 rewritten; version 2 stated; the section C table added.
  `docs/cli.md` (471): lines 22, 138-146, 336-341, 384, 462, 466-467, 471 rewritten; the section D commands listed.
  `docs/shaping.md:63,71` and `docs/typescript-sdk-poc.md:8` reworded; the `validate_edge_endpoint` example block
  (`shaping.md:233-256`) becomes a `#[relation]` example.
- `docs/cache.md`, sentence by sentence: line 28 "the manifest, the edge shards, and the dictionary" becomes "the manifest and
  the dictionary"; lines 35-37 "A changed global unit reloads the edges table whole. Edge rows belong to the global unit and are
  never deleted on a scope change or a scope departure." become "A changed global unit updates its digest row; no family derives
  from it. A changed scope unit also reloads the scope's `relations` rows for each owner kind whose family moved."; line 37 "A
  departed scope loses its rows in the eighteen scoped tables and its digest rows." becomes "A departed scope loses its rows in
  the eighteen tables, its `relations` rows, and its digest rows."; table row 66 (`edges`) becomes "`relations` | derived from
  the seven owner kinds' reference fields; no digest row | the scope's own shards".

## I. Test strategy

Differential harness (`crates/provenance-cli/tests/relation_cut_parity.rs`, over a copy of `.provenance/state` at `ce891fe`):
"before" snapshots of `gaps`, `prime`, `wiki` (assembled model), `traceability` per rule, `impact` per requirement, source, and
resolution, `health`, `orphans`, `sdk query neighbors` and `trace` per record under direction `both` (under `out` or `in` every
flipped relation changes reachability, so they cannot be normalized), all JSON from the `main` binary, committed. "After" runs
on the converted fixture. Normalization is this table over every edge-shaped "before" row:

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

A flipped row also flips `direction` in a neighbor. Neighbor, prime, and traceability rows compare as sets of (owner, relation,
target); trace rows are (node, depth) and compare as sets after the deltas below. The expected-diff file lists, by count, what
is new because the walk reads every declared relation: 67 requirements gain a `domain_id` neighbor and 8 domains the reverse; 9
questions gain `topic_id`, `requirement_id`, and (4) `resolution_id` neighbors, 5 topics and 3 boundaries gain `requirement_id`
neighbors, 1 boundary gains a `cites` neighbor, with the reverse rows on their targets; `res_state_is_jsonl_in_git` gains
`supersedes` `res_convex_chosen_as_backend_over_custom_web` and the reverse (1 row each way, never shown by the edge-only walk);
prime's requirement graph for the 3 field-only citations gains `src_annotation_format_spec`; the contradiction pair loses its
direct adjacency (1 row each way), each side gains the question as a neighbor, and in trace each side reaches the other at depth
2 through the minted question instead of 1, as does every node whose shortest path ran through that pair (by id; L14); the
minted topic and question add their records; impact from each of the 94 resolutions loses its requirements at depth 1 (96 rows)
and whatever was reached only through them, because `requirement_ids` is `target_upstream` where `resolves` put them downstream
(per resolution); gaps and wiki pages are unchanged (the pair gap stays, the `explored` topic adds none, the "Superseded by"
line comes from the reverse scan). After normalization and the listed deltas every output is byte-identical; all else fails.

The parity fixture has 0 dangling rows, so a second fixture plants one dangling target per relation class (single:
`requirement.refines`; list: `rule.resolution_ids`; via-struct: `requirement.source_refs`; `links`; thread parent). The gaps
test asserts the exact wording (section E) on it and the `check` test the refusal, so every class is exercised.

Relation map: `crates/provenance-cli/tests/relation_map.rs` carries the gist's 39 rows as data (relation, owner kind, authoring
command, clear command or "immutable"); per row it authors on a fresh repository, reads back, asserts, clears where named,
asserts it gone. Rows whose act is derived (row 13, `question.requirement_id` copied from the topic) are read back only. Rows
with no authoring path (row 30, proposal `duplicate_of`/`superseded_by`, which ADR 0001 routes through a disposition this cut
does not build) are excluded by name, reason in the data file.

Named tests: `clear` on the last entry of a required list refuses (rule and resolution); `add`/`set` of a `refines`,
`depends_on`, or `supersedes` cycle refuses; the merge gate refuses an empty `requirement_ids` on a resolutions shard and a
duplicate id on a sources shard; the graph validator refuses an empty required list and a cycle. Mutation targets, each a break
that must turn a named test red: drop one `#[relation]` attribute (compile guard, trybuild); skip one owner kind in the
derived-table loader (catch-up content); make `Rule.requirement_ids` optional (writer, validator, `check`, merge gate); backfill
one type wrong (harness); drop one table from `declared_relations()` (hand list); invert one relation's `flow` (harness).

Unchanged: the five catch-up equivalence suites (`cache/tests/catch_up_*.rs`) except the `relations` content comparison they
gain; the edge cases in `unit_digest_behavior.rs:76-176` and `catch_up_domain_coverage.rs:132-176` become `relations` cases.
`trybuild` for the derive: a bad field type, an unknown key, a missing `target`, a `StableId` field with no attribute. 49 Rust
and 2 TS test files mention edges today; each is rewritten to the field it means.

## J. Revised W3 and W5 text

The W3/W5 text is `docs/research/2026-08-27-qrspi-1wh-query-uniformity-plan.md` on branch
`opencode/provenance-20260827T223718Z-87cc1ac4`, not on `main`. The edits land as text on the beads (`provenance-1wh.2` for W3,
`provenance-1wh.3` for W5), not as a PR to that branch.

W3, per-operation mapping, item 2: "`neighbors`, `trace` walk the derived `relations` table (`idx_relations_out`,
`idx_relations_in`). Filters name relations from the section C vocabulary, not edge types (protocol 6, inherited from the
relation shapes cut). Trace gains a resume token as before. Ordering: node rank, canonical id, declaration order, direction."
Item 3: "`impact` walks downstream over the `relations` table, direction derived from each declaration's `flow`; the indirect
filter names `refines`, `depends_on`, `contradicts`, `supersedes`, `spawned_by`." Stamp rows `neighbors`, `trace`, `impact`:
attested fields unchanged; "edges + nodes" becomes "relations + nodes". Protocol flag: "Version is 6 after the cut; W3 adds no
further bump." `SqlFront`: "reads `relations` per scope; no global edges family exists."

W5 landing order, item 2: "W2 equivalence suite green; catch-up is the default freshness step. No journal." Item 3: "Relation
shapes cut (bead 1wh.1) merges; the vocabulary is the declaration table." Delete the journal sentences (plan lines 21, 39,
47-48, 107, 273-440). Knobs: delete `cache.catchup_journal`; keep the `read.*` knobs. Gates: drop `publication/journal.rs` and
`materialize/sweep.rs`. q82f item 6: "Protocol version confirmed at 6."

## K. Sequencing inside the one PR

Branch `1wh-relation-shapes-cut`. Every commit passes the suite; how, per commit. The constant stays 1 until K.7.
1. Harness "before" snapshots from the `main` binary, committed as files. `provenance-macros`: `syn`, `quote`, `proc-macro2`;
   the `Relations` derive; trybuild. Green: additive.
2. Fields on the seven structs with the derive, `declared_relations()` as the concatenation, the hand-list and walking tests.
   New fields are empty and unserialized, so every fixture round-trips and old readers still read edges. Green: additive.
3. Writers and commands (section D) write the fields and still the edges; `superseded_by` and flags stay; the converter (version
   1 via the constant); the map test. Green: readers and adoption still see edges and the field; writers write both.
4. Readers move (section E) with adoption equality and the merge gate in the same commit, and `superseded_by` goes from every
   site at once: `artifacts.rs:255-260,381-386`, `inputs.rs:23,116`, `knowledge.rs:28-29`, `policy.rs:42-43`,
   `handlers/sources.rs:22,38`, `handlers/resolutions.rs:30,51`, `writers.rs:22,43`, `rule_writers.rs:26,55`, `reconcile.rs:66`,
   `graph_records.rs:28-32,146-153`, `dangling.rs:38-72`, `check/scope/core.rs:210-218,344-352`, `wiki/model.rs:273,351` (the
   `PageLink` stays, its source changes), `wiki/assemble/pages/source.rs:26-29,40`,
   `wiki/assemble/pages/resolution.rs:47-53,72`, `wiki/render/pages/source.rs:18,73-87`,
   `wiki/render/pages/resolution.rs:20,136-137`, `schema/artifacts/graph_reference.rs:142,197`, and the fixtures
   `wiki/render/tests/fixtures.rs`, `wiki/assemble/tests/fixtures.rs`. The harness runs the converter over the fixture and goes
   green. Green: no site names the field after this commit, and every reader reads fields the writers already fill.
5. Writers stop writing edges; `add_edge` and callers go; edge tests become field tests. Green: nothing reads edges.
6. Projection: migration 021, derived-table loader, catch-up, 18 families. Wire: protocol 6, graph reference v2, TS.
7. `SUPPORTED_SCHEMA_VERSION` to 2, the four production literals, the TS constant, the two frozen audit digests; run the
   converter on the live state, then `--versions-only` as a no-op check, then `check` and `materialize`; commit
   `.provenance/state` (no edges directory, every record at 2, G.8's topic and question, G.9's `supersedes`).
8. Records and docs (section H). 9. Deletions: `EdgeType`, `Edge`, `edge_validation.rs`, `RelationKind`, the edges commands,
   `check/edges.rs`, the store plumbing listed in E, the conversion subcommand.

500-line cap (`AGENTS.md:20`), split by responsibility before growth: the `add`/`clear` writers go in a new
`state_store/reference_writers.rs`, not `writers.rs` (321); the new subcommands in a new `cli/references.rs`, not
`cli/knowledge.rs` (188); `reconcile.rs` (476) splits per section D before any field lands; field comparison lands in
`adoption/relationships.rs` (213), so `typed_specs/adoption.rs` (483) does not grow; `graph_reference.rs` (422) shrinks. New
test files stay under 300 lines. Before ready: the deslop pass (no `edge` in the relation sense left in production code,
comments, or docs; ADRs, the graph-theory use in `lineage_validation.rs`, and `PARITY.md` exempt; the version-literal grep gate
from F), the six mutation runs from I in the PR body, `cargo clippy --all-targets --all-features`, `cargo test --workspace`, the
TS suite, `provenance check` clean on the converted state, then the reviews.

## L. Risks and open points

1. Required lists and hand edits. An empty `requirement_ids` deserializes. Default: the graph validator, `check`, and the merge
   gate refuse it; `OrphanRule` and `OrphanResolution` gaps are deleted, not kept.
2. A contradiction question needs a topic. Default: the converter mints one `explored` topic per contradiction (1 today);
   `questions create --contradicts` uses the caller's topic.
3. Contradiction settled by a rejected resolution. `is_resolved` ignores status (`contradiction.rs:49-57`). W3 fixes it.
4. `depends_on`: ruled optional list on the dependent requirement, 0 rows, no gap reads it. Default: `target_downstream`,
   treated as indirect by impact, like today's edge.
5. Neighbors order. Today: edge rank then direction (`walk.rs:149-156,173-185`). Default: declaration order (struct field order
   within the owner kind, kinds in node rank) then direction; sets in the harness; W3's cursor freezes it.
6. Adoption equality and the fields a declaration can also set (`refines`, `supersedes`, `spawned_by`, `depends_on`). Default:
   adoption compares only `source_refs`, `requirement_ids`, `resolution_ids`; the rest is "richer canonical metadata"
   (`docs/state-format.md:16-17`) under the section D reconcile rule.
7. Old references and v1 exports. Default: a pre-cut reference does not verify; a v1 export fails on its `edges` key (F).
8. The version rewrite touches every file under `.provenance/state` and moves two frozen digests. Default: one commit, step 7,
   reviewed by count (records in equals records out per family) and by the digest pair.
9. The converter. Default: `handlers/convert_edges.rs` behind `--features dogfood`, its logic a test-visible function the
   harness calls, deleted in K.9.
10. Reverse scan cost. In-memory scans are O(records) per lookup, as `fk`/`embedded` today (`front.rs:376-446`). Default: one
    `BTreeMap<(kind, id), Vec<(owner, relation)>>` index per `RecordFront`, built lazily; W3 uses SQL.
11. `source-ref clear` of a citation the typed reconciler manages. Default: allowed; the next `sdk apply` restores it (today's
    reconciler appends, `reconcile.rs:202-208`).
12. Threads and ideation records reference requirements too (map rows 22-34). Default: out of `relations` in this cut, in after
    the W3 dangling-target prerequisite; the table lists them as `X`-class rows, no loader.
13. Cycle refusal on `refines`, `depends_on`, `supersedes`. Default: the writer walks the field chain and refuses a path back to
    the owner; the graph validator (section E) and `check` refuse one found in state; edges had no guard.
14. Trace deltas reach past the pair. BFS at default `max_depth` 3 changes 20 origins: from `req_state_merges_without_humans` 11
    nodes drop past the horizon (e.g. `req_sources_ground_requirements`, `rule_prov_relation_vocabulary_closed`,
    `res_relations_are_fields_or_action_records`) and 4 shift 2 to 3; from `req_edge_writes_validated`,
    `res_state_is_jsonl_in_git`, `rule_record_merge`, and `source_opencode_session_jsonl_storage` shift 2 to 3. The
    expected-diff file lists horizon drops per origin too.
15. The live `superseded_by` value between K.4 and K.7. Once the struct field is gone, a branch binary that writes the live
    `res.jsonl` rewrites the whole shard and drops the unknown field (`readers.rs:72-77`) before the converter reads it. Convert
    from the state committed on `main`; never run the branch binary there before K.7.
