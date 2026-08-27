---
date: 2026-08-27
bead: provenance-1wh
epic: provenance-46p
stage: structure-complete-awaiting-disposal
model: glm-5.3-flash-high
---

SUBJECT BEAD: provenance-1wh — Query uniformity across node kinds, field-attached records, and lifecycle tiers; decide the served read path. Epic provenance-46p.

=== QUESTION ===

**The question in one statement.** Provenance answers reads through eight fixed operations. Each operation loads the whole scope from canonical JSONL. Each traverses validated `Edge` rows only. Records connect in four structural ways. The served path traverses one of those ways. The question is this. Should cross-tier reads share one traversal story? And which artifact answers them? Three choices exist: canonical shards, a revision-stamped SQLite projection, or a split by state class.

This question is practical, not aesthetic. A machine consumer asks many small queries. Non-uniformity then costs extra calls and extra joins. The review must also settle the role of the SQLite cache. It can stay outside the read path. It can become a first-class read authority. Or it can simply gain readers, so it stops being write-only.

**Sub-questions.**

1. **Vocabulary.** Four target-type enums exist. Their member sets differ. Does uniformity merge them into one vocabulary? Or does it freeze them as formally distinct?
2. **Connection-style parity.** Edges are validated and traversable. Field attachments and embedded references are not. Must a uniform layer promote them to first-class relations? Or does it stay edge-only?
3. **Authority and freshness.** No served response names the state it reflects. Can responses carry a revision or digest stamp? And must reads refuse stale data instead of serving it silently?
4. **Boundedness and paging.** `limit` and `has_more` exist. No cursor exists. Results beyond 200 items are therefore unreachable. They are not merely unpaged. Must a uniform layer add continuation? That forces a promise of stable order.
5. **Work versus output bounding.** Answers have limits. The work that produces them does not.
6. **Tier inclusion.** The pinned-graph export excludes ideation. SQLite includes it. Which precedent governs the served path?
7. **Projected tool surface.** How many tools would each option project into a confined runtime's catalog?
8. **Rebuild cost.** What does each option pay when canonical state changes?
9. **Concrete walk.** Take "impact of a Rule change joined with stale evidence sites". How many calls does it need? Is the join expressible at all?

**In scope:** the served read path (`protocol/query.rs`; executors under `operations/queries/`); the four connection styles; the four target-type vocabularies; read authority (canonical shards versus `provenance.db`); stamp-and-refuse semantics; tier inclusion against both precedents; per-option costs; projected tool counts; the Rule-impact walk.

