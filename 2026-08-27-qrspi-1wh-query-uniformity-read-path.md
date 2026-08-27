---
date: 2026-08-27
bead: provenance-1wh
epic: provenance-46p
stage: structure-complete-awaiting-disposal
model: glm-5.3-flash-high
---

SUBJECT BEAD: provenance-1wh — Query uniformity across node kinds, field-attached records, and lifecycle tiers; decide the served read path. Epic provenance-46p.

=== QUESTION ===

**Sharp restatement.** Provenance answers reads through eight fixed, domain-shaped operations that each load an entire scope's canonical JSONL corpus into memory and traverse only validated `Edge` rows. Records connect in at least four structurally different ways, and only one of those ways is traversable by the served read path. The question: **should cross-tier reads be unified behind a single traversal story, and which artifact is authorized to answer them — canonical shards, a revision-stamped SQLite projection, or a split by state class?**

This is not "is uniformity nice." It is "what is the cost of non-uniformity once a machine consumer is the primary caller, and does the projection have to become a first-class read authority to pay that cost down, or does it merely have to stop being a write-only dead end?"

**Sub-questions.**

1. **Vocabulary.** Four target-type enums exist with different member sets. Does uniformity collapse them into one addressable-entity vocabulary, or freeze them as formally distinct?
2. **Connection-style parity.** Edges are validated and traversable. Field attachments and embedded references are neither at the read layer. Does a uniform layer promote FK/embedded links to first-class relations, or stay edge-only?
3. **Authority and freshness.** No served response names the state it reflects. Can responses carry a revision/digest stamp, and must reads refuse rather than silently serve stale?
4. **Boundedness and paging.** `limit`/`has_more` with no cursor means results beyond 200 are unreachable, not merely paginated. Must a uniform layer add continuation, forcing an order-stability promise?
5. **Work versus output bounding.** Answers are bounded; the work producing them is not.
6. **Tier inclusion.** Pinned-graph export excludes ideation; SQLite includes it. Which precedent governs the served path?
7. **Projected tool surface.** How many tools does each option project into a confined runtime's catalog?
8. **Rebuild/catch-up cost.**
9. **Concrete walk.** "Impact of a Rule change joined with stale evidence sites" — how many calls, and is the join expressible at all?

**In scope:** the served read path (`protocol/query.rs`, executors under `operations/queries/`); the four connection styles and four target-type vocabularies; read authority (canonical shards vs `provenance.db`); stamp-and-refuse semantics; tier inclusion measured against both precedents; per-option costs; projected tool counts; the concrete Rule-impact walk.

