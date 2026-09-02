---
question: q_relations_first_class_or_properties
topic: topic_relation_shapes
anchor: req_implement_a_normalized_knowledge_graph_d
stance: declare, don't unify
date: 2026-09-03
status: tournament artifact, disposable
---

# Declare, don't unify

Central claim: the cost both siblings want to remove is not the storage shape. It is the hand-maintained lists that name the shapes. A record-kind registry removes those lists without touching a byte of canonical state. A shape change touches every byte and leaves the lists in place.

## 1. Manifesto

Values. A storage-shape change is the most expensive move this repository can make. It lands on every reader, every writer, every migration, the merge rules, both SDKs, and every SaaS plan at once. The read layer already made heterogeneous storage uniform. PR 173 declared 21 relation kinds with a derivation tag each, and the W4 plan puts one traversal core over them. The remaining cost is that 19 families are named by hand in at least seven places. That is the lever.

Quality bar. Every cost below names a file, a migration, or a review finding. Every sibling benefit is conceded with evidence or refuted with evidence.

Exit criterion. For each unification direction, show the work it forces and the problem it leaves. Show what the registry gives with no shape change. Stop.

## 2. The cost ledger

### What exists today, counted

- 19 projection families (`crates/provenance-store/src/cache/projection_families.rs:23-43`). 20 shard path functions (`crates/provenance-store/src/shards.rs`). 19 loader arms (`crates/provenance-store/src/cache/materialize/family_rows.rs:53-87`). 9 hand-written INSERT column lists (`crates/provenance-store/src/cache/materialize/graph_records.rs`). A 5-variant merge family map (`crates/provenance-store/src/merge/validation.rs:34-47`). A 19-row derivation table in `docs/cache.md`. 21 relation kinds in `crates/provenance-core/src/model/relations.rs:49-70`. 26 code sites name a family list.
- 20 migrations. 15 of them add, drop, or rename columns or tables for record families. 3 are stamp machinery (018 to 020). Reference fields arrived by migration three times: 007 (five FK columns), 008 (two `superseded_by`), 009 (`domain_id`).
- Canonical state holds 612 edges. 7 of the 9 edge types have rows. `depends_on` and `supersedes` have zero. Supersession lives in fields: 1 `superseded_by` value in use, plus `Supersedes` declared and unused.
- 67 commits touch `.provenance/state`. 45 touch `edges/edges-00.jsonl`. All 45 also touch a record shard. 0 commits touch edges alone. 19 touch records alone. Rules co-change with edges in 35 commits, requirements in 22, resolutions in 21.
- Citation duality. 62 requirements carry 79 `source_refs` entries. 76 `references` edges exist. 76 pairs are mirrored. 3 pairs exist only in `source_refs`. 0 exist only as edges. `add_source_reference` writes the field and the edge in two separate shard mutations (`crates/provenance-store/src/state_store/writers.rs:131-187`).
- `source_refs` has no SQL column. The requirements INSERT carries `scope_id, id, statement, status, domain_id, fog` (`graph_records.rs:46`). The served `SqlFront` cannot see the 3 unmirrored pairs.
- The repository already tried a relation-as-record family. `service_bindings(rule_id, service_id, binding_type)` was created in 009 and dropped in 016 and 017. 017 also deleted canonical shards. That is the precedent for what a shape retreat costs.

### Direction A: every non-containment reference becomes an edge row

Forced work.
- Move 9 FK fields and 3 embedded collections. `domain_id` is named in 45 files, `superseded_by` in 52, `ArtifactLink` in 19, `source_refs` in 33. Overlap exists, but the union is over 100 files.
- Canonical migration of 8 shards in every consuming repository. The precedent is 017. Add 67 `domain_id` values, 79 citations, 3 `resolution_id` values, and 1 boundary citation as new rows in the global edges shard.
- Drop columns from 6 projection tables and 6 indexes (007, 008, 009). Rewrite the gap policies that read those columns (`cache/gaps/frontier.rs:11`, `cache/gaps/dangling.rs:17-60`).
- SDK. `EdgeType` widens in `packages/provenance/src/protocol.ts:247-256` and in 49 Rust files. `CreateRequirementInput` and the four shaping inputs lose fields.
- Merge. Every edge lands in one family checked by `validate_merged_edges`. Today 45 of 67 commits touch the edges shard. After the move every requirement creation touches it too. The single global shard becomes the hotspot of every branch.

