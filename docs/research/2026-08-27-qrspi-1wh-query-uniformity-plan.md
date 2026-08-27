---
date: 2026-08-27
bead: provenance-1wh
epic: provenance-sfzf
stage: plan-pending-human-review
based-on: docs/research/2026-08-27-qrspi-1wh-query-uniformity-read-path.md (branch opencode/provenance-20260827T074514Z-80825ff0)
model: glm-5.3-flash-high
---

SUBJECT BEAD: provenance-1wh — plan stage for the served read path. Question, Research, and
Structure finished in the based-on document; human disposal picked option B with amendments.
This plan turns those decisions into ordered work. It adds no new positions on settled questions.

Path shorthand: `core` = `crates/provenance-core/src`, `store` = `crates/provenance-store/src`.
Citations give file and line range under those roots unless a full path is needed.

=== A. APPROACH SUMMARY ===

`provenance.db` becomes the served read path for the eight sdk operations and for cross-tier
composition. Canonical JSONL stays the sole write target and truth. Every response carries a stamp:
a monotonic serial plus a new projection digest covering every family SQLite stores (W1). Readers
default to catch-up: compare stamp to canonical, materialize what changed, answer locally. Strict
refusal is opt-in through one policy knob (W3).

Incremental materialization replaces delete-and-reload as steady state (W2). Writers record exact
invalidation events inside the existing publication lock. A shard-digest sweep catches edits that
bypass writers. Total reload remains bootstrap and repair.

The eight operations survive as contract presets over a closed relation vocabulary guarded like the
pinned-graph family rule (W4). Domain and Boundary become addressable through a superset node
vocabulary. `GraphQuery` joins collapse onto the same vocabulary, held byte-identical for wiki and
gap answers by an equivalence harness. Cursor continuation, visit and scan budgets, and truthful
per-collection paging ship with the re-back (W3). Reads take the publication lock and answer under
one snapshot. Rollout moves one operation at a time behind a differential gate (W5). MCP generation
for q82f starts only after stated milestones pass.

=== B. WORKSTREAM BREAKDOWN ===

Workstream numbers name work, not calendar order. W5 states the landing order and the gates.
Each workstream states facts with citations first, then design as inference or plan.

---

**W1 — Stamp and schema layer**

*Goal.* Give `provenance.db` a revision stamp that covers everything it stores, not just the
pinned-graph families.

*Facts.* `graph_digest` has one definition site (`store/graph_reference/export.rs` 30–32). It
hashes `GraphExport` bytes through sorted-key canonical JSON and SHA-256
(`store/graph_reference/canonical.rs` 13–52 for bytes, 54–56 for the digest string).
`GraphExport` carries only canonical families plus bindings; collaboration and ideation records
are excluded by structure — the field list is the rule, backed by `deny_unknown_fields`
(`store/graph_reference/projection.rs` 29–45, motive comment 13–17). SQLite stores sixteen tables:
sources, domains, requirements, boundaries, topics, questions, edges, resolutions, rules, messages,
threads, contributions, synthesis_packets, proposal_cards, assertion_records, dispositions
(`store/cache/materialize.rs` 46–63). Materialization writes rows in one transaction today
(`materialize.rs` 28–37) after validating every scope (`materialize.rs` 23–25).

*Design — second digest domain (inference from the two fact sets above; required by approved
decision 4).* Build a new module `store/cache/projection_digest.rs`. Input: all stored families,
all scopes, serialized family by family, records sorted by canonical id. Bytes come from the same
canonical writer (`canonical.rs` 13–52), so determinism costs nothing new. Digest text uses the
existing `sha256:` format (`canonical.rs` 54–56). This domain is distinct from `graph_digest`;
nothing changes at `export.rs` 30–32.

*Storage.* One additive migration beside the seventeen present files
(`crates/provenance-store/migrations/001_initial_cache.sql` … `017_remove_services_shards.sql`),
numbered 018: a `projection_revision` table (serial INTEGER primary key, digest TEXT, created_at)
plus a `projection_family_digests` table (scope_id, family, digest, record_count) for cheap W2
comparisons. Write both inside the load transaction (`materialize.rs` 28–37 pattern), after all
rows, before commit. Store serial and digest together because open decision #5 upstream (companion
doc line 442) has not fixed the representation; both columns avoid a later migration.