**Out of scope:** the write/transaction kernel (Change Set, plan/commit, approvals); choosing the revision representation (companion open decision #5, upstream); whether JSONL remains canonical storage (open decision #12); designing an MCP server (none exists); search ranking quality; retirement semantics; schema migration; scope model changes; the Proposal naming conflict.

**Decisions the answer must enable the reviewer to make:** (D1) edge-only vs promoted relations; (D2) vocabulary convergence vs frozen disjointness; (D3) whether SQLite becomes a read authority, a non-authority, or serves a declared subset; (D4) response stamps — refuse or annotate; (D5) cursor continuation and its order-stability prerequisite; (D6) lifecycle-tier inclusion; (D7) operation count growth, shrinkage, or cap.

=== RESEARCH ===

**Anchor correction.** `docs/research/2026-08-27-data-model-and-erd.md` does not exist — not in the worktree (`docs/research/` holds four files) nor in any branch (`git log --all -- 'docs/research/*erd*'` is empty). No claim below rests on its figures. Bead ids `provenance-1wh`/`provenance-46p` appear nowhere in the worktree either (`.beads/` contains only config/hooks/metadata; nothing matches), so they are taken from tasking, not verified here.

---

**§1. The eight operations and what their contract carries.**

Requests live in `crates/provenance-core/src/protocol/query.rs`: `GetQuery` (42–49), `SearchQuery` (54–64), `NeighborsQuery` (69–83), `TraceQuery` (88–104), `ImpactQuery` (109–119), `EvidenceQuery` (128–140), `StaleQuery` (145–157), `ResolveSymbolQuery` (162–174). All are `#[serde(deny_unknown_fields)]` with optional `protocol_version`. Bounds in `protocol.rs`: version 5 (25), `QUERY_DEFAULT_LIMIT = 50` (28), `QUERY_MAX_LIMIT = 200` (31), `TRACE_DEFAULT_MAX_DEPTH = 3` (34), `TRACE_MAX_DEPTH = 10` (37); guards at 54–81; `take_page` truncates and reports `has_more` (84–88).

The envelope `QueryResponse` (`response.rs` 16–33) carries exactly `protocol_version`, `operation`, and the flattened result — **no field naming the state the answer reflects**: no revision, digest, commit, or timestamp.

Every multi-record result has `has_more` (`response.rs` 43–116), and **there is no cursor, offset, page token, or continuation anywhere** — grep across `crates/provenance-core/src/protocol/`, `crates/provenance-store/src/operations/`, `packages/provenance/src/` returns zero matches. So `has_more: true` is terminal: a caller can raise `limit` to 200 and no further; a 201-item match set has no expressible query.

`docs/cli.md` 72–132 documents the surface; lines 76–77 state the intent: *"so nothing here is a query language and nothing needs a daemon."* The CLI mirrors the eight one-to-one as subcommands (`crates/provenance-cli/src/cli/sdk.rs` 78–113).

**§1a. Two projections of the protocol already exist — and they disagree in shape.** The CLI projects eight distinct subcommands; the TypeScript SDK reaches the same operations through **one generic passthrough** — `packages/provenance/src/engine.ts:53`: `const args = ["sdk", command]` — carrying only the typed *result* shapes in `protocol.ts`. Its own test drives the real binary (`query.test.ts` imports `execFileSync` and `../../../target/debug/provenance`, lines 2 and 24). Measured precedent: eight named operations can be served from a single dispatch surface, with domain knowledge carried by parameters rather than endpoints.

---

**§2. Four target-type vocabularies, not one.**

| Enum | Location | Members |
|---|---|---|
| `NodeType` | `model/graph.rs` 7–20 | source, requirement, resolution, rule, topic, question (**6**) |
| `IdeationTargetType` | `model/ideation.rs` 14–29 | the six **+ domain** (**7**) |
| `ArtifactLinkTargetType` | `model/shaping.rs` 8–17 | source, requirement, resolution, rule (**4**) |
| `CanonicalArtifactType` | `model/ideation.rs` 49–58 | source, requirement, resolution, rule (**4**) |

`GraphNode` (`protocol/node.rs` 18–25) mirrors `NodeType`. **Domain and Boundary are unaddressable by every served operation**, since `get` requires a `NodeType` (`query.rs` 45). Three artifacts disagree about what a graph node is: the pinned-graph export carries domains and boundaries (`graph_reference/projection.rs` 36, 38); SQLite's table list includes `"domains"` and `"boundaries"` (`cache/materialize.rs` 47–62); the served path excludes both.

---

**§3. Connection styles — there are four, not three.**

**(1) Validated edges.** Endpoint table `edge_validation.rs` 20–33 under `#[rule("rule_prov_edge_endpoint_table")]` (14). `Produces` accepts Resolution→Rule or Requirement→Rule (30–32). Rule is a leaf: oracle restatement 90–92, exhaustive proofs 96–105 and 109–123. Topic/Question refused as endpoints for every type — asserted at 164–174, guaranteed by absence from the match arms.

**(2) Foreign-key attachments.** `Boundary.requirement_id` (`shaping.rs` 118), `Topic.requirement_id` (130), `Question.topic_id` (147), `Question.requirement_id` (149), `Question.resolution_id` (163–168), `Requirement.domain_id` (`artifacts.rs` 302–303). Write-time parent checks: `state_store/writers.rs` 78 ("domain does not exist"), 125/162 ("requirement does not exist"), 151 ("source does not exist"), 311 ("{side} endpoint does not exist").

**(3) Embedded references.** `Requirement.source_refs: Vec<SourceReference>` (`artifacts.rs` 304–305; struct 276–281). `IdeationTarget { artifact_type, artifact_id }` (`ideation.rs` 274–281) embedded in proposals (`ideation/proposals.rs` 12), synthesis packets (`synthesis.rs` 91), contributions (`contributions.rs` 70).

**(4) `ArtifactLink` — absent from the brief's framing.** `shaping.rs` 104–110, appearing as `Topic.links` (138) and `Question.links` (162): a second embedded-reference style with its own four-member vocabulary, attached to exactly the two kinds edges cannot reach.

**Drift check on source_refs.** `writers.rs` 131–187 writes the embedded ref and the `References` edge together inside one `with_repository_publication` call (131–133) — **on this path they cannot drift**; but no reconciler or invariant check guards state arriving by import or hand edit.

---

**§4. Existence validation is asymmetric.**

`canonical_artifacts.rs` builds an index whose key covers only the four canonical kinds (78–89) and offers `ensure_exists` (53–66). Callers: `proposal_writers.rs` 363, `ideation_batches.rs` 119/232 — all passing `disposition.canonical_artifact`. **Nothing validates that an `IdeationTarget.artifact_id` resolves**, while styles (1) and (2) validate at write. Style (3) admits dangling references by construction.

---

**§5. A second traversal layer already exists, unserved.**

`cache/gaps/graph_query.rs` defines `GraphQuery` over `GapGraph` (51–53), documented at 46–50 as *"the single home for the traversals the wiki assembler needs too."* Method surface (60–261) is bespoke joins: `resolving_resolutions` (122), `produced_rules_for_requirement` (150), `producing_requirements` (192), `missing_rule_producers` (228), `rule_trace_reaches_source` (241), `requirement_has_valid_source` (247), `source_is_referenced` (261). Its loader `cache/gaps/state_adapter.rs::GraphRecords::load` (35–42) reads canonical `StateStore` — **not SQLite despite living under `cache/`** — and at 65–69 derives topic retirement by following `topic.requirement_id`, i.e. **FK-style traversal already exists in production code**. Likewise `cache/health.rs::graph_evidence_locked` unions embedded `source_refs` (79–83) with edge-derived citations (88–90). Cross-style traversal exists twice; it is bespoke, duplicated, and exposed through zero of the eight operations.

---

**§6. Execution cost: answers bounded, work unbounded.**

`operations/queries.rs::open` (26–30) constructs a `StateStore` with **no lock**. `with_repository_publication` appears 53 times in provenance-store, none in `operations/`; `find_gaps` locks (`state_adapter.rs` 10), `graph_evidence` locks (`health.rs` 65). So `get/search/neighbors/trace/impact/resolve-symbol` can observe torn cross-shard state mid-write; `stale` cannot (it locks transitively at `stale.rs` 36 → `health.rs` 65).

`records.rs::load` (12–61) loads and sorts every record kind in the scope; called unconditionally by `get` (80), `search` (103), `neighbors` (`walk.rs` 72), `trace` (`walk.rs` 107), `impact` (`impact.rs` 26), and `resolve-symbol` (`symbols.rs` 53). **A get-by-ID pays whole-corpus load plus sort; `find` is linear** (63–71).

`trace` nests frontier × all edges per depth (`walk.rs` 115–128) and breaks when `reached.len() > request.limit` (135–137) — **mid-depth truncation** with no resume point.

`impact` walks exactly `TRACE_MAX_DEPTH` unconditionally (`impact.rs` 34), ignores edge-type filters, then full-scans the repository source tree on every call (`impact.rs` 65, `scan_path(repo)`).

**Resolve-symbol likewise scans the working tree** (`symbols.rs` 29) and *unions* scanned sites with canonical implementation and verification bindings (31–52) — a cross-source join served today. `evidence` composes five families in one call — implementations (26–31), verifications (32–37), verification runs sorted newest-first (38–50, from cache JSONL), open reviews raising `review_required` (51–58), and optionally the git-diff `stale` computation (64–70 → `stale.rs` 30–44). So: **three of eight operations depend on live filesystem scans; two on git history.** Those cannot be served from any projection of canonical state alone.

Paging composition bug class, evidenced: `evidence.rs` applies independent `take_page`s to four separate collections (60–63) and computes one top-level `has_more` (its return usage), while each sub-collection may itself have been cut at `limit + 1` — the reader cannot tell which collection truncated.

---

**§7. SQLite is write-only today.**

`docs/cache.md` 3–9: rebuildable, *"never the source of truth"* (5), validated under the aggregate validator (6–7), copied under the publication lock (7–8). **No revision/stamp/freshness concept exists in the document or the module** — grep over `crates/provenance-store/src/cache/` excluding tests returns zero hits for `revision|stamp|generation|digest|freshness`.

`materialize_state` (`materialize.rs` 19–43): snapshot (20), validate scopes (23–25), migrate (27), begin tx (28), `clear_cache` (29), reload all scopes and edges (32–36), commit. `clear_cache` DELETEs sixteen tables (45–68). **Rebuild is total; no incremental catch-up exists.**

**Nothing reads it back.** Every repo-wide `SELECT` lives in `cache/tests/materialization_behavior.rs` or in `migrations.rs` (95, 157, 200, 207 — all `_schema_migrations`/`sqlite_master` bookkeeping). Indexes are thin: `migrations/005_report_indexes.sql` defines four (two edge indexes `(scope_id, edge_type, from_type/to_type, from_id/to_id)`); seventeen migrations total. Yet coverage is broadest of any artifact: tables span graph, shaping, domains/boundaries, **and** ideation/collaboration (`materialize.rs` 56–62).

---

**§8. The exclusion precedent and an existing digest.**

`graph_reference/projection.rs` 11–28 is the precedent, semantically motivated:

> 14–17: `collaboration and ideation records ... say who was talking and what they were still arguing about; they are not what the graph asserts, and they never enter the projection.`
> 19–23: `This field list is the rule, not a check laid over it ... a family with no field here cannot leave (`load_projection` has nowhere to put it) and cannot come back (`deny_unknown_fields` refuses a document that names it).`

Carried by `#[rule("rule_pinned_graph_families")]` (29) and `deny_unknown_fields` (31); `strip_collaboration_fields` (208–210, applied at 93). Note `GraphExport` still carries Domain and Boundary (36, 38).

A self-verifying digest machinery exists: `canonical_bytes`/`sha256` over sorted-key JSON (`canonical.rs` 13–56), `graph_digest` defined once (`export.rs` 24–32), offline verification narrative (34–56). It digests `GraphExport` bytes — valid precisely because the export *is* the records.

---

**§9. The companion doc.**

146–148: structured graph queries *"can remain a separate read interface"*; a REPL composes read and transaction interfaces *"without adding a new command or query language."*
164–165 (invariants 11–12): *"Query projections expose the graph revision that they represent. / A stale projection never returns results without a visible stale condition."*
State classes table (256–264) verbatim:

```
| Graph intent | Sources, Requirements, Rules, Resolutions, Boundaries, Domains, authored edges | Planned Change; approval policy can depend on scope and risk |
| Working state | Fog, claims, open Questions and Topics, shaping threads | Revision-checked transaction; usually no approval ceremony |
| Immutable audit | current Proposals, Assertions, Dispositions, landings | Lifecycle-specific append operation; never rewritten |
| Engine-derived durable state | retirement markers, evidence review records | Engine writes as a consequence of a transaction |
| Volatile evidence | verification runs | Cache; never canonical graph state |
| Pure projection | SQLite, wiki, coverage and frontier reports | Rebuildable and revision-stamped |
| Code-owned state | scanned bindings and declarations owned by an integration | Owner reconciliation through the same transaction kernel |
```

Note: Boundaries/Domains are Graph intent (258) and Topics/Questions Working state (259) — **the opposite cut to `NodeType`** (§2).
319–341: *"The SDK query operations read the state store directly, not SQLite. There is no shared graph revision..."* (322–323); comparison-before-serving (333–334); four remedies — *"catch up, rebuild, use a canonical read path, or return a typed stale-projection refusal"* (336); watcher ≠ correctness (337–338).
Non-goals: no query language/DSL (424); *"Do not treat SQLite or another rebuildable projection as canonical without a separate decision"* (429). Open decision #5: serial, digest, or both (442). Repository-evidence note cites the canonical graph digest precedent (530–531).

---

**§10. Actively sought contradicting evidence.**

- **Against a generic layer:** non-goal 424 forbids a query language; 146–148 endorses keeping the eight named operations; `cli.md` 76–77 repeats it. A too-generic relation API risks becoming the forbidden thing.
- **Against "FK/embedded traversal doesn't exist":** it exists twice (`state_adapter.rs` 65–69; `health.rs` 79–90). The gap is exposure and duplication, not capability.
- **Against "source_refs drift from edges":** sole write path couples them under one lock (`writers.rs` 131–186).
- **Against "SQLite is nearly a read path":** zero readers, four indexes, delete-all rebuild.
- **Against "eight operations = canonical-only":** three need live filesystem scans, two need git (`impact.rs` 65, `symbols.rs` 29, `stale.rs` 41–43; `evidence.rs` 64–70). Any projection-based candidate must special-case them anyway.
- **Against the brief's three-style framing:** `ArtifactLink` is a fourth style (§3-4).
- **Against tool-count reasoning as a hard input:** no MCP/tool-tree/catalog exists anywhere in the repo (grep, zero matches); the only measured precedent is one-generic-dispatch vs eight-subcommands (§1a).

**Evidence-split checklist.**

*Repository facts (directly verified, path + lines above):* (1) eight request shapes, bounds, guards (`query.rs` 42–174; `protocol.rs` 25–88); (2) envelope carries no state stamp (`response.rs` 16–33); (3) no cursor anywhere (grep zero); (4) `NodeType` = 6 without Domain/Boundary (`graph.rs` 7–20; `node.rs` 18–25); (5) four disjoint target-type enums (`graph.rs` 7–20; `ideation.rs` 14–29, 49–58; `shaping.rs` 8–17); (6) endpoint table + rule-leaf proofs (`edge_validation.rs` 20–33, 90–123, 164–174); (7) FK fields and write checks (`shaping.rs` 118/130/147/149/168; `artifacts.rs` 303; `writers.rs` 78–311); (8) fourth style `ArtifactLink` (`shaping.rs` 104–110, 138, 162); (9) `IdeationTarget` unvalidated against a 4-kind index (`canonical_artifacts.rs` 53–89; callers `proposal_writers.rs` 363, `ideation_batches.rs` 119/232); (10) coupled source_ref+edge write under lock (`writers.rs` 131–186); (11) second unserved traversal layer incl. FK traversal (`gaps/graph_query.rs` 46–261; `gaps/state_adapter.rs` 35–69); (12) embedded∪edge union in production (`health.rs` 79–90); (13) query path takes no publication lock; gaps/evidence paths do (grep 53 hits, none in `operations/`; `state_adapter.rs` 10; `health.rs` 65); (14) whole-corpus load per query (`records.rs` 12–61 and six call sites); (15) trace truncates mid-depth (`walk.rs` 135–137); (16) impact depth-10 unconditional + full repo scan (`impact.rs` 34, 65); (17) resolve-symbol scan ∪ bindings (`symbols.rs` 26–52); (18) evidence composes five families with per-collection paging (`evidence.rs` 26–70); (19) stale = git range + locked evidence load (`stale.rs` 30–44); (20) SQLite never truth, no revision concept (`docs/cache.md` 3–9; grep zero); (21) total DELETE+reload of 16 tables (`materialize.rs` 19–68); (22) zero production readers of SQLite (SELECT audit); (23) four indexes / 17 migrations (`005_report_indexes.sql`); (24) SQLite's tier set is broadest incl. ideation (`materialize.rs` 47–62); (25) pinned-graph structural exclusion, semantic rationale (`projection.rs` 11–48, 93, 208–210); (26) existing canonical digest (`canonical.rs` 13–56; `export.rs` 24–56); (27) companion doc quotes as cited (146–148, 164–165, 256–264, 319–341, 424, 429, 442, 530–531); (28) ERD anchor and MCP surface absent from tree; (29) TS SDK = one generic CLI passthrough + typed result shapes (`engine.ts` 53; `query.test.ts` 2/24); CLI = eight subcommands (`cli/sdk.rs` 78–113).

*My inference (not established by the repo):* that >200-result sets are unreachable follows from (1)+(3) but no test asserts it; that whole-corpus-per-call is a practical scaling problem (no benchmark or fixture size in-tree); every tool-count/catalog-budget figure (only the §1a dispatch-surface contrast is measured); that incremental catch-up would be cheaper than total rebuild (no incremental path exists to measure); that the State-classes-vs-NodeType divergence is a defect rather than deliberate; that `ArtifactLink` is a designed style rather than residue; that projection-staleness refusal would be a common ergonomic failure (frequency of stale-projection states is unmeasured); that locking queries would have observable cost; that the double-traversal duplication causes wrong-answer risk today (drift is theoretical until behavior differs).

=== STRUCTURE ===

Four candidates. They differ on *which authority answers*, *which connection styles become traversable*, and *which tiers are visible*. No winner selected.

---

**Candidate A — Uniform traversal over canonical shards.**

*Position.* One closed, enumerated `Relation` vocabulary executed directly over canonical JSONL. Relations subsume edge rows, FK fields, `ArtifactLink`s, and `IdeationTarget`s as declared derivations. The eight operations become thin presets. SQLite stays a non-read-path.

*Mechanism sketch.* One addressable-entity superset enum replacing the four vocabularies (superset required: `IdeationTargetType` already exceeds `NodeType`, and `GraphExport`/SQLite both carry Domain/Boundary). Each `Relation` variant declares endpoint types, direction semantics, and derivation (edge-row | fk-field | embedded-collection). Invariants: relations never masquerade as edges (the endpoint table stays authoritative for `Edge` writes, `edge_validation.rs` 20–33); FK relations remain physically one-directional and reverse traversal is labeled a scan; traversal is visit-budgeted in addition to output-limited (fixing the work/output asymmetry of §6); a cursor requires promoting `records::rank` + canonical ID (`records.rs` 55–59, 122–131) from detail to contract, with a defined resume point replacing `trace`'s mid-breadth cut (`walk.rs` 135–137). Freshness stamps come free via the existing canonical digest (`canonical.rs` 13–56) — answers are current-as-of-stamp by construction, nothing to refuse.

*Must preserve:* response envelope and bounds (`response.rs` 16–33; `protocol.rs` 25–81) or a protocol bump past 5; byte determinism (`records.rs` 8–11); active-view default (`records.rs` 54); the git/scanner dependencies of `stale`/`impact`/`resolve-symbol`/`evidence` untouched (`stale.rs` 30–44; `impact.rs` 65; `symbols.rs` 29; `evidence.rs` 64–70) — those halves are not graph traversal.

*Tradeoffs.* Single authority eliminates the staleness bug class outright; directly satisfies companion invariants 11–12; collapses the §5 duplication into one home; smallest projected tool surface (relation kinds are parameters, matching the SDK's one-generic-dispatch precedent, `engine.ts` 53); compatible with non-goal 429. Costs: inherits and amplifies whole-corpus-per-call (`records.rs` 12–61) on a substrate with **zero indexes** (the system's only indexes are the ones this option refuses to use, `005_report_indexes.sql`); converging four `deny_unknown_fields` vocabularies (`ideation.rs` 275; `projection.rs` 31) plus TS types is breaking and wide.

*What makes it wrong:* (i) the line between "closed relation vocabulary" and the forbidden DSL (non-goal 424) has no structural guardian here — unlike the pinned-graph precedent, where the constraint is "no predicate to run and nothing to remember" (`projection.rs` 19–23); composition pressure will push toward a language. (ii) It argues against a position the companion doc already took (146–148 blesses the eight named operations staying a separate interface) — a reversal to be weighed knowingly.

---

**Candidate B — Revision-stamped SQLite becomes the served read path.**

*Position.* Implement the companion freshness design literally: `provenance.db` gains a revision, executes structured reads, compares against canonical before serving (doc 319–341).

*Mechanism sketch.* Revision cell written in the existing reload transaction (`materialize.rs` 28–37); read adapter compares and applies one of the doc's four remedies (336): catch up, rebuild, canonical fallback, or typed stale refusal — **that choice is itself reviewer-decidable, and "fallback" collapses this into D.** Responses carry stamp + status per invariants 11–12. Keyset pagination is native SQL, closing the §1 cursor gap off-the-shelf using the existing edge indexes (`005_report_indexes.sql`). Upstream dependency: revision representation (doc 442). Gap identified: `graph_digest` digests `GraphExport` bytes, which exclude ideation (`projection.rs` 13–17) — **no digest exists over what SQLite actually stores**; a second digest domain would be needed, or B inherits incomparable claims.

*Must preserve:* `cache.md` 5 wording via the "separate decision" clause of non-goal 429 (explicitly invoked); validator gate and publication-lock snapshot (`materialize.rs` 23–25); byte-identical answers including `rank` ordering; leave-the-projection semantics for the five non-graph-dependent half-operations (§10).

*Tradeoffs.* Only option whose tier coverage already matches "cross-tier" ambition (ideation included, `materialize.rs` 56–62); real joins make the Rule-change ∧ stale-sites walk a SQL join instead of ten nested scans; implements invariants 11–12 head-on. Costs: largest build — §7 shows there is **no reader at all**, so execution, revisioning, comparison adapter, and refusal types are all net-new; worst invalidation behavior (total rebuild per write burst, `clear_cache` 45–68); fail-closed reads on a derived artifact regress today's always-works ergonomics (every op functions with the DB deleted, `cache.md` 3).

*What makes it wrong:* a revision stamp proves recency, not correctness. The correctness obligation is answer-equivalence between a lossy column projection and canonical records; a serial number says nothing about it. Canonical-bytes digests work because the export *is* the records — a materialization cannot make that move. Equivalence would need its own differential proof forever.

---

**Candidate C — Hybrid split by state class.**

*Position.* Adopt the companion State classes table (256–264) as the read partition: Graph-intent/engine-derived serve canonical; Pure-projection/Volatile serve stamped SQLite; Immutable-audit excluded from the graph path per the pinned-graph precedent.

*Mechanism sketch.* Each served operation declares its state class and hence authority; authority becomes part of the response contract. Invariant: no silent authority change across calls. Exclusion enforced structurally à la `projection.rs` 19–23 — the served node union simply has no ideation variant, rather than a runtime filter. Cross-class joins are refused as single calls; the caller composes (sanctioned by doc 147). Blocked precondition: the table classes Boundaries/Domains as Graph intent (258) and Topics/Questions as Working state (259), **inverted relative to `NodeType`** (`graph.rs` 7–20) — C cannot be specified without first ruling which cut is wrong. Further misalignment discovered: fog makes one Requirement record span two classes simultaneously (Working-state content inside a Graph-intent record, doc 258–259 + 269–295 + `artifacts.rs` 297–300), so a per-operation partition cannot even align records to classes.

*Must preserve:* the exclusion rationale verbatim (*"not what the graph asserts,"* `projection.rs` 15–17); existing op semantics unchanged for graph-intent reads; the observed reality that `stale` behaves like Volatile/Code-owned and `get` like neither (`stale.rs` 30–44; grep §6).

*Tradeoffs.* Derives its partition from a classification the project wrote down rather than inventing one; predicts existing behaviour accurately; incremental (classify-and-stamp one operation at a time). Costs: least uniform — uniformity was the question; multiplies contracts (per-class freshness, bounding, paging); permanent maintenance surface at every future family boundary, in a project demonstrably able to hold two incompatible cuts without noticing (§9 vs §2).

*What makes it wrong:* the table's third column is *write policy*, not *read authority*. A taxonomy built to answer "what ceremony does this write need" is borrowed across a purpose boundary, and it does not cut the read problem at its joints — fog is the concrete counterexample. If the joints do not fit reads, C reifies the wrong seams.

---

**Candidate D — Canonical sole authority; the projection a stamped accelerator behind it.**

*Position.* Separate the fused questions: uniformity is a contract question, authority a performance question. Canonical answers every served read permanently; SQLite gains a revision stamp and is consulted only when provably current, otherwise the canonical path answers — silently, because falling back to the authority is not a staleness event. This is the doc's third listed remedy (line 336: *"use a canonical read path"*), which the B framing omits.

*Mechanism sketch.* Uniformity delivered as in A's relation vocabulary, authority fixed at canonical by construction. Projection stores a revision; hit ⇒ serve from SQLite, miss ⇒ canonical; either way current. Invariants: answer-equivalence proven by a differential harness run against both paths (feasible because answers are already promised byte-deterministic, `records.rs` 8–11); projection miss never user-visible as error (contrast B's refusal); the stamp gates *use*, never defines *content*. Unserved `GraphQuery` joins (`gaps/graph_query.rs` 60–261) collapse into the relation vocabulary with wiki/gap answers pinned identical (`graph_query.rs` 46–50).