What it leaves unsolved.
- Required, single-valued references become existence checks. `question.topic_id` is `NOT NULL` in 007. As an edge, "every question has one topic" moves from the type to the gap report. The reviews show what happens to invariants that live only in a scan: the round-4 card defect was missed by the completeness gate (fable review, finding 8).
- The 19 family lists stay. Nothing in the edge move touches `shards.rs`, `family_rows.rs`, or `ShardFamily`.
- The vocabulary stays at 21. The derivation tag becomes uniform, which is the whole gain, and W4 already hides the tag from every operation.

### Direction B: every reference lives as a field on its owner

Forced work.
- Rewrite 612 edges into fields on 4 shards. `produces` (323 rows) needs an owner. Requirements and resolutions both produce rules, so a `produced_by` list on the rule is the only single owner.
- Choose an owner for `needs` and `resolves`. They are one fact stored twice today (97 and 95 rows). `contradicts` is symmetric and has no owner.
- Every edge `label` becomes a struct in a list. That is `source_refs`. The move reproduces the duality shape for every kind.
- `EdgeType` disappears from 49 Rust files, 6 `CreateEdgeInput` sites, and the TS protocol.

What it leaves unsolved.
- Merge checking gets worse. `ShardFamily::for_shard_path` checks Edges, Requirements, Rules, and landings only. Sources and resolutions are `Unrecognized` and "pass unchecked" (`merge/validation.rs:43-46`). A `resolves` list on a resolution would merge with no validation at all.
- Reverse traversal needs the projection anyway. "Which requirements produce rule R" is an index lookup today (`idx_edges_to`). With fields it is a scan or a derived table.
- The 19 family lists stay. The vocabulary stays at 21.

### Both directions share one hidden cost

The adversarial reviews measured what one hand mirror of reader behaviour cost. 17 of 19 family byte domains were hand-assembled. Both rejection-grade findings came from that mirror (GLM review, question 1c). The fix was to delete the declaration and hash scopes (ADR 0009 as reviewed). Neither sibling touches the remaining mirrors. Both add a migration to a repository whose migration count is already 20.

## 3. The record-kind registry

One declaration per kind. A Rust `const` table or a macro in `provenance-core`, one entry per family:

```
kind:        Requirement
shard:       scopes/<scope>/requirements/req.jsonl     (scoped)
table:       requirements
fields:      statement, status, fog, domain_id, source_refs, ...
references:
  domain_id    -> Domain      optional  column    RequirementInDomain
  source_refs  -> Source      many      embedded  RequirementCitesSource (label: clause)
merge:       typed (validate as Requirement)
```

Edges are one entry with `shard: edges/*.jsonl (global)` and nine references derived from `EdgeType`.

Derived from the table, not written by hand:
- `shards.rs` path functions (20 today).
- `ProjectionFamily::ALL` and the loader dispatch in `family_rows.rs:53-87`.
- The INSERT column lists in `graph_records.rs`, `collaboration_records.rs`, `integration_records.rs`.
- The expected SQL schema, checked against the applied migrations by a test, so a new field cannot exist without a column.
- `ShardFamily::for_shard_path` and its merge policy per kind. Sources and resolutions stop being `Unrecognized`.
- `RelationKind` with its derivation tag, target, direction, and cardinality. The node-struct completeness gate from PR 173 becomes a compile error instead of a scan.
- The `SqlFront` query fragment per relation kind for the W4 traversal core.
- The `docs/cache.md` derivation table and the TS `NodeType` and `EdgeType` unions.

Code paths replaced, by name: `shards::*_path`, `ProjectionFamily::ALL`, `family_rows::load_rows`, `ShardFamily::for_shard_path`, the per-kind `list_*` readers in `state_store.rs:148-200` (they become one generic reader keyed by kind), and the hand-written `declared_relations` list.

What it does not do. It does not move a byte of canonical state. It does not add a migration. It does not change a wire type. A registry entry that disagrees with a struct is a compile error, not a review finding.

