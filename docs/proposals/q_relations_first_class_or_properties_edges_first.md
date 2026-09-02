# Relations are evidence: every non-containment connection is an edge record

Stance artifact for `q_relations_first_class_or_properties` (topic `topic_relation_shapes`).
Disposable. Written to be argued with, not merged.

## 1. Manifesto

A connection between two records is a claim. "This requirement cites that source" can be
wrong, can be reviewed, and can be replaced. A claim needs an identity, a place for its
metadata, and a lifecycle. A field on one endpoint gives it none of those.

The repository already believes this three times over.

- ADR 0001 forbids authoring `superseded_by` or `duplicate_of` on a proposal. Only a
  disposition record carries that authority. The link is a verdict, and a verdict is a record.
- ADR 0003 makes a verification binding "a first-class canonical relationship" with its own
  identity, updateable facts, and a `retired` flag. That is an edge record in all but name.
- ADR 0007 writes one review record per affected Rule, naming the Requirement, the field, the
  before and after, and when the change landed. The record is the authority.

Values. One fact has one form. Metadata about a connection lives on the connection. A
connection can be superseded without rewriting either endpoint.

Rule. Everything that is not pure containment is a typed edge record. Containment is a parent
field on the child. A question belongs to a topic. A boundary belongs to a requirement. A
requirement belongs to a domain. A thread belongs to its parent node. A proposal is filed under
its target. Nothing else is a field.

Quality bar. Every traversal has one query shape. Every relation can be cited, reviewed, and
superseded on its own line.

Exit criterion. Stop when each of today's 21 declared relation kinds has exactly one storage
form and no fact is stored twice.

## 2. The shapes

Today's edge row is `schema_version, scope_id, id, edge_type, from_type, from_id, to_type,
to_id, label`. The 612 rows in `edges-00.jsonl` use `label` zero times. There is no status, no
metadata, and no author. That is why `clause` had to live in `requirement.source_refs` instead of
on the `references` edge, and why one citation is stored twice (62 requirements carry embedded
refs, 76 `references` edges exist for the same facts).

The edge record schema, shown once. Existing column names stay so the 612 rows stay valid.

```json
{"schema_version":1,"scope_id":"default",
 "id":"references_source_source_codebase_provenance_3fb49a3_to_requirement_req_asserted_evidence_preserved",
 "edge_type":"references",
 "from_type":"source","from_id":"source_codebase_provenance_3fb49a3",
 "to_type":"requirement","to_id":"req_asserted_evidence_preserved",
 "metadata":{"clause":"immutable proposal lifecycle evidence freeze"},
 "status":"active","asserted_by":"ben","asserted_at":"2026-09-01T10:04:00Z","superseded_by":null}