*Data and migration notes.* Additive migration only. No column drops, no renames. An empty database
from `materialize_empty_state` (`materialize.rs` 8–17) holds no revision row; readers treat absent
revision as never-materialized (W3 behavior).

*Test strategy.* Three properties. One: materialize the same canonical state into two fresh
databases; digests match. Two: change one record in any family covered by either loader
(`store/cache/materialize/graph_records.rs`; `store/cache/materialize/collaboration_records.rs`);
digest changes. Three: vary ideation content while graph content stands still; projection digest
moves, `graph_digest` does not. That pins the two domains apart.

*Gates.* Lands before W3 serves anything from SQLite. A stamp without full-family coverage makes
false freshness claims; that is why decision 4 demands this domain exist first.

*Complexity M.*

---

**W2 — Incremental materialization**

*Goal.* Steady-state refresh stops deleting sixteen tables per pass (`materialize.rs` 45–68).
Catch-up applies only what changed. Full reload remains bootstrap and repair.

*Invalidation-trigger enumeration.* Each trigger names how the system detects it.

1. Committed writes through StateStore writers. Detection: precise event. Every published write runs
   under `with_repository_publication` (53 call sites across provenance-store; the ref-plus-edge
   coupled write sits at `store/state_store/writers.rs` 131–187). Journal events recorded inside
   the same locked section see exactly what committed.
2. Engine-derived durable state (retirement markers, review records). Detection: same journal channel.
3. Out-of-band shard edits and imports. Detection: shard-digest sweep during catch-up. Inference:
   no reconciler guards imported state today; the only coupling guarantee on record covers the
   writer path (`writers.rs` 131–187). So content hashes, not assumptions, must close this window.
4. Database loss or corruption. Detection: missing pool or failed integrity check routes to the
   existing total rebuild (`materialize_state`, 19–43). Deleting `provenance.db` stays legal
   (`docs/cache.md` 3–9; today nothing reads it back — zero production SELECTs outside tests and
   `_schema_migrations` bookkeeping).
5. Migration version change. Detection: `run_migrations` report (`materialize.rs` 27); a changed
   schema version forces full rebuild before any serve.

*Catch-up design (inference: no incremental path exists to measure; the shape below follows from
the trigger list).* New function beside `materialize_state`: `catch_up_state(layout) ->
CatchUpReport` in `store/cache/materialize.rs`.

- Step 1. Read stored revision. Absent or behind schema → run `materialize_state`.
- Step 2. Drain journal events newer than the stored serial. Events carry scope, family, record id,
  operation. Apply as keyed UPSERT or DELETE in one transaction.
- Step 3. Sweep shard digests for content no event covered. Recheck files whose size or mtime moved;
  hash them; replace that family's rows for that scope when the hash differs.
- Step 4. Commit rows and the new revision row atomically, following the single-transaction pattern
  of `materialize.rs` 28–37.

Readers consume this through W3's policy point; W2 ships only the function and its report.

*Crash-consistency analysis.*

- Repository side keeps its existing recovery machinery (`store/publication.rs`
  `recover_pending_publication`, 177–207). W2 adds no second recovery story there.
- Known torn window (inference, stated honestly): crash between canonical commit and journal flush
  loses events. Step 3 exists to detect exactly that case; the sweep is therefore required, not
  optional tuning.
- Cost honesty for the sweep (inference): hashing is O(total shard bytes) even when nothing moved.
  Mitigation: metadata fast path (size + mtime) skips unchanged files; hash runs only when metadata
  moved. Measured numbers are out of scope here because no fixture corpus exists yet.
- SQLite side: one transaction per catch-up. A killed process leaves the previous stamped state
  readable, never half-applied. Stamp and rows commit together or not at all.
- Idempotence: UPSERT keyed by scope_id and id. Replaying drained events converges to the same rows.

*Test strategy.* Equivalence property leads: after every supported trigger sequence, catch-up output
compares equal — rows and digest — to a fresh total rebuild. Crash injection drives failures at
labeled points (journal written / canonical committed / db commit pending) and asserts a consistent
readable state plus correct recovery. One test per enumerated trigger, including hand-edited JSONL.
Replay test drains the same journal twice and asserts zero row churn.

*Gates.* The equivalence suite must pass before W3 flips any default serving path to cached mode.
This answers approved decision 5 directly.

*Complexity L.*

---