## 4. Three traversals, today plus registry

Each traversal runs on the W4 core. The registry supplies the per-kind fragment. Storage shape is invisible to the caller.

**Everything requirement X touches to depth 2.**
Today: union of `edges` rows where `from_id = X` or `to_id = X` (indexes `idx_edges_from`, `idx_edges_to`), plus `boundaries.requirement_id = X`, `topics.requirement_id = X`, `questions.requirement_id = X`, plus `requirements.domain_id` of X. Repeat for each neighbour. Registry: the core iterates every `RelationKind` whose endpoint includes Requirement and runs the generated fragment. Adding a reference field adds a fragment automatically.

**All sources cited by requirements under domain D.**
Today: `requirements WHERE domain_id = D` joined to `edges WHERE edge_type = 'references' AND to_id = req.id`. This misses the 3 pairs that exist only in `source_refs`, because `source_refs` has no column. Registry: the same query, and the same miss. See section 5.

**The supersession chain of source S.**
Today: recursive CTE over `sources.superseded_by`. One column, one walk over 52 rows. Registry: `SourceSupersededBy` declares `column, optional, chain`, and the core emits the CTE. A field is the right shape for an optional single successor. Direction A would turn one column into a row lookup per hop for no gain.

## 5. Where the siblings are right

Conceded, with evidence.

1. The citation duality is a defect. 3 of 79 pairs are unmirrored. The writer commits two shards non-atomically. The union is hand-written twice (`cache/health.rs:79-99`, `cache/gaps/graph_query.rs:255-283`). A registry declares the duality but cannot delete it.
2. Embedded collections have no projection. `source_refs` and the two `links` lists are opaque to the `SqlFront`. A registry can generate a derived relation table at materialization, but that is a projection shape change, not a canonical one.
3. The global edges shard is a merge hotspot. 45 of 67 commits touch it. Direction B removes the hotspot. Direction A makes it worse.
4. Merge checking is uneven. Direction A gets uniform checking for free.

Minimal shape change that fixes only these. Retire `Requirement.source_refs`. Keep the `References` edge and carry the clause in the existing `Edge.label` (`crates/provenance-core/src/model/graph.rs:110-111`). Backfill the 3 missing edges. That touches 33 files and one canonical migration for one shard. Split `edges-00.jsonl` by edge type or by scope, which `read_edge_shards` already supports (`readers.rs:377-395`). That is two moves against one field and one file. It is not a unification.

## 6. SaaS mapping

A database with concurrent writers needs three things from the shapes. Each reference must be addressable by both endpoints. Each write must be validated against the referenced record in the same transaction. Each kind must have a stable identity for optimistic concurrency.

Edges give the first for free through two indexes. FK columns give it through one index per column, which 007 and 009 already create. Embedded collections give it only after unnesting. The registry can generate the unnest. The duality fails the second requirement today because two shard mutations are two transactions. A database fixes that with one transaction regardless of shape.

Heterogeneity does not hurt in the database. A relational schema holds FK columns and a join table side by side without cost. A property graph holds a single-valued reference as a property or as an edge and indexes both. What hurts is 19 hand-written mappings from canonical shape to table. That is the registry's job in both hosts.

## 7. Unsupported speculation

- A graph database backend would prefer Direction A. No graph backend exists and no query load is measured.
- A registry could generate the migrations themselves. The repo has no schema-diff tool and I did not design one.
- The 3 unmirrored citations came from hand edits. Commit `4a40de0` says "Hand-edited because the CLI has no resolutions update command", which suggests the pattern, but I did not trace the 3 pairs.
- Concurrent SaaS writers would contend on a single edges table. I have no write-rate estimate.

## 8. Uncertainty

Medium.

Low on the cost counts. Every number came from `jq`, `grep`, `git log`, and the migration directory in this checkout.

Medium on the registry's payoff. Both reviews warn that a declaration is itself a list to maintain. The GLM review says the completeness gate "compares the declaration to a second hand-written list". The registry earns its keep only if every consumer is generated from it and nothing else names a family. If one hand list survives, the registry is a third mirror.

High on the SaaS section. Nothing is measured. The claim that heterogeneity is free in a database is a schema argument, not a benchmark.