```

`id` stays derived from type and endpoints, as `Edge::stable_id` does today. Two branches that
assert the same claim produce the same line. `status` is one of `proposed`, `active`,
`superseded`, `retired`. `superseded_by` names the replacing edge id. That is the one field an
edge may hold about another edge, because it is lifecycle, not a graph relation. New fields
default when absent, so old rows read unchanged and the family digest does not move.

A containment field is a plain id on the child. It is required, never optional, and it never
carries metadata. `{"id": "question_x", "topic_id": "topic_y", ...}`.

Requirement cites source. One `references` edge, clause on the edge, `source_refs` deleted. The
current direction stays because 76 rows and the endpoint table agree on it.

```json
{"edge_type":"references","from_type":"source","from_id":"source_a","to_type":"requirement","to_id":"req_b","metadata":{"clause":"section 3"},"status":"active"}
```

Requirement refines requirement. Unchanged.

```json
{"edge_type":"refines_into","from_type":"requirement","from_id":"req_parent","to_type":"requirement","to_id":"req_child","status":"active"}
```

Boundary belongs to requirement. Containment. Field stays.

```json
{"id":"boundary_no_proprietary_ste_replacement","requirement_id":"req_ste_syntax_checked_before_graph_write","statement":"..."}
```

Boundary cites source. `boundary.source_ref` becomes a `references` edge. The endpoint table
gains source to boundary.

```json
{"edge_type":"references","from_type":"source","from_id":"source_ste_issue_9","to_type":"boundary","to_id":"boundary_no_proprietary_ste_replacement","metadata":{"clause":"1.1"},"status":"active"}
```

Question belongs to topic. Containment. `question.requirement_id` is derivable through the
topic and is dropped as a stored field.

```json
{"id":"question_ste_authoring_integration","topic_id":"topic_ste_authoring_and_graph_checks","status":"open"}
```

Supersession, three kinds, one edge type. `source.superseded_by` and
`resolution.superseded_by` become `supersedes` edges. Requirement supersession already is one.
The endpoint table gains source to source and resolution to resolution.

```json
{"edge_type":"supersedes","from_type":"source","from_id":"source_v2","to_type":"source","to_id":"source_v1","status":"active","asserted_by":"ben"}
{"edge_type":"supersedes","from_type":"resolution","from_id":"res_new","to_type":"resolution","to_id":"res_old","status":"active"}
{"edge_type":"supersedes","from_type":"requirement","from_id":"req_new","to_type":"requirement","to_id":"req_old","status":"active"}
```

Question settled by resolution. `question.resolution_id` becomes a `resolves` edge, resolution
to question. Same verb the graph already uses for requirements.

```json
{"edge_type":"resolves","from_type":"resolution","from_id":"res_x","to_type":"question","to_id":"question_y","status":"active"}
```

Verification binding. Already a relationship record under ADR 0003. It keeps its own family
because its `to` side is a code site, not a graph node, and ADR 0003 refuses a Test node. It
adopts the same envelope so one reader shape covers it.

```json
{"edge_type":"verified_by","from_type":"rule","from_id":"rule_graph_gaps","to_type":"site","to_id":"provenance-store:gaps:exhaustion","metadata":{"method":"exhaustion","file":"crates/provenance-store/src/cache/gaps/tests.rs","symbol":"..."},"status":"active","asserted_by":"provenance-store"}
```

Thread parent. Containment. Field stays, exactly as `ThreadParent` is today.

```json
{"id":"thr_requirement_req_x_0","parent":{"node_type":"requirement","node_id":"req_x"},"status":"open"}
```

Ideation target. Containment. A proposal is filed under its target and is immutable under ADR
0001. Its `source_ids` stay embedded because the card is a frozen snapshot outside the canonical
graph, which ADR 0001 keeps out of graph projection. This is the one deliberate exception.

```json
{"id":"prop_agents_md_ste_onboarding_v2","traceability":{"target":{"artifact_type":"resolution","artifact_id":"res_injection_block_is_lean_cli_only"},"source_ids":["source_fresh_project_ste_onboarding_2026_08_28"]}}
```

Artifact links on topics and questions become `links` edges to a linkable record.

```json
{"edge_type":"links","from_type":"question","from_id":"question_y","to_type":"resolution","to_id":"res_z","status":"active"}
```

## 3. Three traversals, one query shape

Every traversal is a scan of active edges plus a scan of one parent column. Nothing else.

Everything requirement X touches to depth 2.

```sql
WITH RECURSIVE reach(node_type, node_id, depth) AS (
  SELECT 'requirement', :x, 0
  UNION
  SELECT CASE WHEN e.from_id = r.node_id THEN e.to_type ELSE e.from_type END,
         CASE WHEN e.from_id = r.node_id THEN e.to_id ELSE e.from_id END, r.depth + 1
  FROM reach r JOIN edges e ON e.status = 'active'
   AND ((e.from_type, e.from_id) = (r.node_type, r.node_id)
     OR (e.to_type, e.to_id) = (r.node_type, r.node_id))
  WHERE r.depth < 2)
SELECT DISTINCT node_type, node_id FROM reach;
```

Parent fields join as one extra union arm per containment kind, or the projection emits them as
`contains` edges (section 6). Either way there is no third shape.

All sources cited by requirements under domain D.

```sql
SELECT DISTINCT e.from_id AS source_id, e.metadata->>'clause'
FROM requirements q
JOIN edges e ON e.edge_type = 'references' AND e.status = 'active'
            AND e.to_type = 'requirement' AND e.to_id = q.id
WHERE q.domain_id = :d;
```

Today this answer needs two arms, one over `source_refs` and one over the edge table, and the
gap policy carries both in `requirement_has_valid_source` and `source_is_referenced`.

The supersession chain of source S.

```sql
WITH RECURSIVE chain(id, depth) AS (
  SELECT :s, 0
  UNION ALL
  SELECT e.from_id, c.depth + 1 FROM chain c JOIN edges e
    ON e.edge_type = 'supersedes' AND e.status = 'active' AND e.to_type = 'source' AND e.to_id = c.id)