**W3 — Operations re-back, per query operation**

*Goal.* Serve each of the eight operations from the stamped projection. Preserve response shapes.
Ship the contracted defect fixes with the re-back, not after it.

*Prerequisite inside W3 — dangling-target existence validation (approved decision 8).* Fact:
`IdeationTarget { artifact_type, artifact_id }` reaches proposals, synthesis packets, and
contributions (`core/model/ideation.rs` 276–281, embedded via proposals, synthesis, contributions),
and nothing checks that `artifact_id` resolves. The existence index covers only the four canonical
kinds; topic, question, and domain have no slot (`store/state_store/canonical_artifacts.rs` key
78–89, `ensure_exists` 53–66; callers `proposal_writers.rs` 363, `ideation_batches.rs` 119 and 232).
Plan: extend the index key to the superset vocabulary landed by W4, check targets at the three call
sites above, and surface misses as typed gap items using the DanglingReference precedent
(`store/cache/gaps/dangling.rs` 7–15 and family). Ideation-tier exposure in served reads turns on
only after this lands.

*Per-operation mapping.* Facts cite current executors.

1. `get`, `search` — become direct row lookups and indexed text predicates. The whole-corpus loader
   dies: `records::load` loads and sorts every kind per call (`store/operations/queries/records.rs`
   12–61) and six operations hit it unconditionally (get 80, search 103, neighbors
   `walk.rs` 72, trace `walk.rs` 107, impact `impact.rs` 26, resolve-symbol via its bindings path).
2. `neighbors`, `trace` — walk edge indexes instead of nested scans. Indexes already exist:
   `idx_edges_scope_type_from` and `idx_edges_scope_type_to`
   (`crates/provenance-store/migrations/005_report_indexes.sql`). Trace gains a resume token that
   continues from a depth plus rank-plus-id watermark, replacing the mid-breadth cut at
   `walk.rs` 135–137 (break when `reached.len() > limit`). Ordering contract promoted to writing:
   node rank then canonical id (`records.rs` 122–131 `rank`, sort at 55–59; trace order via
   `node_order`, `walk.rs` 158–163). Cursor pages must reproduce it bit for bit.
3. `impact` — traversal served from the projection; depth cap unchanged (`TRACE_MAX_DEPTH`,
   `core/protocol.rs` 37; loop bound `impact.rs` 34). New visit budget bounds steps walked; new scan
   budget bounds the repository source scan (`impact.rs` 65 `scan_path(repo)`), which stays live
   because code-owned state is not projected.
4. `resolve_symbol` — hybrid, kept honest: scanned sites stay filesystem-side
   (`store/operations/queries/symbols.rs` 29 `scan_path`); canonical implementation and verification
   bindings come from the DB into the same union (union shape at symbols.rs 31–52).
5. `evidence` — implementations and verifications from DB (collection filters at
   `store/operations/queries/evidence.rs` 26–37); review records from DB (51–58); git-diff stale
   half untouched (64–81). Paging truthfulness fix lands here: four collections cut independently
   with one merged flag hide which side truncated (`take_page` calls at evidence.rs 60–63; OR merge
   at 85). Response gains per-collection `has_more` flags and per-collection cursors. Existing
   top-level fields stay.
6. `stale` — git machinery only; near-zero re-back. It keeps reading the diff, never the working
   tree, per its own doc comment (`stale.rs` header, 13–22 area) and keeps its lock discipline
   (already locks transitively through `health.rs` 59–66).

*Consistency decision callout (required; approved decision 6 leaves the choice explicit).* Fact:
every sdk query enters through `open`, which builds a StateStore with no publication lock
(`store/operations/queries.rs` 26–30); zero `with_repository_publication` references exist under
`operations/`. Contrast: gap policy locks
(`state_adapter.rs` 10) and evidence health locks (`health.rs` 65). Plan chooses
**snapshot-under-publication-lock**: one reader-entry helper takes the lock, runs the W2 freshness
step, and answers from the stamped snapshot; live-scan halves execute inside the same section.
Justification: the lock kernel and recovery already exist and carry 53 call sites; the unlocked
window can observe torn cross-shard state during concurrent publication. Costs stated plainly
(inference, qualitative): reads serialize against publications; long publishes delay interactive
queries; no latency figure exists in-tree. The rejected alternative — document-only single-writer
assumption — would be cheaper but leaves import and hand-edit windows unsynchronized and appears
nowhere in the tree as a written rule. Code isolates the lock acquisition in one helper so reversal
touches one site.

