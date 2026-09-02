# Proposal: the record is the unit of authorship

Question: `q_relations_first_class_or_properties` under `topic_relation_shapes`.
Anchor: `req_implement_a_normalized_knowledge_graph_d`.
Stance: record-owned references. A relation is a field on the record that makes the claim.
Status: disposable tournament artifact. Not a decision.

## 1. Manifesto

A record is authored, reviewed, and merged as one JSONL line. Everything that record
asserts about the world lives on that line. That includes what it points at.

Values:

- A reference belongs to the record that makes the claim. A requirement claims its
  grounding. A rule claims the requirement it refines. A successor claims what it replaces.
- An edge exists only where neither side makes the claim.
- A typed declaration already states a record this way. `RequirementDeclaration.sources`
  and `RuleDeclaration.requirement` nest the reference on the record
  (`packages/provenance/src/protocol.ts`). Storage should match authorship.

Quality bar:

- A reviewer reads one line and knows everything that record claims.
- A merge conflict is always about one record, in that record's shard.
- A typed declaration can state a record completely, with no side table.

Exit criterion: every relation has exactly one owning record. The global edges shard holds
only relations with no owner, or it is empty.

## 2. What the evidence says today

These facts come only from the partition I was given.

- `add_source_reference` writes one fact twice: `requirement.source_refs` with the clause,
  and a `references` edge without it (`state_store/writers.rs`). The live scope has 79
  `source_refs` entries and 76 `references` edges. The copies have drifted.
- `write_resolution` turns `requirement_id` into two edges, `needs` and `resolves`, and
  drops it. `Resolution` has no `requirement_id` field (`rule_writers.rs`). The line does
  not say what it resolves. Live counts: 97 `needs`, 95 `resolves`. Drifted too.
- `write_rule` turns `requirement_id` and `resolution_id` into `produces` edges and drops
  both. The rule line does not say what it refines.
- Every edge row carries `label: None` and nothing else. No `declared_by`, `retired`,
  clause, or date. The id derives from type and endpoints (`Edge::stable_id`).
- No `supersedes` or `depends_on` edge exists. Supersession is already a field.
- `ensure_edge_endpoint_exists` refuses `Domain` and `Boundary`. Those kinds are related
  only by fields today, and nothing is missing for them.
- The typed reconciler derives `references` and `produces` edges from the document and
  deletes stale ones, for those two types only (`typed_specs/relationships.rs`). The other
  seven types get no lifecycle.
- The merge gate sees one file. It checks edges and deserializes requirements and rules.
  Sources, resolutions, boundaries, topics, and questions merge unchecked
  (`merge/validation.rs`).
- The two binding families are relation records done right: scoped shard, `declared_by`,
  `retired`, derived id, retire-in-place (ADR 0005, ADR 0006). They are not edges.

## 3. The shapes

Rule: the record that makes the claim carries the reference. The reference is an id, or an
object with an id plus the metadata that only this claim has. Reverse lookups are derived.

Requirement cites source. Owner: the requirement. The clause is part of its claim. Already
the shape. The `references` edge is deleted.

```json
{"id":"req_edge_writes_validated","statement":"...","source_refs":[{"source_id":"source_docs_provenance_80bfa00","clause":"State format, edges"}]}
```

Requirement refines requirement. Owner: the child. It is the line being authored when the
relation appears, and the parent line stays untouched. `refines_into` is deleted.

```json
{"id":"req_expiry_configuration","refines":"req_share_link_expiry","statement":"..."}
```

Boundary belongs to requirement. Owner: the boundary. Already the shape.
Boundary cites source. Owner: the boundary. Already the shape, as `source_ref`.

```json
{"id":"boundary_no_proprietary_ste_replacement","requirement_id":"req_ste_syntax_checked_before_graph_write","source_ref":{"source_id":"source_asd_ste_faq","clause":"FAQ 3"},"statement":"..."}
```

Question belongs to topic, topic belongs to requirement. Owners: the question and the topic.
Already the shape. Both keep `links`. A question that waits on another owns `blocked_by`.
`depends_on` is deleted.

```json
{"id":"question_ste_authoring_integration","topic_id":"topic_ste_authoring_and_graph_checks","requirement_id":"req_ste_syntax_checked_before_graph_write","blocked_by":["question_ste_deterministic_syntax_subset"],"resolution_method":"grill","status":"open","links":[]}
```

Resolution resolves requirement. Owner: the resolution. A decision lives in one place, its
resolution (`docs/shaping.md`). The `needs` and `resolves` pair collapses to one field.