SELECT id, depth FROM chain ORDER BY depth;
```

The same query answers resolutions and requirements by changing one literal. In Rust the shape
is one `RelationSource::related` call per hop. `RelationDerivation` shrinks to `EdgeRow` and
`ParentField`. `EmbeddedCollection`, `embedded_related`, `same_fact_as`, and
`drop_duality_echoes` are deleted.

## 4. Migration from today's 19 families

No family is added. No family is removed. Four families lose fields. One gains columns.

What moves to edges.
- `requirement.source_refs` to `references` edges with `metadata.clause`. 62 requirements.
- `boundary.source_ref` to `references` source to boundary. 3 boundaries at most.
- `source.superseded_by` and `resolution.superseded_by` to `supersedes`.
- `question.resolution_id` to `resolves` resolution to question.
- `topic.links` and `question.links` to `links` edges.

What is deleted. The five fields above. `question.requirement_id`. The `EmbeddedCollection`
derivation. Six `RelationKind` variants fold into existing verbs, so the closed list goes from
21 to 15.

What stays. `boundary.requirement_id`, `topic.requirement_id`, `question.topic_id`,
`requirement.domain_id`, `thread.parent`, proposal traceability.

Ordering, staged. Each stage merges alone and leaves the graph valid.

1. Additive edge fields with serde defaults. No row changes. Digest stamps under migration 018
   stay put because absent defaults do not serialize.
2. Extend `validate_edge_endpoint` to the new endpoint pairs and add `links`. Extend the SQL
   `edges` table from migration 002 with `status`, `metadata`, `asserted_by`, `asserted_at`,
   `superseded_by`, plus an index on `(scope_id, edge_type, status)`.
3. Backfill. Write one `references` edge per embedded ref, copying `clause`. Write the other
   four field kinds as edges. Derived ids make the backfill idempotent.
4. Flip readers. Gap policy and `RecordFront` read edges only. Delete the embedded arms.
5. Stop the dual write and delete the fields. Bump the schema check that rejects them.
6. Shard edges per scope and per `from_type` file. `ProjectionFamily::is_scoped` for `Edges`
   flips to true. This is the one structural change and the one that needs its own PR.

Merge validator. `rule_record_merge` merges untyped JSON by id and re-validates against the
shard type. Nothing changes in the algorithm. Two more things now fall out of it. A relation
conflict shows up as a conflict on one edge line, with a status on each side, instead of hiding
inside a requirement's field diff. And a retirement on one branch against a supersession on
another is a normal same-id conflict that a human sees.

Typed spec. Declarations that emit `source_refs` emit `references` edges instead. Verification
bindings are already edge shaped and keep their identity rule. No new typed family.

## 5. What breaks or gets worse

PR review. A citation change is no longer next to the statement it supports. The reviewer opens
the requirement shard and the edges shard. The clause moves out of the sentence's neighborhood.
This is the real cost and I do not want to hide it.

Hot file. One global `edges-00.jsonl` with 612 rows becomes the file every branch touches, and
it grows by roughly 150 rows on backfill. Stage 6 exists because of this. Until it lands, every
concurrent shaping branch conflicts in one file. Derived ids soften it because identical claims
from two branches merge clean.

Status everywhere. Every reader must filter `status = 'active'`, the way `state_adapter` already
filters retired records. A forgotten filter shows a superseded citation as live.

Gap and wiki wording. "requirement has no source refs" becomes "no source references this
requirement". `dangling.rs` collapses from seven walkers to two, one over edges and one over
parent fields. `with_requirement` on question gaps must join through the topic.

Closed vocabulary. The read layer's 21 kinds become 15. That is a shrink, not a break, but every
exhaustive match on `RelationKind` is touched once.

Ideation stays an exception. A proposal's `source_ids` remain embedded. The rule "everything
non-containment is an edge" has one asterisk, and I put it there on purpose.

## 6. SaaS mapping

Postgres with concurrent writers. `edges(scope_id, id, edge_type, from_type, from_id, to_type,
to_id, metadata jsonb, status, asserted_by, asserted_at, superseded_by)`, primary key
`(scope_id, id)`. The derived id gives a natural unique claim key, so two writers asserting the
same relation collapse with `INSERT ... ON CONFLICT DO NOTHING`. Lifecycle moves are
compare-and-set: `UPDATE edges SET status = 'superseded', superseded_by = :new WHERE id = :old AND
status = 'active'`, and a zero-row update means someone got there first. Parent fields are
foreign key columns with `ON DELETE RESTRICT`. The endpoint table becomes a check constraint or
a trigger. Nothing here needs a lock wider than one row.

Property graph. Each record is a node. Each edge record is a relationship whose properties are
the metadata, status, and provenance fields, with the derived id stored as a property for
round-trip. Property graphs do not have fields as edges, so the projection emits one `CONTAINS`
relationship per parent field. The field remains canonical and the relationship is derived.
The depth-2 query in section 3 becomes a single variable-length pattern over active
relationships.

## 7. Unsupported speculation

Marked as such. None of this is grounded in the partition I read.

- The SaaS may never arrive. If it does not, stage 6 is the only stage with a hard payoff.
- Swarm-authored edges probably want `status = 'proposed'` with the PR merge as acceptance.
  ADR 0001 says swarm output cannot supply disposition authority, and this would extend that
  to relations. I did not check the swarm landing path.
- `question.requirement_id` may be load bearing in the shaping loop in ways the gap policy does
  not show. Dropping it may cost a join in a hot read.
- A graph database would make the parent-field asymmetry feel wrong and push toward edges for
  containment too. I am not proposing that today.

## 8. Uncertainty

Medium.

For. Three accepted ADRs already treat relations as records with identity and lifecycle. The
duality is measured, not hypothetical, and the read layer already carries dedupe code to paper
over it. The endpoint validator, the merge path, and the SQL cache are all edge shaped already,
so the migration reuses machinery rather than building it.

Against. The PR review cost is real and unmeasured. The global edges shard is a hot file today
and this makes it hotter until stage 6. I did not read the typed spec writers or the state
format document, so stage 5 and the typed spec paragraph are informed guesses. The ideation
exception weakens the claim that the rule has no asterisks.