*Contract additivity statement (approved decision 2).* Additive response fields: stamp object
(serial, digest, applied policy), `next_cursor` where paging extends, per-collection paging flags.
Additive request fields: optional cursor, visit_budget, scan_budget. Requests refuse unknown fields
(`deny_unknown_fields` on every request type, `core/protocol/query.rs` 41–174 area); optional
additions parse cleanly. Envelope gains fields only; protocol_version, operation, flattened result
stay (`core/protocol/response.rs` 16–33). **Protocol bump flag:** none taken. Version stays 5
(`SDK_PROTOCOL_VERSION`, `protocol.rs` 25) because every change is additive-with-default-absent.
A bump becomes necessary only if a field must be removed, renamed, or semantics narrowed; that goes
to humans explicitly, never chosen silently.

*Test strategy.*

- Differential harness per operation: old executor versus served executor over the shared fixture
  corpus; serialized JSON equal except additive fields. Runs in CI permanently as drift alarm.
- Order stability property: interleaved inserts and deletes preserve contract ordering.
- Cursor exhaustion loops: page to end on small fixtures; union of pages equals unpaginated result;
  repeating a page request returns identical bytes.
- Typed-shape updates land additively in the TypeScript layer; the envelope types sit at
  `packages/provenance/src/protocol.ts` 205–212 and the engine dispatches generically
  (`packages/provenance/src/engine.ts` 53), so no per-op SDK logic changes.

*Gates.* Each operation flips its default path only after its differential suite passes. Until
flip, it keeps answering over canonical, as today. No flag day. *Complexity L.*

---

**W4 — Relation vocabulary, Domain/Boundary addressability, GraphQuery collapse, equivalence harness**

*Goal.* One closed relation vocabulary parameterizes every traversal. Domain and Boundary gain
addressability. The bespoke `GraphQuery` joins collapse onto the same vocabulary. Wiki and gap
answers stay byte-identical throughout.

*Vocabulary design (approved decision 7).* New module `core/model/relations.rs` defining a closed
`RelationKind` enum. Each variant declares endpoint pair, direction semantics, and derivation tag
one of `edge_row | fk_field | embedded_collection`. Seed set comes exhaustively from what exists:

- Nine edge types (`edge_rank`, `store/operations/queries/walk.rs` 173–184).
- Six foreign-key attachments: `Boundary.requirement_id` (`core/model/shaping.rs` 118),
  `Topic.requirement_id` (130), `Question.topic_id` (147), `Question.requirement_id` (149),
  `Question.resolution_id` (163–168), `Requirement.domain_id` (`core/model/artifacts.rs` 303).
- Embedded reference collections: `Requirement.source_refs` (`artifacts.rs` 304–305 over the
  struct at 276–281) and the `ArtifactLink` lists on Topic and Question (`shaping.rs` 105–110,
  138, 162).
- Ideation target references (`core/model/ideation.rs` 276–281) enter only after W3's validation
  prerequisite lands.