```json
{"id":"res_share_link_ttl","requirement_id":"req_share_link_expiry","position":"7-day default","rationale":"..."}
```

Rule refines requirement and is produced by a resolution. Owner: the rule. The typed
declaration already says `requirement` on the rule. Both `produces` edges are deleted.

```json
{"id":"rule_share_link_expiry","requirement_ids":["req_share_link_expiry"],"resolution_id":"res_share_link_ttl","statement":"Share links MUST expire within 30 days"}
```

Resolution spawns requirement. Owner: the spawned requirement. The `spawns` edge is deleted.

```json
{"id":"req_expiry_configuration","spawned_by":"res_share_link_ttl","refines":"req_share_link_expiry"}
```

Supersession, for source, resolution, and requirement. Owner: the successor, the record being
authored. Today `superseded_by` sits on the old record, so a supersession edits a line that
was already reviewed. Flip it. `superseded_by` becomes derived.

```json
{"id":"source_policy_v3","supersedes":["source_policy_v2"],"name":"Privacy policy v3","source_type":"policy"}
```

Verification binding. Owner: the binding record. A test outside the graph makes the claim, so
no graph record can own it. It stays a record in its scoped shard, unchanged.

```json
{"id":"verification_binding_3f...","rule_id":"rule_share_link_expiry","key":"ttl_ceiling","method":"exhaustion","declared_by":"vitest","file":"tests/share.test.ts","retired":false}
```

Implementation binding. Same reasoning, unchanged. The scanner and the typed spec both
materialize it and can have different owners from the rule, so it is not the rule's claim.

Thread parent. Owner: the child thread, `parent_id`. Messages own `thread_id`. Records own
`origin_thread` and `origin_message` already. Threads stay outside the graph projection.

Ideation target. Owner: the proposal (`target`), the assertion (proposal id), the
disposition (`canonical_artifact`, `external_action`). All embedded already. Unchanged.

What remains an edge: `contradicts`. It is symmetric. Neither requirement claims it. The
frontier computes from it. It stays in the edges shard. After migration the shard holds one
row in the live scope.

## 4. The three traversals

Canonical state stores only forward fields. The cache derives one adjacency relation, both
directions, at materialize time. Call it `refs`: `(from_kind, from_id, relation, to_kind,
to_id, meta)`. It is PR 173's read layer fed from fields instead of rows. The SDK
`NeighborsRequest` with `direction: "in"` is unchanged.

Everything requirement X touches to depth 2.

```
frontier = {X}
for depth in 1..=2:
  out = refs where from_id in frontier          # fields on X: source_refs, refines, spawned_by, domain_id
  in  = refs where to_id in frontier            # owners that name X: rules.requirement_ids, resolutions.requirement_id,
                                                #   boundaries.requirement_id, topics.requirement_id, children.refines
  frontier = out.to ∪ in.from, tagged with depth
```

All sources cited by requirements under domain D.

```
reqs    = requirements where domain_id = D
sources = flatten(reqs.source_refs).source_id, with clause kept per pair
```

No reverse lookup is needed. The owner side is the side the query starts from.

The supersession chain of source S.

```
forward:  follow S.supersedes repeatedly           # older records
backward: refs where relation = supersedes and to_id = S, repeatedly   # newer records
```

Backward uses the derived index, or a scan of the 52-line sources shard. `provenance prime`
already does reverse scans of this size for "no requirement references this source".

## 5. Migration from today's families

Staged. Three independently mergeable steps. Each step leaves `provenance check` green.

Step 1, stop the double writes. No schema change.
- `add_source_reference` writes `source_refs` only. The typed reconciler stops emitting
  `references` edges. A human reviews the diff between 79 refs and 76 edges, then backfill.
- `write_resolution` stores `requirement_id`. `write_rule` stores `requirement_ids` and
  `resolution_id`. Both stop writing edges. Backfill from the 97 `needs`, 95 `resolves`, and
  323 `produces` rows. Report every pair where `needs` and `resolves` disagree.

Step 2, move the remaining owned relations.
- Add `refines` and `spawned_by` on requirements, `blocked_by` on questions, `supersedes` on
  sources, resolutions, and requirements. All optional. Backfill from 19 `refines_into`,
  1 `spawns`, and the `superseded_by` fields. Keep `superseded_by` readable one release.
- `neighbors`, `trace`, and gaps read `refs`, not the edges shard.