*Must preserve:* everything, behaviorally — including "cache may be deleted," now honored by construction since it already has no readers (§7); non-goal 429 satisfied without invoking its escape clause; `cache.md` 5 verbatim.

*Tradeoffs.* Fails open (worst case slow, never wrong or refused); decouples sequencing (contract ships with zero projection work; acceleration is additive and deletable); discharges invariants 11–12 trivially; leaves the lifecycle-inclusion question genuinely open since the projection exerts no surface pressure. Costs: two execution paths recreate exactly the duplication drift §5 documents, held together only by the harness, which is not free; under total-rebuild-only invalidation the cache is cold whenever it matters and hot whenever it doesn't (page-cached shards make canonical cheap precisely when the projection is fresh), so the accelerator may never pay; does nothing about whole-corpus load, unconditional depth-10, or repo scans on the fallback path (`records.rs` 12–61; `impact.rs` 34/65).

*What makes it wrong:* it may optimize the one case needing no optimization, conceding forever that the eight operations' fixed costs are load-bearing — and building equivalence machinery around an answer that is too expensive to produce either way. If raw read performance is the actual driver of "cross-tier query demand," D declines to solve it.

---

**Decisions left explicitly to the human reviewer.**

1. **Uniformity vs the query-language non-goal.** Is a closed `Relation` enum on the legal side of non-goal 424, and what structural guard — in the spirit of `projection.rs` 19–23 — holds that line? *(Blocks A and D.)*
2. **Vocabulary convergence.** Merge the four target-type enums, freeze them disjoint, or declare a lattice with non-traversable boundaries? Includes a standalone ruling on `ArtifactLink`, which the original framing omitted.
3. **Domain/Boundary addressability.** Carried by `GraphExport` and SQLite, absent from `NodeType`. Fixable independently of the authority choice; fix now or accept the inconsistency?
4. **State-classes vs NodeType contradiction.** Table rows 258–259 and `graph.rs` 7–20 cut oppositely on Boundary/Domain/Topic/Question. Which is wrong? *(Hard precondition for C; informs all.)*
5. **Read authority.** A/B/C/D — with the explicit note that if B is chosen, picking "canonical fallback" among the doc's four remedies (line 336) turns it into D, and if the stale refusal is chosen it decides question 7 with it.
6. **Stamp representation and scope.** Open decision #5 upstream (serial/digest/both); plus no digest exists over SQLite's stored families — second digest domain, or stamp only exported families?
7. **Refuse vs annotate.** Invariant 12 demands visibility, not refusal; refusing fails closed on a derived artifact that today may be deleted at will. Fail-closed accepted?
8. **Cursor continuation.** `has_more` is terminal and >200 unreachable. Add continuation, thereby promoting rank+ID ordering to contract and defining `trace`'s resume point?
9. **Work bounding.** Visit/scan budget alongside `limit`/`max_depth`? Sharpest case `impact.rs` 34+65; applies to every candidate equally.
10. **Read consistency.** Six of eight operations read unlocked; two lock transitively. Deliberate design or drift? Snapshot-consistency for reads?
11. **Lifecycle-tier inclusion.** Pinned-graph exclusion (semantic rationale quoted) or SQLite inclusion (tables exist)? Same surface or separate one?
12. **Dangling ideation targets.** `artifact_id` unvalidated and index-blind to topic/question/domain. Close before exposing ideation to any traversal, or document dangling refs as property-of-model?
13. **Traversal-layer convergence.** Collapse `GraphQuery` into the chosen story with wiki/gap answer-preservation as a hard constraint?
14. **Weight of the tool-surface criterion.** No MCP/tool-tree exists in-tree; only measured datum is one-generic-dispatch (TS SDK) vs eight-subcommands (CLI). Decide the authority question on other grounds now, defer tool counts until a confined-runtime surface exists, or settle it by product commitment?