*Structural guard, in the pinned-graph spirit.* The enum admits no wildcard fallback: traversals
match exhaustively, so a family without a declared variant cannot traverse at all — compile error,
not runtime filter. This copies the mechanism of `projection.rs` 19–23 ("this field list is the
rule") onto relations. It also draws the non-goal 424 line in code: parameters, not predicates;
no composition grammar anywhere.

*Superset node vocabulary (approved decision 6).* `NodeType` gains Domain and Boundary variants
(current six at `core/model/graph.rs` 7–20). `GraphNode` mirrors it (`core/protocol/node.rs` 18–25).
`rank` extends (`records.rs` 122–131) and the position pins by test. Wire names follow the file's
parse convention (`graph.rs` 22–25 and `normalize_enum_value`). Cost statement (fact):
GetQuery hard-requires membership in this type (`core/protocol/query.rs` 42–49 — `node_type` has
no default), which is precisely why Domains and Boundaries are unreachable from every served
operation today; widening closes that defect mechanically but touches match sites and TypeScript
types broadly. Expect wide but mechanical edits.

*GraphQuery collapse.* The hand-written joins — `resolving_resolutions` (122),
`produced_rules_for_requirement` (150), `producing_requirements` (192),
`missing_rule_producers` (228), `rule_trace_reaches_source` (241),
`requirement_has_valid_source` (247), `source_is_referenced` (261) — reimplement over the shared
relation executor, keeping `GraphRecords::load` (`state_adapter.rs` 35–42) as the construction front.
The embedded-union behaviors pin: source_refs ∪ References edges in health
(`health.rs` 79–99) and topic retirement derivation (`state_adapter.rs` 65–69).

*Equivalence harness spec (byte-identical mandate, approved decision 7).*

- Corpus: existing gap fixtures (`store/cache/gaps/tests/fixtures.rs`) plus adversarial additions —
  dangling targets, retired chains, records connected by both edge and FK simultaneously.
- Method: keep pre-collapse implementations verbatim under test cfg. Run old and new over identical
  inputs. Assert byte equality of wiki assembler output and of serialized `GapItem` vectors.
- Cadence: CI permanent. Removal of the preserved originals needs its own later decision.

*Tests beyond the harness.* Enumeration completeness (every FK field maps to a declared variant —
compile-time). Serde round-trips for widened NodeType. Rank-order pinning.

*Gates.* Vocabulary merges before W3 re-back begins; the superset and relation executor are inputs
to it. Collapse deletes bespoke copies only after the harness runs green on the whole corpus.
*Complexity L.*

---

**W5 — Rollout staging, configuration knobs, MCP handoff**

*Landing order and gates.*

1. W1 merges. Nothing serves from SQLite yet. Update `docs/cache.md` 3–9 wording so readers learn
   the database now answers queries under a stamp; "never the source of truth" stays true and stays
   written.
2. W2 equivalence suite green across consecutive CI runs; catch-up eligible as default freshness step.
3. W4 part one (vocabulary plus superset NodeType) merges. CLI docs update where node kinds print
   (`docs/cli.md` 72–132 documents the command surface).
4. W3 flips operations in order: get → search → neighbors → trace → impact → evidence →
   resolve_symbol. Stale last and mostly unchanged. Each flip reverses independently by config.
5. Ideation-tier exposure opens per operation only after the dangling-validation prerequisite lives.
6. W4 part two completes; bespoke GraphQuery copies delete.

*Configuration knobs (approved decision 3 made concrete).*

- `read.freshness_policy`: `catch_up` (default) | `annotate_only` | `refuse_stale`.
  Implemented at one reader-policy module, planned location `store/operations/read_policy.rs`.
  `catch_up` materializes then serves locally. `annotate_only` stamps without catching up, for
  offline use. `refuse_stale` returns a typed staleness error naming the gap between stamps —
  reserved enum member, machine-readable, opt-in.
- `read.visit_budget` and `read.scan_budget`: defaults fixed at implementation; requests may override
  downward within caps added in W3.
- `cache.catchup_journal`: boolean enabling the write-side event journal. Off means sweep-driven
  catch-up only: simpler, slower steady state; allowed for repositories that want fewer moving parts.

*MCP consumer q82f handoff (approved decision 9).* Generation starts when all of these hold:

1. Every operated response carries the stamp (W1 plus flips complete), so tool descriptions can
   promise annotated freshness.
2. Cursor continuation is live for get, search, neighbors, trace — q82f paging designs rely on
   tokens, not just `has_more`.
3. Evidence reports per-collection paging truthfully.
4. Both equivalence harnesses green in CI: op-level differential parity and wiki/gap byte parity.
5. Freshness policy documented publicly, so generated tool guidance can cite it.
6. Protocol version confirmed at 5, so clients pin v5 with confidence.

Not blocking q82f: ideation enablement (lands later, gated), budget tuning, journal switchover.
Both remaining policies work regardless of incremental mode. *Complexity M* (staging itself;
cost carried by earlier workstreams).

=== C. OPEN QUESTIONS FOR HUMAN REVIEW ===

Each item blocks a named deliverable. None relitigates settled decisions.

1. **Lock reversal window.** W3 picks snapshot-under-publication-lock with stated costs. Reverse it
   before the reader-entry helper freezes if a cheaper accepted inconsistency is preferred.
   Blocks: W3 flip order start.
2. **Stamp representation tie-in.** Open decision #5 upstream owns serial-versus-digest. W1 stores
   both columns meanwhile. Confirm dual storage or pick now. Blocks: migration 018 final shape.
3. **Budget exposure.** Do visit/scan budgets appear as request fields, config only, or both? Plan
   defaults to both. Blocks: W3 contract freeze that q82f depends on.
4. **Domain/Boundary rank slots.** Where the two new members sit in contract ordering changes
   observable page boundaries. Blocks: W4 vocabulary merge.
5. **Evidence cursor shape.** Per-collection tokens returned inline versus one composite token.
   Blocks: W3 evidence flip.
6. **Verification-run storage.** Verification runs live in their own cache JSONL today
   (`evidence.rs` 38–50). Keep them outside the projection (plan default) or move them in? Moves
   change the W1 digest coverage list. Blocks: W1 family list freeze.
7. **Journal-first ordering.** Journal-before-commit costs one extra fsync per publish; sweep-only
   detection avoids it but widens the crash window W2 documents. Pick the tradeoff. Blocks: W2 kernel
   touch approval.
8. **cache.md amendment.** Lines 3–9 say nothing reads the database back and frame it as rebuildable
   scratch. Serving reads keeps truth in JSONL but ends "write-only" as a description. Approve the
   doc rewrite angle before W5 stage 1. Blocks: rollout stage 1 docs landing.

=== D. ACCEPTANCE CHECKLIST ===

Pre-existing defects, each mapped to an observable verification. All verifications are commands or
test invocations against fixtures, checked by a human reading output.

| Defect (as evidenced) | Observable verification |
|---|---|
| Results past 200 unreachable: `take_page` truncates once, `has_more` ends the conversation (`core/protocol.rs` 84–88; limit cap 200 at 31; no cursor anywhere) | Fixture with 500 matches; loop search/neighbors pages via cursors to exhaustion; concatenated pages equal ground truth listing; repeated page fetch returns identical bytes |
| Trace truncates mid-breadth with no resume (`walk.rs` 135–137) | Wide fixture exceeding limit at depth 2; final resumed walk equals an untruncated run at same max_depth; no duplicate TracedNode across boundary; resume token rejected on mismatched request params |
| Impact work unbounded: depth-10 forced walk plus whole-tree scan (`impact.rs` 34, 65) | Instrumented counters printed in test prove stop at budget; scan budget halves `scan_path` visits on fixture; both knobs accept and reject values per caps |
| Resolve-symbol scans the working tree unbounded (`symbols.rs` 29) | Same scan-budget knob visible on resolve-symbol; counter reports capped visits; union result unchanged for sub-budget runs |
| Evidence hides which collection truncated (`evidence.rs` 60–63, OR merge 85) | Fixture where implementations and reviews both exceed limit; response shows true per-collection flags; each collection paginates to completion independently |
| Domain/Boundary unaddressable (`NodeType` lacks them, `query.rs` 45 requires membership; GetQuery at 42–49) | `sdk get --node-type domain <id>` returns the record; neighbors traversal crosses `Requirement.domain_id`; serde round-trip includes new variants; gap outputs unchanged (harness green) |
| Unlocked reads risk torn views (`queries.rs` 26–30 no lock; contrast `state_adapter.rs` 10) | Concurrency test interleaves a publication with reads; every observed response self-consistent, stamped serial matches snapshot contents; helper logs lock acquisitions |

=== E. OUT OF SCOPE RESTATED ===

- **Write path untouched.** Change Set, plan, commit, approvals behave as today
  (`writers.rs` call chains). The only addition is journal emission inside the already-locked
  section (W2). No approval ceremony changes.
- **Canonical JSONL stays sole truth.** `provenance.db` remains deletable and rebuildable
  (`docs/cache.md` 3–9); loss degrades speed, never correctness.
- **Non-goal 424 binds generated surfaces equally.** q82f tools expose the eight fixed operations.
  The relation vocabulary stays a closed parameterization with no predicate grammar; no MCP tool
  accepts free-form queries.
- **No silent protocol bumps.** v5 persists additively. Removal, rename, or semantic narrowing
  forces an explicit human version decision first (W3 contract statement).
- **Non-goal 429 honored.** Nothing treats the projection as canonical; the served-read reversal
  rests on the approved disposal of bead provenance-1wh, recorded upstream, not assumed here.
- Also untouched: search ranking quality, retirement semantics redesign, MCP server implementation
  itself, state-class taxonomy reconciliation, tool-count arbitrage (deferred per decision 9).