**Out of scope:** the write path (Change Set, plan, commit, approvals); choosing the revision representation (companion open decision #5, upstream); whether JSONL stays canonical storage (open decision #12); designing an MCP server (none exists); search ranking quality; retirement semantics; schema migration; scope model changes; the Proposal naming conflict.

**Decisions this answer must let a human reviewer make:** (D1) edge-only traversal or promoted relations; (D2) vocabulary convergence or frozen disjointness; (D3) SQLite as read authority, as non-authority, or for a declared subset; (D4) response stamps that refuse or that annotate; (D5) cursor continuation and its order-stability prerequisite; (D6) inclusion of lifecycle tiers; (D7) growth, shrinkage, or a cap on the operation count.

=== RESEARCH ===

**Anchor correction.** `docs/research/2026-08-27-data-model-and-erd.md` does not exist. The worktree holds four files under `docs/research/`. All-branch history has no match (`git log --all -- 'docs/research/*erd*'` returns nothing). No claim below uses its figures. The bead ids `provenance-1wh` and `provenance-46p` appear nowhere in the worktree either (`.beads/` holds only config, hooks, and metadata). This report takes them from tasking.

---

**§1. The eight operations and their response contract.**

Request types live in `crates/provenance-core/src/protocol/query.rs`: `GetQuery` (42–49), `SearchQuery` (54–64), `NeighborsQuery` (69–83), `TraceQuery` (88–104), `ImpactQuery` (109–119), `EvidenceQuery` (128–140), `StaleQuery` (145–157), `ResolveSymbolQuery` (162–174). Each sets `#[serde(deny_unknown_fields)]`. Each carries an optional `protocol_version`.

Bounds live in `protocol.rs`. Version 5 sits at line 25. `QUERY_DEFAULT_LIMIT = 50` at 28. `QUERY_MAX_LIMIT = 200` at 31. `TRACE_DEFAULT_MAX_DEPTH = 3` at 34. `TRACE_MAX_DEPTH = 10` at 37. Guards sit at 54–81. `take_page` truncates results and reports `has_more` (84–88).

The envelope `QueryResponse` sits in `response.rs` 16–33. It carries three fields: `protocol_version`, `operation`, and the flattened result. No field names the state behind the answer. No revision exists there. No digest, commit, or timestamp exists either.

Every multi-record result carries `has_more` (`response.rs` 43–116). No cursor, offset, page token, or continuation exists anywhere. Grep across `crates/provenance-core/src/protocol/`, `crates/provenance-store/src/operations/`, and `packages/provenance/src/` returns zero matches. So `has_more: true` ends the conversation. A caller can raise `limit` to 200. Nothing more exists. A match set with 201 items has no expressible query.

`docs/cli.md` 72–132 documents this surface. Lines 76–77 state the intent: *"so nothing here is a query language and nothing needs a daemon."* The CLI mirrors the eight operations one-to-one as subcommands (`crates/provenance-cli/src/cli/sdk.rs` 78–113).

**§1a. Two projections of the protocol already exist, and they differ.** The CLI projects eight distinct subcommands. The TypeScript SDK reaches the same operations through one generic passthrough. Line `packages/provenance/src/engine.ts:53` builds `const args = ["sdk", command]`. It carries typed result shapes only, in `protocol.ts`. Its test drives the real binary (`query.test.ts` imports `execFileSync` and `../../../target/debug/provenance`; lines 2 and 24). Measured precedent: eight named operations can share one dispatch surface. Parameters carry the domain knowledge; endpoints do not.

---

**§2. Four target-type vocabularies, not one.**

| Enum | Location | Members |
|---|---|---|
| `NodeType` | `model/graph.rs` 7–20 | source, requirement, resolution, rule, topic, question (**6**) |
| `IdeationTargetType` | `model/ideation.rs` 14–29 | the six **+ domain** (**7**) |
| `ArtifactLinkTargetType` | `model/shaping.rs` 8–17 | source, requirement, resolution, rule (**4**) |
| `CanonicalArtifactType` | `model/ideation.rs` 49–58 | source, requirement, resolution, rule (**4**) |

`GraphNode` (`protocol/node.rs` 18–25) mirrors `NodeType`. Domain and Boundary follow a hard consequence. Every served operation needs a `NodeType` (`get` requires one, `query.rs` 45). So no served operation can address Domain or Boundary. Three artifacts disagree about what a graph node is. The pinned-graph export carries domains and boundaries (`graph_reference/projection.rs` 36, 38). SQLite lists `"domains"` and `"boundaries"` tables (`cache/materialize.rs` 47–62). The served path excludes both.

---

**§3. Connection styles — there are four, not three.**

**(1) Validated edges.** The endpoint table sits at `edge_validation.rs` 20–33. It runs under `#[rule("rule_prov_edge_endpoint_table")]` (14). `Produces` accepts Resolution→Rule or Requirement→Rule (30–32). Rule is always a leaf. Two proofs show this exhaustively (96–105, 109–123), plus an oracle restatement (90–92). Topic and Question end no edge of any type. Tests assert this at 164–174. Absence from the match arms guarantees it besides.

**(2) Foreign-key attachments.** Six fields link records without edges. `Boundary.requirement_id` sits at `shaping.rs` 118. `Topic.requirement_id` sits at 130. `Question.topic_id` sits at 147. `Question.requirement_id` sits at 149. `Question.resolution_id` sits at 163–168. `Requirement.domain_id` sits at `artifacts.rs` 302–303. Writes check parents. Sites: `state_store/writers.rs` 78 ("domain does not exist"), 125 and 162 ("requirement does not exist"), 151 ("source does not exist"), 311 ("{side} endpoint does not exist").

**(3) Embedded references.** `Requirement.source_refs: Vec<SourceReference>` sits at `artifacts.rs` 304–305; the struct sits at 276–281. `IdeationTarget { artifact_type, artifact_id }` sits at `ideation.rs` 274–281. Proposals embed it (`ideation/proposals.rs` 12). Synthesis packets embed it (`synthesis.rs` 91). Contributions embed it (`contributions.rs` 70).

**(4) `ArtifactLink` — absent from the brief's framing.** `shaping.rs` 104–110 defines it. `Topic.links` holds it at 138. `Question.links` holds it at 162. It is a second embedded-reference style. It has its own four-member vocabulary. It attaches to exactly the two kinds edges cannot reach.

**Drift check on source_refs.** `writers.rs` 131–187 writes the embedded ref and the `References` edge together. One call wraps both: `with_repository_publication` (131–133). So this write path cannot drift. But no reconciler guards state that arrives by import or by hand edit. No invariant check exists for it.

---

**§4. Existence validation is asymmetric.**

`canonical_artifacts.rs` builds an index. Its key covers only the four canonical kinds (78–89). It offers `ensure_exists` (53–66). Three callers pass `disposition.canonical_artifact`: `proposal_writers.rs` 363, `ideation_batches.rs` 119 and 232. Nothing checks that an `IdeationTarget.artifact_id` resolves. Styles (1) and (2) validate at write time. Style (3) admits dangling references by construction.

---

**§5. A second, unserved traversal layer exists.**

`cache/gaps/graph_query.rs` defines `GraphQuery` over `GapGraph` (51–53). Its documentation says it is *"the single home for the traversals the wiki assembler needs too"* (46–50). Its method surface (60–261) holds hand-written joins: `resolving_resolutions` (122), `produced_rules_for_requirement` (150), `producing_requirements` (192), `missing_rule_producers` (228), `rule_trace_reaches_source` (241), `requirement_has_valid_source` (247), `source_is_referenced` (261).

Its loader is `cache/gaps/state_adapter.rs::GraphRecords::load` (35–42). It reads the canonical `StateStore`, not SQLite, despite living under `cache/`. At 65–69 it derives topic retirement by following `topic.requirement_id`. So FK-style traversal already runs in production code. Likewise `cache/health.rs::graph_evidence_locked` unions embedded `source_refs` (79–83) with edge-derived citations (88–90). Cross-style traversal exists twice. Both copies are bespoke and duplicated. Zero of the eight operations expose it.

---

**§6. Execution cost: answers have bounds; the work does not.**

`operations/queries.rs::open` (26–30) builds a `StateStore`. It takes no lock. `with_repository_publication` appears 53 times in provenance-store. None appear in `operations/`. By contrast, `find_gaps` locks (`state_adapter.rs` 10). `graph_evidence` locks (`health.rs` 65). Consequence: `get`, `search`, `neighbors`, `trace`, `impact`, and `resolve-symbol` can observe torn cross-shard state during a concurrent write. `stale` cannot. It locks transitively (`stale.rs` 36 → `health.rs` 65).

`records.rs::load` (12–61) loads and sorts every record kind in the scope. Calls hit it unconditionally: `get` (80), `search` (103), `neighbors` (`walk.rs` 72), `trace` (`walk.rs` 107), `impact` (`impact.rs` 26), and `resolve-symbol` (`symbols.rs` 53). A get-by-ID pays whole-corpus load plus sort. Lookup is then linear (`find`, 63–71).

`trace` nests frontier times all edges per depth (`walk.rs` 115–128). It breaks when `reached.len() > request.limit` (135–137). Truncation can land mid-depth. No resume point exists.

`impact` walks exactly `TRACE_MAX_DEPTH` unconditionally (`impact.rs` 34). It ignores edge-type filters. Then it scans the whole repository source tree on every call (`impact.rs` 65, `scan_path(repo)`).

**Resolve-symbol also scans the working tree** (`symbols.rs` 29). It unions scanned sites with canonical implementation and verification bindings (31–52). That is a cross-source join served today. `evidence` composes five families in one call: implementations (26–31); verifications (32–37); verification runs sorted newest-first from cache JSONL (38–50); open reviews raising `review_required` (51–58); and optionally the git-diff `stale` computation (64–70 → `stale.rs` 30–44). Summary counts: three of eight operations need live filesystem scans. Two need git history. No projection of canonical state alone can serve those halves.

One paging composition defect is evidenced here. `evidence.rs` applies independent `take_page`s to four collections (60–63). One top-level `has_more` covers them. Each sub-collection may itself be cut at `limit + 1`. The reader cannot tell which collection truncated.

---

**§7. SQLite is write-only today.**

`docs/cache.md` 3–9 states the contract. SQLite is rebuildable and *"never the source of truth"* (5). Materialization validates with the aggregate validator (6–7) and copies under the publication lock (7–8). No revision, stamp, or freshness concept exists in that document. None exists in the module either. Grep over `crates/provenance-store/src/cache/` excluding tests returns zero hits for `revision|stamp|generation|digest|freshness`.

`materialize_state` (`materialize.rs` 19–43) runs these steps: snapshot (20); validate scopes (23–25); migrate (27); begin transaction (28); `clear_cache` (29); reload all scopes and edges (32–36); commit (37). `clear_cache` DELETEs sixteen tables (45–68). Rebuild is total. No incremental catch-up exists.

Nothing reads the database back. Every repo-wide `SELECT` lives in two places. One is `cache/tests/materialization_behavior.rs`. The other is `migrations.rs` lines 95, 157, 200, and 207 — all `_schema_migrations` or `sqlite_master` bookkeeping. Indexes are thin: `migrations/005_report_indexes.sql` defines four. Two cover edges with `(scope_id, edge_type, from_type/to_type, from_id/to_id)`. Seventeen migration files exist in total. Yet tier coverage is the broadest of any artifact. Tables span graph, shaping, domains/boundaries, and ideation/collaboration (`materialize.rs` 56–62).

---

**§8. The exclusion precedent and an existing digest.**

`graph_reference/projection.rs` 11–28 sets the precedent. Its motive is semantic:

> 14–17: `collaboration and ideation records ... say who was talking and what they were still arguing about; they are not what the graph asserts, and they never enter the projection.`
> 19–23: `This field list is the rule, not a check laid over it ... a family with no field here cannot leave (`load_projection` has nowhere to put it) and cannot come back (`deny_unknown_fields` refuses a document that names it).`

Two mechanisms carry it: `#[rule("rule_pinned_graph_families")]` (29) and `deny_unknown_fields` (31). `strip_collaboration_fields` scrubs leftovers (208–210, applied at 93). Note: `GraphExport` still carries Domain and Boundary (36, 38).

A self-verifying digest machinery exists. `canonical_bytes` and `sha256` hash sorted-key JSON (`canonical.rs` 13–56). `graph_digest` has one definition site (`export.rs` 24–32). Offline verification works from that digest alone (34–56). It digests `GraphExport` bytes. That works because the export *is* the records.

---

**§9. The companion doc.**

Lines 146–148: structured graph queries *"can remain a separate read interface"*. A REPL composes read and transaction interfaces *"without adding a new command or query language."*

Lines 164–165 give invariants 11 and 12: *"Query projections expose the graph revision that they represent. / A stale projection never returns results without a visible stale condition."*

The State classes table (256–264) reads verbatim:

```
| Graph intent | Sources, Requirements, Rules, Resolutions, Boundaries, Domains, authored edges | Planned Change; approval policy can depend on scope and risk |
| Working state | Fog, claims, open Questions and Topics, shaping threads | Revision-checked transaction; usually no approval ceremony |
| Immutable audit | current Proposals, Assertions, Dispositions, landings | Lifecycle-specific append operation; never rewritten |
| Engine-derived durable state | retirement markers, evidence review records | Engine writes as a consequence of a transaction |
| Volatile evidence | verification runs | Cache; never canonical graph state |
| Pure projection | SQLite, wiki, coverage and frontier reports | Rebuildable and revision-stamped |
| Code-owned state | scanned bindings and declarations owned by an integration | Owner reconciliation through the same transaction kernel |
```

Note the cut. Row 258 puts Boundaries and Domains in Graph intent. Row 259 puts Topics and Questions in Working state. `NodeType` cuts the opposite way (§2).

Lines 319–341 cover projection freshness. Quote: *"The SDK query operations read the state store directly, not SQLite. There is no shared graph revision..."* (322–323). A read adapter compares revisions before serving (333–334). Four remedies exist: *"catch up, rebuild, use a canonical read path, or return a typed stale-projection refusal"* (336). A watcher prewarns but is not correctness (337–338).

Two non-goals bind later options. First: no command language, query language, DSL, or textual grammar (424). Second: *"Do not treat SQLite or another rebuildable projection as canonical without a separate decision"* (429). Open decision #5 asks whether the revision is a serial, a digest, or both (442). Repository-evidence notes cite the canonical graph digest precedent (530–531).

---

**§10. Contradicting evidence I sought.**

- **Against a generic layer:** non-goal 424 forbids a query language. Lines 146–148 endorse keeping the eight named operations. `cli.md` 76–77 repeats both. A relation API with composition pressure risks becoming that forbidden language.
- **Against "FK/embedded traversal doesn't exist":** it exists twice (`state_adapter.rs` 65–69; `health.rs` 79–90). The gap is exposure and duplication, not capability.
- **Against "source_refs drift from edges":** the sole write path couples them under one lock (`writers.rs` 131–186).
- **Against "SQLite is nearly a read path":** zero readers. Four indexes. Delete-all rebuild.
- **Against "eight operations = canonical-only":** three need live filesystem scans; two need git (`impact.rs` 65, `symbols.rs` 29, `stale.rs` 41–43; `evidence.rs` 64–70). Any projection-based candidate must special-case those halves anyway.
- **Against the brief's three-style framing:** `ArtifactLink` is a fourth style (§3-4).
- **Against tool-count reasoning as a hard input:** no MCP surface, tool tree, or catalog exists anywhere in the repo (grep, zero matches). Only one datum is measured: one-generic-dispatch versus eight-subcommands (§1a).

**Evidence-split checklist.**

*Repository facts (directly verified, path + lines above):* (1) eight request shapes, bounds, guards (`query.rs` 42–174; `protocol.rs` 25–88); (2) envelope carries no state stamp (`response.rs` 16–33); (3) no cursor anywhere (grep zero); (4) `NodeType` = 6 without Domain/Boundary (`graph.rs` 7–20; `node.rs` 18–25); (5) four disjoint target-type enums (`graph.rs` 7–20; `ideation.rs` 14–29, 49–58; `shaping.rs` 8–17); (6) endpoint table + rule-leaf proofs (`edge_validation.rs` 20–33, 90–123, 164–174); (7) FK fields and write checks (`shaping.rs` 118/130/147/149/168; `artifacts.rs` 303; `writers.rs` 78–311); (8) fourth style `ArtifactLink` (`shaping.rs` 104–110, 138, 162); (9) `IdeationTarget` unvalidated against a 4-kind index (`canonical_artifacts.rs` 53–89; callers `proposal_writers.rs` 363, `ideation_batches.rs` 119/232); (10) coupled source_ref+edge write under lock (`writers.rs` 131–186); (11) second unserved traversal layer incl. FK traversal (`gaps/graph_query.rs` 46–261; `gaps/state_adapter.rs` 35–69); (12) embedded∪edge union in production (`health.rs` 79–90); (13) query path takes no publication lock; gaps/evidence paths do (grep 53 hits, none in `operations/`; `state_adapter.rs` 10; `health.rs` 65); (14) whole-corpus load per query (`records.rs` 12–61 and six call sites); (15) trace truncates mid-depth (`walk.rs` 135–137); (16) impact depth-10 unconditional + full repo scan (`impact.rs` 34, 65); (17) resolve-symbol scan ∪ bindings (`symbols.rs` 26–52); (18) evidence composes five families with per-collection paging (`evidence.rs` 26–70); (19) stale = git range + locked evidence load (`stale.rs` 30–44); (20) SQLite never truth, no revision concept (`docs/cache.md` 3–9; grep zero); (21) total DELETE+reload of 16 tables (`materialize.rs` 19–68); (22) zero production readers of SQLite (SELECT audit); (23) four indexes / 17 migrations (`005_report_indexes.sql`); (24) SQLite's tier set is broadest incl. ideation (`materialize.rs` 47–62); (25) pinned-graph structural exclusion, semantic rationale (`projection.rs` 11–48, 93, 208–210); (26) existing canonical digest (`canonical.rs` 13–56; `export.rs` 24–56); (27) companion doc quotes as cited (146–148, 164–165, 256–264, 319–341, 424, 429, 442, 530–531); (28) ERD anchor and MCP surface absent from tree; (29) TS SDK = one generic CLI passthrough + typed result shapes (`engine.ts` 53; `query.test.ts` 2/24); CLI = eight subcommands (`cli/sdk.rs` 78–113).

*My inference (not established by the repo):* that >200-result sets are unreachable follows from (1)+(3), but no test asserts it; that whole-corpus-per-call is a practical scaling problem (no benchmark or fixture size in-tree); every tool-count/catalog-budget figure (only the §1a dispatch-surface contrast is measured); that incremental catch-up would cost less than total rebuild (no incremental path exists to measure); that the State-classes-versus-NodeType divergence is a defect rather than deliberate; that `ArtifactLink` is a designed style rather than residue; that projection-staleness refusal would fail often in practice (frequency of stale-projection states is unmeasured); that locking queries costs observable time; that the double-traversal duplication causes wrong answers today (drift stays theoretical until behavior differs).

=== STRUCTURE ===

Four candidates follow. They differ on three axes: which authority answers; which connection styles become traversable; which tiers stay visible. This document picks no winner.

---

**Candidate A — Uniform traversal over canonical shards.**

*Position.* Use one closed, enumerated `Relation` vocabulary. Execute it directly over canonical JSONL. Relations subsume edge rows, FK fields, `ArtifactLink`s, and `IdeationTarget`s as declared derivations. The eight operations become thin presets. SQLite stays out of the read path.

*Mechanism sketch.* Replace the four vocabularies with one superset enum. Superset status is necessary: `IdeationTargetType` already exceeds `NodeType` (7 members versus 6), and `GraphExport` and SQLite both carry Domain/Boundary. Each `Relation` variant declares three things: endpoint types; direction semantics; derivation (edge-row | fk-field | embedded-collection). Three invariants apply. First, relations never pretend to be edges. The endpoint table keeps authority over `Edge` writes (`edge_validation.rs` 20–33). Second, FK relations stay physically one-directional. Reverse traversal is labeled as a scan. Third, traversal gets a visit budget in addition to output limits. This fixes the work/output asymmetry of §6. A cursor needs the other prerequisite too. Promote `records::rank` plus canonical ID to contract (`records.rs` 55–59, 122–131). Define a resume point, replacing `trace`'s mid-breadth cut (`walk.rs` 135–137). Freshness stamps then come free via the existing canonical digest (`canonical.rs` 13–56). Answers are current-as-of-stamp by construction. Nothing needs refusing.

*Must preserve:* the response envelope and bounds (`response.rs` 16–33; `protocol.rs` 25–81), or a protocol bump past 5; byte determinism (`records.rs` 8–11); the active-view default (`records.rs` 54); the git and scanner dependencies of `stale`, `impact`, `resolve-symbol`, and `evidence` untouched (`stale.rs` 30–44; `impact.rs` 65; `symbols.rs` 29; `evidence.rs` 64–70). Those halves are not graph traversal.

*Tradeoffs.* For: one authority removes the staleness bug class entirely. Companion invariants 11–12 gain direct satisfaction. The §5 duplication collapses into one home. Projected tool surface shrinks to the minimum (relation kinds are parameters; compare the SDK's single-dispatch precedent, `engine.ts` 53). Non-goal 429 stays satisfied. Against: costs inherit and amplify whole-corpus-per-call (`records.rs` 12–61). The substrate holds zero indexes. The system's only indexes are exactly the ones this option refuses to use (`005_report_indexes.sql`). Merging four `deny_unknown_fields` vocabularies (`ideation.rs` 275; `projection.rs` 31) plus TypeScript types breaks widely.

*What makes it wrong:* two failure modes exist. (i) Nothing structurally guards the line between a closed relation vocabulary and the forbidden DSL. Compare the pinned-graph guard: there, *"there is no predicate to run and nothing to remember"* (`projection.rs` 19–23). Composition pressure will push toward a language. Non-goal 424 gives no further help. (ii) The companion doc already took the opposite position. Lines 146–148 bless keeping the eight named operations as a separate interface. Candidate A reverses a recorded decision. A reviewer should weigh it as such.

---

**Candidate B — Revision-stamped SQLite becomes the served read path.**

*Position.* Apply the companion freshness design literally (doc 319–341). `provenance.db` gains a revision. It executes structured reads. It compares itself against canonical before serving.

*Mechanism sketch.* Write the revision cell inside the existing reload transaction (`materialize.rs` 28–37). A read adapter compares revisions, then applies one of the doc's four remedies (line 336): catch up; rebuild; fall back to canonical; return a typed stale refusal. That choice is reviewer-decidable. Note the collapse risk: "fall back to canonical" turns B into D. Responses carry stamp plus status per invariants 11–12. Keyset pagination is native SQL. It closes the §1 cursor gap using the existing edge indexes (`005_report_indexes.sql`). One upstream dependency binds: the revision representation (doc 442). One gap needs closing first. `graph_digest` digests `GraphExport` bytes, which exclude ideation (`projection.rs` 13–17). No digest covers what SQLite actually stores. A second digest domain is needed, or B inherits incomparable claims.

*Must preserve:* the wording of `cache.md` 5, through the "separate decision" clause of non-goal 429 — invoked explicitly here; the validator gate and publication-lock snapshot (`materialize.rs` 23–25); byte-identical answers including `rank` ordering; leave-the-projection behavior for the five non-graph-dependent half-operations (§10).

*Tradeoffs.* For: coverage alone already matches the cross-tier ambition (ideation included, `materialize.rs` 56–62). Real joins make the Rule-change-with-stale-sites walk a SQL join instead of ten nested scans. Invariants 11–12 get direct implementation. Against: largest build of the four. §7 shows no reader at all. Execution, revisioning, the comparison adapter, and the refusal types are all net-new. Invalidation behaves worst: total rebuild per write burst (`clear_cache` 45–68). Reads fail closed on a derived artifact. Today every operation works even with the database deleted (`cache.md` 3). That ergonomics regresses.

*What makes it wrong:* a revision stamp proves recency. It does not prove correctness. Correctness means answer-equivalence between a lossy column projection and canonical records. A serial number says nothing about equivalence. Canonical-bytes digests avoid this problem because the export *is* the records. A materialization cannot make that move. Equivalence would need a differential proof maintained forever.

---

**Candidate C — Hybrid split by state class.**

*Position.* Adopt the companion State classes table (256–264) as the read partition. Serve Graph-intent and engine-derived state from canonical. Serve Pure-projection and Volatile state from stamped SQLite. Exclude Immutable-audit from the graph path, per the pinned-graph precedent.

*Mechanism sketch.* Each served operation declares its state class; the class names its authority. Authority becomes part of the response contract. One invariant applies: an operation never changes authority between calls silently. Enforce exclusion the way `projection.rs` 19–23 does: structurally. The served node union simply has no ideation variant. No runtime filter runs. Refuse cross-class joins as single calls. The caller composes instead; doc line 147 sanctions this. One precondition blocks specification. The table classes Boundaries/Domains as Graph intent (row 258) and Topics/Questions as Working state (row 259). `NodeType` cuts inverted (`graph.rs` 7–20). Someone must rule which cut is wrong first. One deeper misalignment follows. Fog puts Working-state content inside a Graph-intent record (doc 258–259 + 269–295; `artifacts.rs` 297–300). One Requirement record spans two classes. A per-operation partition cannot align records to classes.

*Must preserve:* the exclusion rationale verbatim (*"not what the graph asserts,"* `projection.rs` 15–17); existing operation semantics unchanged for graph-intent reads; observed reality: `stale` acts like Volatile/Code-owned and `get` acts like neither (`stale.rs` 30–44; grep facts in §6).

*Tradeoffs.* For: the partition derives from a classification the project already wrote down. It predicts existing behavior accurately. Adoption is incremental — classify and stamp one operation at a time. Against: least uniform of the four; uniformity was the question. Contracts multiply: per-class freshness, bounding, and paging. Maintenance gains a permanent boundary surface for each future family. The project demonstrably already holds two incompatible cuts without noticing (§9 versus §2).

*What makes it wrong:* the table's third column is write policy. It is not read authority. The taxonomy answers "which ceremony does this write need". The read question asks "who may answer". Borrowing across that purpose line imports joints that do not fit. Fog is the concrete counterexample. If the joints do not fit, C makes the wrong divisions durable in the read path.

---

**Candidate D — Canonical sole authority; the projection a stamped accelerator behind it.**

*Position.* Split the fused questions. Uniformity is a contract question. Authority is a performance question. Canonical answers every served read permanently. SQLite gains a revision stamp. It serves only when provably current; otherwise the canonical path answers. Falling back to the authority is not a staleness event. This is remedy three in the doc (line 336: *"use a canonical read path"*). The B framing omits it.

*Mechanism sketch.* Deliver uniformity as in candidate A's relation vocabulary; fix authority at canonical by construction. Store the projection revision. On a hit, serve from SQLite. On a miss, serve from canonical. Either way the answer is current. Prove answer-equivalence with a differential harness over both paths. Feasibility rests on the existing byte-determinism promise (`records.rs` 8–11). Keep a projection miss invisible to users, unlike B's refusal. The stamp gates use of the cache. It never defines answer content. Collapse the unserved `GraphQuery` joins (`gaps/graph_query.rs` 60–261) into the relation vocabulary. Pin wiki-assembler and gap answers identical (`graph_query.rs` 46–50).

*Must preserve:* everything, behaviorally. Including "the cache may be deleted" — honored by construction because no readers exist today (§7). Non-goal 429 stays satisfied without invoking its escape clause. `cache.md` 5 keeps meaning verbatim.

*Tradeoffs.* For: the worst case is slow, never wrong, never refused. Sequencing decouples: the contract ships with zero projection work, and acceleration lands later as additive, deletable code. Invariants 11–12 discharge trivially. The lifecycle-inclusion question stays genuinely open, since the projection exerts no surface pressure. Against: two execution paths recreate the §5 duplication drift. The differential harness holds them together, and it is not free. Total-rebuild invalidation weakens the payoff: the cache is fresh only when nothing changed, and canonical shards are cheap exactly then. So the accelerator may never pay. Fixed costs survive untouched on the fallback path: whole-corpus load (`records.rs` 12–61), unconditional depth-10 (`impact.rs` 34), repo scans (`impact.rs` 65).

*What makes it wrong:* it may optimize the one case needing no optimization. It concedes forever that the eight operations' fixed costs stay load-bearing. And it builds equivalence machinery around an answer that costs too much either way. If raw read speed drives "cross-tier query demand", candidate D declines to solve it.

---

**Decisions left explicitly to the human reviewer.**

1. **Uniformity versus the query-language non-goal.** Is a closed `Relation` enum legal under non-goal 424? If yes, which structural guard — in the spirit of `projection.rs` 19–23 — holds that line? *(Blocks A and D.)*
2. **Vocabulary convergence.** Merge the four target-type enums, freeze them disjoint, or declare a lattice with non-traversable boundaries? Include a standalone ruling on `ArtifactLink`. The original framing omitted it.
3. **Domain/Boundary addressability.** `GraphExport` and SQLite carry them. `NodeType` does not. The fix is independent of the authority choice. Fix now, or accept the inconsistency?
4. **State-classes versus NodeType contradiction.** Table rows 258–259 and `graph.rs` 7–20 cut oppositely on Boundary/Domain/Topic/Question. Which cut is wrong? *(Hard precondition for C; informs all others.)*
5. **Read authority.** Choose A/B/C/D. Two couplings bind the choice: if B wins and adopts "canonical fallback" among the doc's four remedies (line 336), it becomes D; if the stale refusal wins, question 7 is decided with it.
6. **Stamp representation and scope.** Open decision #5 sits upstream (serial/digest/both). No digest covers SQLite's stored families. Define a second digest domain, or stamp only exported families?
7. **Refuse versus annotate.** Invariant 12 demands visibility, not refusal. Refusing fails closed on a derived artifact. Today that artifact may be deleted freely. Accept fail-closed?
8. **Cursor continuation.** `has_more` currently terminates. Results beyond 200 are unreachable. Add continuation? That promotes rank+ID ordering to contract (`records.rs` 55–59, 122–131) and requires defining `trace`'s resume point.
9. **Work bounding.** Add a visit/scan budget beside `limit` and `max_depth`? Sharpest case: `impact.rs` 34+65. Every candidate shares this decision.
10. **Read consistency.** Six of eight operations read unlocked. Two lock transitively. Is that design or drift? Should reads get snapshot-consistency?
11. **Lifecycle-tier inclusion.** Follow the pinned-graph exclusion (semantic rationale quoted in §8) or the SQLite inclusion (tables exist)? Same surface or separate one?
12. **Dangling ideation targets.** `artifact_id` goes unvalidated. The existence index cannot represent topic/question/domain (`canonical_artifacts.rs` 78–89). Close the gap before exposing ideation to any traversal, or document dangling refs as property-of-model?
13. **Traversal-layer convergence.** Collapse `GraphQuery` into the chosen story? Make wiki/gap answer-preservation a hard constraint or not?
14. **Weight of the tool-surface criterion.** No MCP/tool-tree exists in-tree. Only one datum is measured: one-generic-dispatch (TS SDK) versus eight-subcommands (CLI). Decide the authority question on other grounds now, defer tool counts until a confined-runtime surface exists, or settle it by explicit product commitment?