Step 3, delete the derived edge rows.
- Delete every edge whose type is not `contradicts`. The endpoint table shrinks to one row.
- State schema stays at 1. Every new field is optional and omitted when absent, the rule
  `docs/state-format.md` already states. The graph reference projection changes because the
  `edges` family shrinks. That is a digest change for every scope. Bump the projection.

Merge validator (`merge/validation.rs`):
- `ShardFamily` recognizes and deserializes sources, resolutions, boundaries, topics, and
  questions. Today they merge unchecked. Once references live on them, an unchecked merge
  can land a malformed reference. Dangling ids stay with `provenance check`, because a
  merge driver sees one file. The edges check stays, for `contradicts`.

Typed spec:
- `desired_references`, `desired_produces`, and `remove_superseded_edges` go away. The
  reconciler compares `source_refs` and `requirement_ids` as fields. It already does this
  for `source_refs` (`typed_specs/reconcile.rs`). ADR 0008 adoption becomes field equality.
- `RuleDeclaration.requirement` already exists on the wire. No protocol bump for step 1.
  Adding `refines` and `supersedes` to declarations is a later bump.

## 6. What breaks or gets worse

Relation-level metadata and lifecycle. Today an edge cannot be retired, because it has no
`retired` field and no owner. Under this proposal a retired reference is an object on the
owner with `retired: true`. That edits a reviewed line. A relation that needs its own
lifecycle gets what the bindings have: a scoped record family with `declared_by` and
`retired`. That is a record, not an edge in a global shard.

Citing a relation as evidence. There is no edge id to cite. A citation becomes the triple
(owner id, field, target id). `canonical_artifact` names a type and an id, so a relation
cannot be a canonical artifact. It cannot today either. This gets no better.

Reverse traversal cost. Reverse lookups need the derived index or a shard scan. At 68
requirements and 167 rules the scan is free. The cost lands in SaaS, section 7.

The closed vocabulary. PR 173 closed it at the read layer. Here the vocabulary is the set of
typed fields. Adding a relation touches a struct, its serde shape, the merge deserializer,
and the read-layer mapping. More places than an enum variant. Harder to get wrong, because
a `StableId` field cannot hold an unknown kind.

Cross-scope relations. Every reference lives inside one scope's shard. A reference from
scope A to scope B has no home. The global edges shard could hold one today, though
`add_edge` takes a single `scope_id`.

## 7. SaaS mapping

Postgres with concurrent writers.
- One table per record kind. Single references are FK columns
  (`resolutions.requirement_id`, `requirements.refines`). Multi-valued references with
  metadata are child tables keyed by the owner:
  `requirement_source_refs(requirement_id, source_id, clause)`, primary key on the pair.
- A write locks the owner row. Two writers citing one source from two requirements never
  contend. Two writers changing one requirement's citations contend on that requirement.
  That is the conflict the JSONL merge has today.
- Reverse lookups are an index on the target column. The `refs` view is a `UNION ALL` over
  child tables and FK columns. No trigger keeps two copies in sync. There is one copy.
- `contradicts` is one table with a symmetric-pair constraint.

Property graph.
- Each owner field becomes an edge from owner to target. The field name is the label and the
  embedded object is the property bag. The edge id derives from (owner, field, target), as
  `Edge::stable_id` does today.
- Direction is fixed by ownership. A graph database traverses either way at equal cost, so
  the reverse lookup penalty disappears.
- The graph is a projection of the record store, and the record store stays canonical. The
  authorship story is the same in git and in SaaS.

## 8. Unsupported speculation

- `contradicts` could become a record. A contradiction is a finding, and the frontier turns
  it into a question. A Question with `contradiction: [a, b]` would own it and the edges
  shard would be empty. I have no evidence on how the frontier code consumes it.
- Thread parent. I did not read the thread writers. `parent_id` is by analogy with
  `origin_thread`.
- The Postgres contention claims are reasoning, not measurement.
- I did not read `relations.rs`, PR 173's rationale, or the research notes. The sibling
  stances may hold evidence for relation lifecycle needs this proposal underweights.

## 9. Uncertainty

Medium.

For the core claim, low. The evidence that today's edges are derived is direct: every edge
carries no metadata, three edge types are double-written and have already drifted, two edge
types do not exist, two record kinds are already field-only, and the typed SDK authors
references on the record. Nothing in the partition uses an edge as anything but a lookup.

For the whole design, medium. I did not read the material that argues for relation-level
lifecycle, and the SaaS section assumes a record store stays canonical in front of any
graph database. If the product commits to a graph database as the authority, the
"one copy" argument weakens and the sibling stance gains ground.
