---
date: 2026-08-27
bead: provenance-1wh
epic: provenance-sfzf
stage: plan-pending-human-review
based-on: docs/research/2026-08-27-qrspi-1wh-query-uniformity-read-path.md (branch opencode/provenance-20260827T074514Z-80825ff0)
model: glm-5.3-flash-high
---

SUBJECT BEAD: provenance-1wh — plan stage for the served read path. Question, Research, and
Structure finished in the based-on document. Human disposal settled the authority question:
`provenance.db` is THE served read path; canonical JSONL stays the sole write target and
durable truth; the earlier shards-served authority position is superseded. This revision folds
in an independent maintainability review: truthful catch-up, per-operation stamp semantics,
atomic projection writers, one traversal core, digest-machinery reuse, corrected counts and
citations, and file-growth gates. It adds no new positions on other settled questions.

Path shorthand: `core` = `crates/provenance-core/src`, `store` = `crates/provenance-store/src`.
Citations give file and line range under those roots unless a full path is needed.

=== A. APPROACH SUMMARY ===

`provenance.db` becomes the served read path for the eight sdk operations and for cross-tier
composition. Canonical JSONL stays the sole write target and durable truth. Every response
carries a stamp: a monotonic serial plus a projection digest covering every family SQLite
stores (W1). The stamp names the attested domains and the live constituents it does not
attest (W3 table). Readers default to catch-up: compare stamp to canonical, materialize what
changed, answer locally. A response may claim full freshness only under the W2 truth rules:
complete shard bytes are hashed whenever the journal cannot prove event coverage. Strict
refusal is opt-in through one policy knob (W5).

Incremental materialization is the steady-state path (W2). Total reload is bootstrap and
repair only. Every write to `provenance.db`, including total rebuild, runs under the
repository publication lock from migrations through transaction commit (W2). Writers record
invalidation events in a journal module beside `publication.rs`.

The eight operations survive as contract presets over one Relation traversal core with two
declared provider fronts (W4). Domain and Boundary become addressable through a superset node
vocabulary. `GraphQuery` joins collapse onto the same core, held byte-identical for wiki and
gap answers by an equivalence harness. Cursor continuation, visit and scan budgets, and
truthful per-collection paging ship with the re-back (W3, W5). Reads take the publication
lock and answer under one snapshot. Rollout moves one operation at a time behind a
differential gate (W5). MCP generation for q82f starts only after stated milestones pass.

=== B. WORKSTREAM BREAKDOWN ===

Workstream numbers name work, not calendar order. W5 states the landing order and the gates.
Each workstream states facts with citations first, then design as inference or plan.

---

**W1 — Digest machinery reuse, stamp schema, projection families**

*Goal.* Give `provenance.db` a revision stamp that covers everything it stores, and build it
from the existing canonical digest machinery, not a second copy.

*Facts.* `graph_digest` has one definition site (`store/graph_reference/export.rs` 30–32). It
hashes `GraphExport` bytes through sorted-key canonical JSON and SHA-256
(`store/graph_reference/canonical.rs` 13–52 for bytes, 54–56 for the digest string). Those
functions are `pub(super)` (canonical.rs 13, 54): only `graph_reference` can call them today.
`GraphExport` carries only canonical families plus bindings; collaboration and ideation
records are excluded by structure — the field list is the rule, backed by
`deny_unknown_fields` (`store/graph_reference/projection.rs` 29–45, motive 13–17). SQLite
stores sixteen tables: sources, domains, requirements, boundaries, topics, questions, edges,
resolutions, rules, messages, threads, contributions, synthesis_packets, proposal_cards,
assertion_records, dispositions (`store/cache/materialize.rs` 46–63; loaders
`store/cache/materialize/graph_records.rs` 5–13 and
`store/cache/materialize/collaboration_records.rs` 5–10). Materialization writes rows in one
transaction today (`materialize.rs` 28–37) after validating every scope (`materialize.rs`
23–25).

Three canonical families the eight operations read are canonical shards today and are NOT
stored: implementation bindings, verification bindings, requirement reviews (shard paths
`store/shards.rs` 76, 62, 69; readers `store/operations/queries/bindings.rs` 13–29,
`store/operations/queries/evidence.rs` 26–37 and 51–58). Verification runs are cache JSONL,
not canonical (`layout.rs` 41–46; `docs/cache.md` 11–14).

*Design — digest machinery reuse (review mandate).* Move `canonical_bytes`,
`write_canonical_json`, `digest`, and `sha256` out of `graph_reference/canonical.rs` into a
crate-level module `store/canonical_digest.rs`; `graph_reference` re-exports them and
`export.rs` 30–32 keeps its exact behavior. `cache` code calls the same functions. No
reimplementation of canonical serialization exists anywhere in this plan.

*Design — second digest domain (inference from the two fact sets above; settled by disposal).*
Build `store/cache/projection_digest.rs`. Input: the projection family table below — all
stored families, all scopes, serialized family by family, records sorted by canonical id.
Bytes come from the relocated canonical writer, so determinism costs nothing new. Digest text
uses the existing `sha256:` format. This domain is distinct from `graph_digest`; nothing
changes at `export.rs` 30–32.

*Design — table-driven assembly (review mandate).* One static table,
`PROJECTION_FAMILIES`, names every family SQLite stores: table name, record type, loader
binding, serializer. The digest assembler, the materialize loaders, the W2 sweep, and the W2
journal all read this one table, so adding a family updates coverage, invalidation, and
verification together. A new family without a table row cannot be stored or stamped.

*Family set.* Nineteen families: the sixteen stored today plus implementation bindings,
verification bindings, and requirement reviews, so the canonical halves of `impact`,
`evidence`, and `resolve_symbol` become projection-attested (W3 table). Verification runs
stay outside the projection: volatile evidence, cache JSONL (`docs/cache.md` 11–14; companion
doc State classes row, based-on document lines 256–264). Open question 5 below owns that cut.

*Storage.* One additive migration beside the seventeen present files
(`crates/provenance-store/migrations/001_initial_cache.sql` … `017_remove_services_shards.sql`),
numbered 018. It creates the three family tables plus: a `projection_revision` table (serial
INTEGER primary key, digest TEXT, created_at) and a `projection_family_digests` table
(scope_id, family, digest, record_count, size_bytes, mtime_ns) for cheap W2 comparisons. The
metadata columns back the W2 skip rules. Store serial and digest together because open
decision #5 upstream (companion doc line 442) has not fixed the representation; both columns
avoid a later migration.

*Data and migration notes.* Additive migration only. No column drops, no renames. An empty
database from `materialize_empty_state` (`materialize.rs` 8–17) holds no revision row;
readers treat absent revision as never-materialized (W2 behavior).

*Test strategy.* Four properties. One: materialize the same canonical state into two fresh
databases; digests match. Two: change one record in any family in `PROJECTION_FAMILIES`;
digest changes. Three: vary ideation content while graph content stands still; projection
digest moves, `graph_digest` does not. Four: the digest assembler consumes only
`PROJECTION_FAMILIES`; a test walks the table, serializes each family, and reproduces the
assembler's digest byte for byte.

*Gates.* Lands before W3 serves anything from SQLite. A stamp without full-family coverage
makes false freshness claims; that is why the disposal demands this domain exist first.

*Complexity M.*

---

**W2 — Incremental materialization, journal, atomic writers**

*Goal.* Steady-state refresh stops deleting sixteen tables per pass (`materialize.rs` 45–68).
Catch-up applies only what changed and may claim freshness only when the truth rules hold.
Every projection write is atomic under the publication lock.

*Facts — the unlocked write window today.* `materialize_state` (`materialize.rs` 19–43)
takes the publication lock only for its snapshot (`materialize.rs` 20, via
`snapshot_state`, `store/publication.rs` 400–412). The lock releases when the snapshot
returns. `run_migrations` (`materialize.rs` 27) and the row transaction (`materialize.rs`
28–37) then run unlocked. (Inference: a concurrent publication can interleave with the
SQLite write today.) `recover_pending_publication` (`store/publication.rs` 177–208) covers
repository-side recovery; W2 adds no second recovery story there.

*Fact — lock usage audit (corrected counts).* `rg -n 'with_repository_publication' crates/`
reports 60 matches: 2 definition sites (`publication.rs` 46 free function, 451 StateStore
method), 48 production call sites (37 in provenance-store outside `publication.rs`; 4 inside
`publication.rs` at 401, 447, 455, 466; 7 in provenance-cli), and 10 test call sites. The
earlier "53 call sites" figure was wrong and is withdrawn.

*Fact — reader audit (attribution corrected).* No production code reads `provenance.db`
back. Every `SELECT` string in the workspace lives in `store/migrations.rs` (95 and 157 are
production `_schema_migrations` bookkeeping; 200 and 207 are test assertions) and in
`store/cache/tests/materialization_behavior.rs` (tests). This is the audit's finding; the
based-on research document section 7 records it. `docs/cache.md` 3–9 claims only that the
database "can be deleted and rebuilt" and is "never the source of truth"; it makes no
no-readers claim.

*Fact — no integrity check exists.* No `integrity_check` or `quick_check` call exists in the
tree. An earlier draft routed rebuilds to a "failed integrity check"; that mechanism does not
exist and is withdrawn. Detection of a lost or corrupt database is: pool open failure or
migration failure, both of which route to total rebuild.

*Fact — shard layout.* Canonical shards are per-family files under
`state/scopes/<scope>/<family>/` (`store/shards.rs` 6–142, nineteen scoped path functions).
Edges are one global shard with no scope in the path (`shards.rs` 149). A sweep and a journal
must address scope and family exactly this way.

*Invalidation-trigger enumeration.* Each trigger names how the system detects it.

1. Committed writes through StateStore writers. Detection: precise event. Every published
    write runs under `with_repository_publication` (48 production call sites; the
    ref-plus-edge coupled write sits at `store/state_store/writers.rs` 131–187). Journal
    events recorded inside the same locked section see exactly what committed.
2. Engine-derived durable state (retirement markers, review records). Detection: same journal
    channel.
3. Out-of-band shard edits and imports. Detection: byte-verify sweep during catch-up
    (rules below). Inference: no reconciler guards imported state today; the only coupling
    guarantee on record covers the writer path (`writers.rs` 131–187). So content hashes,
    not assumptions, must close this window.
4. Database loss or corruption. Detection: missing pool, failed open, or failed migration
    routes to the existing total rebuild (`materialize_state`, 19–43). Deleting
    `provenance.db` stays legal (`docs/cache.md` 3–9; zero production readers per the audit
    above).
5. Migration version change. Detection: `run_migrations` report (`materialize.rs` 27); a
    changed schema version forces full rebuild before any serve.

*Design — atomic projection writers (review mandate).* Every write to `provenance.db`,
including total rebuild, runs inside one `with_repository_publication` section that spans
snapshot, validation, `run_migrations`, the row transaction, and commit. The current split —
locked snapshot, then unlocked migrations and transaction — ends. The lock closure is
synchronous and the SQLite work is async; the implementation bridges this inside the closure
(`block_on` or a dedicated thread) and must not release the lock across the bridge. Rebuild
(`materialize_state`) and catch-up (`catch_up_state`) take the same lock, so they serialize
against each other and against canonical publications.

*Design — journal module (review mandate: place it before growing `publication.rs`).*
`publication.rs` is 476 lines today (`wc -l`), against the hard 500-line limit per Rust file.
Journal logic goes in a new sibling module `store/publication/journal.rs` — the file already
uses siblings (`mod read_only`, `containment_tests`, `tests`) — and `publication.rs` gains
only `mod journal;` plus re-exports. Journal entry: `{ sequence, scope, family, record_id,
operation }`, appended as JSONL under the cache directory. Events carry family names from
`PROJECTION_FAMILIES`; families with no SQLite table are recorded but ignored by catch-up.

*Serial space.* One monotonic serial space covers journal sequences and
`projection_revision.serial`. Allocation happens inside the locked section: journal appends
extend the journal tail (seeded from that tail under the lock); the revision row's serial is
written inside the SQLite transaction as the highest drained journal sequence, or stored + 1
after a sweep-only pass. A rolled-back transaction consumes no serial. After a committed
catch-up, drained journal entries (sequence ≤ new serial) are pruned; the journal therefore
holds only entries newer than the stored serial.

*Truthful catch-up (review mandate).* `catch_up_state(layout) -> CatchUpReport` beside
`materialize_state`:

- Step 1. Read stored revision. Absent or behind schema → run `materialize_state`.
- Step 2. Drain journal events newer than the stored serial. For each drained
  (scope, family), re-read that family's complete shard bytes, re-derive rows, and replace
  them keyed by scope and id. Events are hints that name what to re-derive; row content
  always comes from shard bytes, so a phantom event (canonical rollback after append) can
  never inject a row that canonical does not hold.
- Step 3. Byte-verify the rest. For each stored (scope, family) no event covered: if
  size and mtime match `projection_family_digests`, skip; otherwise hash the complete shard
  bytes and replace rows when the digest differs.
- Step 4. Commit rows and the new revision row atomically, then prune the journal.

*Freshness claims (the truth rules).* A response may claim full freshness for a domain only
when every stored (scope, family) behind it was either re-derived from complete shard bytes
in step 2 or byte-verified in step 3. Size+mtime skipping is licensed only when the journal
proved event coverage for that scope and family in this pass. When the journal is off,
exhausted, or pruned past the stored serial, it cannot prove coverage for anything: step 3
hashes complete shard bytes for every family before any full-freshness claim. Default:
`cache.catchup_journal` is ON when `read.freshness_policy` is `catch_up` or `refuse_stale`;
journal-off remains possible but pays full hashing per catch-up.

*Crash-consistency analysis.*

- Repository side keeps its existing recovery machinery (`publication.rs` 177–208). W2 adds
  no second recovery story there.
- Torn window: crash between canonical commit and journal append loses the event. The
  uncovered family then fails its metadata check or gets hashed, so step 3 detects it; the
  sweep is required, not optional tuning. Correctness never depends on the journal; the
  journal only licenses skips.
- SQLite side: one transaction per catch-up or rebuild. A killed process leaves the previous
  stamped state readable, never half-applied. Stamp and rows commit together or not at all.
- Rollback behavior: canonical rollback after journal append leaves a phantom event;
  step 2 re-derives from bytes, so the phantom is harmless. SQLite rollback leaves the stored
  serial and digests untouched; the next pass re-drains.
- Interleaving: rebuild and catch-up serialize on the publication lock. A catch-up that
  starts after a rebuild's commit sees the rebuild's serial and drains nothing.
- Idempotence: re-derived rows keyed by scope and id. Replaying drained events converges to
  the same rows.
- Cost honesty (inference): journal-off catch-up hashes O(total shard bytes) per pass.
  Journal-on steady state hashes only drained families plus metadata-moved files. No fixture
  corpus exists to measure yet.

*Test strategy.* Equivalence property leads: after every supported trigger sequence,
catch-up output compares equal — rows and digest — to a fresh total rebuild. Crash injection
drives failures at labeled points (journal appended / canonical committed / db commit
pending) and asserts a consistent readable state plus correct recovery. One test per
enumerated trigger, including hand-edited JSONL. Replay test drains the same journal twice
and asserts zero row churn. Same-size mutation test (review mandate): rewrite one shard with
equal byte length and an explicitly reset mtime, journal off — and again journal on with no
covering event; both runs must detect the change through hashing, which pins the rule that
metadata alone never licenses a skip for an uncovered family. Interleaving test: rebuild
against catch-up under the lock; assert ordering and a single serial progression.

*Gates.* The equivalence suite must pass before W5 flips any default serving path to cached
mode.

*Complexity L.*

---

**W3 — Operations re-back, stamp semantics, per query operation**

*Goal.* Serve each of the eight operations from the stamped projection. Preserve response
shapes. State exactly what each stamp attests. Ship the contracted defect fixes with the
re-back, not after it.

*Prerequisite inside W3 — dangling-target existence validation (settled by disposal).*
Fact: `IdeationTarget { artifact_type, artifact_id }` reaches proposals, synthesis packets,
and contributions (`core/model/ideation.rs` 275–281), and nothing checks that `artifact_id`
resolves. The existence index covers only the four canonical kinds; topic, question, and
domain have no slot (`store/state_store/canonical_artifacts.rs` key 78–89, `ensure_exists`
53–66; callers `proposal_writers.rs` 363, `ideation_batches.rs` 119 and 232), while
`IdeationTargetType` already carries all seven names including domain (`core/model/ideation.rs`
14–29). Plan: extend the index key to the superset vocabulary landed by W4, check targets at
the three call sites above, and surface misses as typed gap items using the DanglingReference
precedent (`store/cache/gaps/dangling.rs` 7–15 and family). Ideation-tier exposure in served
reads turns on only after this lands.

*Per-operation mapping.* Facts cite current executors.

1. `get`, `search` — become direct row lookups and indexed text predicates. The whole-corpus
    loader leaves the served paths: `records::load` loads and sorts every kind per call
    (`store/operations/queries/records.rs` 12–61) and six operations hit it unconditionally
    (get 80, search 103, neighbors `walk.rs` 72, trace `walk.rs` 107, impact `impact.rs` 26,
    resolve-symbol `symbols.rs` 53). Loader deletion is a separate final milestone (W5
    stage 7), not part of the first flip.
2. `neighbors`, `trace` — walk edge indexes instead of nested scans. Indexes already exist:
    `idx_edges_scope_type_from` and `idx_edges_scope_type_to`
    (`crates/provenance-store/migrations/005_report_indexes.sql` lines 1–2). Trace gains a
    resume token that continues from a depth plus rank-plus-id watermark, replacing the
    mid-breadth cut at `walk.rs` 135–137. Ordering contract promoted to writing: node rank
    then canonical id (`records.rs` 122–131 `rank`, sort at 55–59; trace order via
    `node_order`, `walk.rs` 158–163). Cursor pages must reproduce it bit for bit.
3. `impact` — traversal served from the projection; depth cap unchanged (`TRACE_MAX_DEPTH`,
    `core/protocol.rs` 37; loop bound `impact.rs` 34). New visit budget bounds steps walked;
    new scan budget bounds the repository source scan (`impact.rs` 65 `scan_path(repo)`),
    which stays live because code-owned state is not projected.
4. `resolve_symbol` — hybrid, kept honest: scanned sites stay filesystem-side
    (`store/operations/queries/symbols.rs` 29); canonical implementation and verification
    bindings come from the projection into the same union (union shape at symbols.rs 31–52).
5. `evidence` — implementation and verification bindings come from the canonical shards
    today (`bindings.rs` 13–29; collection filters `evidence.rs` 26–37); after W1 they are
    projected and the re-back reads them from SQLite. Review records come from the
    requirement-reviews shard today (`evidence.rs` 51–58); likewise projected. Verification
    runs stay cache JSONL (`evidence.rs` 38–50). The git-diff stale half is untouched
    (`evidence.rs` 64–81). Paging truthfulness fix lands here: four collections cut
    independently with one merged flag hide which side truncated (`take_page` calls at
    evidence.rs 60–63; OR merge at 85). Response gains per-collection `has_more` flags and
    per-collection cursors. Existing top-level fields stay.
6. `stale` — git machinery only; near-zero re-back. It keeps reading the diff, never the
    working tree, per its own doc comment (`stale.rs` 11–14) and keeps its lock discipline
    (already locks transitively through `health.rs` 59–66).

*Stamp semantics table (review mandate).* The wire stamp carries
`{ serial, digest, policy, attested: [...], live: [...] }`. `attested` names the projection
domains behind the answer; `live` names the constituents the stamp does not cover. A stamp
never implies freshness for a domain it does not list.

| Operation | Projection-attested answer fields | Live constituents not attested |
|---|---|---|
| get | `found`, `node` | none |
| search | `limit`, `has_more`, `nodes` | none |
| neighbors | `id`, `limit`, `has_more`, `neighbors` (edges + nodes) | none |
| trace | `id`, `max_depth`, `limit`, `has_more`, `nodes` with depths | none |
| impact | the reached-rule identities behind `affected_rules`, `limit`, `has_more` | each rule's `implementations`/`verifications` (`node.rs` 133–137): binding rows attested; scanner sites from the working tree not attested (`impact.rs` 65; `sites.rs` 17–58 union) |
| evidence | `implementation_bindings`, `verification_bindings`, `reviews`, `review_required`, `limit` | `verification_runs` and `latest_verification_run` (cache JSONL, `evidence.rs` 38–50); the `stale` half (git diff, `evidence.rs` 64–81) |
| stale | none — every field derives from git ranges and the diff gate (`stale.rs` 30–44) | the whole answer; the stamp names the operation unattested |
| resolve_symbol | canonical binding matches and the Rule records behind them | scanned sites (`symbols.rs` 29–38); the union answer is hybrid, so only the stored halves are attested |

*Consistency decision callout (settled by disposal: snapshot-consistent reads).* Fact: every
sdk query enters through `open`, which builds a StateStore with no publication lock
(`store/operations/queries.rs` 26–30); zero `with_repository_publication` references exist
under `operations/`. Contrast: gap policy locks (`state_adapter.rs` 10) and evidence health
locks (`health.rs` 65). Plan keeps **snapshot-under-publication-lock**: one reader-entry
helper takes the lock, runs the W2 freshness step, and answers from the stamped snapshot;
live-scan halves execute inside the same section. The lock kernel and recovery already carry
48 production call sites. Costs stated plainly (inference, qualitative): reads serialize
against publications; long publishes delay interactive queries; no latency figure exists
in-tree. Code isolates the lock acquisition in one helper so reversal touches one site.

*Contract additivity statement (settled by disposal).* Additive response fields: stamp object,
`next_cursor` where paging extends, per-collection paging flags. Additive request fields:
optional cursor, visit_budget, scan_budget. Requests refuse unknown fields
(`deny_unknown_fields` on every request type, `core/protocol/query.rs` 40–174); optional
additions parse cleanly. Envelope gains fields only; protocol_version, operation, flattened
result stay (`core/protocol/response.rs` 16–33). **Protocol bump flag:** none taken. Version
stays 5 (`SDK_PROTOCOL_VERSION`, `protocol.rs` 25) because every change is
additive-with-default-absent. A bump becomes necessary only if a field must be removed,
renamed, or semantics narrowed; that goes to humans explicitly, never chosen silently.

*Test strategy.*

- Differential harness per operation: old executor versus served executor over the shared
  fixture corpus; serialized JSON equal except additive fields. Runs in CI permanently as
  drift alarm. Old executors live in separate test-only files (W4 lifecycle).
- Order stability property: interleaved inserts and deletes preserve contract ordering.
- Cursor exhaustion loops: page to end on small fixtures; union of pages equals unpaginated
  result; repeating a page request returns identical bytes.
- Stamp truthfulness test per operation: for each `live` constituent named in the table
  above, mutate that constituent alone; the attested fields and digest must stand still.
- Typed-shape updates land additively in the TypeScript layer; the envelope types sit at
  `packages/provenance/src/protocol.ts` 205–223 and the engine dispatches generically
  (`packages/provenance/src/engine.ts` 53), so no per-op SDK logic changes.

*Gates.* Each operation flips its default path only after its differential suite passes.
Until flip, it keeps answering over canonical, as today. No flag day. *Complexity L.*

---

**W4 — One traversal core, Relation vocabulary, Domain/Boundary addressability, GraphQuery collapse, equivalence harness**

*Goal.* One Relation traversal core with two declared provider fronts parameterizes every
traversal. Domain and Boundary gain addressability. The bespoke `GraphQuery` joins collapse
onto the same core. Wiki and gap answers stay byte-identical throughout.

*One traversal core, two fronts (review mandate).* The core executes over a provider seam:
a `RelationSource` trait with exactly two fronts. `RecordFront` serves in-memory record
vectors — the shape `GraphRecords` builds today (`store/cache/gaps/state_adapter.rs` 35–106).
`SqlFront` serves indexed row lookups from `provenance.db`. The eight served operations
consume the `SqlFront`. Wiki and gaps keep the `RecordFront` now, and their convergence onto
SQLite is scheduled, not open: after the served operations prove the `SqlFront` in W5 stage 4,
wiki and gaps move onto the same core through the `SqlFront` in a later stage, with the W4
byte-parity harness as the gate. The core lives beside the vocabulary so both fronts share
one traversal implementation; no operation keeps a private walk.

*Vocabulary design (settled by disposal).* New module `core/model/relations.rs` defining a
closed `RelationKind` enum. Each variant declares endpoint pair, direction semantics, and
derivation tag one of `edge_row | fk_field | embedded_collection`. Seed set comes
exhaustively from what exists:

- Nine edge types (`edge_rank`, `store/operations/queries/walk.rs` 173–184).
- Six foreign-key attachments: `Boundary.requirement_id` (`core/model/shaping.rs` 118),
  `Topic.requirement_id` (130), `Question.topic_id` (147), `Question.requirement_id` (149),
  `Question.resolution_id` (163–168), `Requirement.domain_id` (`core/model/artifacts.rs` 303).
- Embedded reference collections: `Requirement.source_refs` (`artifacts.rs` 304–305 over the
  struct at 275–281) and the `ArtifactLink` lists on Topic and Question (`shaping.rs` 105–110,
  138, 162).
- Ideation target references (`core/model/ideation.rs` 275–281) enter only after W3's
  validation prerequisite lands.

*Structural guard, in the pinned-graph spirit, with explicit no-wildcard gates (review
mandate).* The enum admits no wildcard fallback: traversals match exhaustively, so a family
without a declared variant cannot traverse at all — compile error, not runtime filter. This
copies the mechanism of `projection.rs` 19–23 ("this field list is the rule") onto relations.
Two gates hold the line. First, exhaustion proofs in the existing pattern:
`edge_validation.rs` keeps `all_edge_types` (54) and `all_node_types` (72) helpers and
`#[verifies(..., exhaustion)]` proofs (96–123); `RelationKind` gets the same helpers and
proofs. Second, a source-scan test fails CI when any `match` over `RelationKind` or
`NodeType` in a production crate carries a `_` arm. It also draws the non-goal 424 line in
code: parameters, not predicates; no composition grammar anywhere.

*Superset node vocabulary (settled by disposal).* `NodeType` gains Domain and Boundary
variants (current six at `core/model/graph.rs` 7–20). `GraphNode` mirrors it
(`core/protocol/node.rs` 18–25). `rank` extends (`records.rs` 122–131) and the position pins
by test. Wire names follow the file's parse convention (`graph.rs` 22–36 over
`normalize_enum_value`, `core/model/parsing.rs` 1–3). Cost statement (fact): GetQuery
hard-requires membership in this type (`core/protocol/query.rs` 42–49 — `node_type` has no
default), which is precisely why Domains and Boundaries are unreachable from every served
operation today; widening closes that defect mechanically but touches match sites and
TypeScript types broadly. Expect wide but mechanical edits. The same no-wildcard gate covers
the widened `NodeType`.

*GraphQuery collapse.* The hand-written joins — `resolving_resolutions` (122),
`produced_rules_for_requirement` (150), `producing_requirements` (192),
`missing_rule_producers` (228), `rule_trace_reaches_source` (241),
`requirement_has_valid_source` (247), `source_is_referenced` (261) — reimplement over the
shared traversal core on the `RecordFront`, keeping `GraphRecords::load`
(`state_adapter.rs` 35–106) as the construction front. The embedded-union behaviors pin:
source_refs ∪ References edges in health (`health.rs` 79–99) and topic retirement derivation
(`state_adapter.rs` 65–69).

*Equivalence harness spec (byte-identical mandate, settled by disposal).*

- Corpus: existing gap fixtures (`store/cache/gaps/tests/fixtures.rs`, 183 lines) plus the
  wiki assembler fixtures (`crates/provenance-cli/src/wiki/assemble/tests/fixtures.rs`,
  406 lines) — the review requires wiki fixtures in the byte-parity evidence, since
  `wiki/assemble.rs` 30–60 and `wiki/assemble/context.rs` 9–19 consume `GraphQuery` for
  page output — plus adversarial additions: dangling targets, retired chains, records
  connected by both edge and FK simultaneously.
- Method: move the pre-collapse `GraphQuery` implementations verbatim into a separate
  test-only file (`store/cache/gaps/tests/graph_query_original.rs`), not an inline module.
  Run old and new over identical inputs. Assert byte equality of wiki assembler output and
  of serialized `GapItem` vectors.
- Cadence: CI permanent.
- Lifecycle of the test-only originals (review mandate): each original file carries a header
  naming the operation it preserves and the decision that may remove it. Removal needs its
  own later decision, and cannot land before the scheduled wiki/gaps convergence completes,
  because the harness needs a stable old side until then.

*Tests beyond the harness.* Enumeration completeness (every FK field maps to a declared
variant — compile-time). Serde round-trips for widened NodeType. Rank-order pinning.

*Gates.* Vocabulary and traversal core merge before W3 re-back begins; the superset, the
relation executor, and the `SqlFront` are inputs to it. Collapse deletes bespoke copies only
after the harness runs green on the whole corpus. *Complexity L.*

---

**W5 — Rollout staging, configuration knobs, file-growth gates, MCP handoff**

*Landing order and gates.*

1. W1 merges. Nothing serves from SQLite yet. Update `docs/cache.md` 3–9 so readers learn
   the database now answers queries under a stamp; "never the source of truth" stays true
   and stays written. The same rewrite removes the stale "services, service bindings" words
   from cache.md line 3 (review mandate): migrations `016_drop_rule_code_and_services.sql`
   and `017_remove_services_shards.sql` dropped those tables and shards, and the projection
   stores none.
2. W2 equivalence suite green across consecutive CI runs; catch-up eligible as default
   freshness step. Journal default ON under `catch_up` and `refuse_stale`.
3. W4 part one (vocabulary, traversal core, superset NodeType) merges. CLI docs update where
   node kinds print (`docs/cli.md` 72–132 documents the command surface).
4. W3 flips operations in order: get → search → neighbors → trace → impact → evidence →
   resolve_symbol. Stale last and mostly unchanged. Each flip reverses independently by
   config.
5. Ideation-tier exposure opens per operation only after the dangling-validation
   prerequisite lives.
6. W4 part two completes; bespoke GraphQuery copies move out of production files into the
   test-only originals file.
7. Loader removal milestone (review mandate: corrected). `records::load` leaves the served
   paths flip by flip, but the loader itself is deleted only after the final dependent
   operation flips — its last served callers are impact and resolve_symbol — and test-only
   originals keep their own copies regardless. `GraphRecords::load` stays until the
   scheduled wiki/gaps convergence completes.

*File-growth gates (review mandate; the hard limit is 500 lines per Rust file, tests
included; no CI line-count gate exists today, so this plan makes the discipline binding).*

- `publication.rs` is 476 lines. Journal logic lands in the sibling `publication/journal.rs`
  (W2) before any `publication.rs` edit; `publication.rs` gains only module wiring.
- Catch-up, sweep, and journal-drain logic land as responsibility modules beside the
  orchestrator: `store/cache/materialize/catch_up.rs` and `store/cache/materialize/sweep.rs`,
  matching the existing `materialize/` module split; `materialize.rs` keeps only
  orchestration.
- Trace resume-token handling lands in its own module
  (`store/operations/queries/trace_token.rs`) before token work grows `walk.rs` (185 lines
  today): encode, decode, and request-parameter validation live there.
- Old GraphQuery originals live in separate test-only files (W4), never inline in production
  files.
- Responsibility-based extraction happens BEFORE any proposed change crosses or pressures
  the 500-line boundary; an edit that would push a file past the limit is split first.

*Configuration knobs (settled by disposal, made concrete).*

- `read.freshness_policy`: `catch_up` (default) | `annotate_only` | `refuse_stale`.
  Implemented at one reader-policy module, planned location
  `store/operations/read_policy.rs`. `catch_up` materializes then serves locally.
  `annotate_only` stamps without catching up, for offline use. `refuse_stale` returns a
  typed staleness error naming the gap between stamps — reserved enum member,
  machine-readable, opt-in. Typed refusal also applies when catch-up cannot make the stamp
  current (a failed canonical read, for instance).
- `read.visit_budget` and `read.scan_budget`: defaults fixed at implementation; requests may
  override downward within caps added in W3.
- `cache.catchup_journal`: boolean enabling the write-side event journal. Default follows
  the freshness policy: ON under `catch_up` and `refuse_stale`, OFF under `annotate_only`
  unless set. Off means sweep-driven catch-up with full hashing: simpler, slower steady
  state; allowed for repositories that want fewer moving parts.

*MCP consumer q82f handoff (settled by disposal).* Generation starts when all of these hold:

1. Every operated response carries the stamp with its attested-domain list (W1 plus flips
   complete), so tool descriptions can promise annotated freshness truthfully.
2. Cursor continuation is live for get, search, neighbors, trace — q82f paging designs rely
   on tokens, not just `has_more`.
3. Evidence reports per-collection paging truthfully.
4. Both equivalence harnesses green in CI: op-level differential parity and wiki/gap byte
   parity, wiki fixtures included.
5. Freshness policy documented publicly, so generated tool guidance can cite it.
6. Protocol version confirmed at 5, so clients pin v5 with confidence.

Not blocking q82f: ideation enablement (lands later, gated), budget tuning, journal
switchover, wiki/gaps front convergence. Both remaining policies work regardless of
incremental mode. *Complexity M* (staging itself; cost carried by earlier workstreams).

=== C. OPEN QUESTIONS FOR HUMAN REVIEW ===

Each item blocks a named deliverable. None relitigates settled decisions. Two earlier
questions left this list. The lock-reversal question is gone because the disposal preserves
snapshot-consistent reads. The journal-first ordering question is gone because, under the W2
design, the journal only licenses skips and shard bytes decide truth, so its placement cannot
affect correctness; the plan appends events after the canonical commit, inside the same
locked section.

1. **Stamp representation tie-in.** Open decision #5 upstream owns serial-versus-digest. W1
   stores both columns meanwhile. Confirm dual storage or pick now. Blocks: migration 018
   final shape.
2. **Budget exposure.** Do visit/scan budgets appear as request fields, config only, or
   both? Plan defaults to both. Blocks: W3 contract freeze that q82f depends on.
3. **Domain/Boundary rank slots.** Where the two new members sit in contract ordering
   changes observable page boundaries. Blocks: W4 vocabulary merge.
4. **Evidence cursor shape.** Per-collection tokens returned inline versus one composite
   token. Blocks: W3 evidence flip.
5. **Verification-run storage.** Verification runs live in their own cache JSONL today
   (`evidence.rs` 38–50; `docs/cache.md` 11–14). Keep them outside the projection (plan
   default, matching the volatile state class) or move them in? Moves change the W1 family
   table and the stamp table row for evidence. Blocks: W1 family freeze.
6. **cache.md amendment scope.** Stage 1 rewrites lines 3–9 for stamp semantics and removes
   the stale services and service-bindings wording from line 3. Confirm the rewrite angle
   before W5 stage 1. Blocks: rollout stage 1 docs landing.

=== D. ACCEPTANCE CHECKLIST ===

Pre-existing defects, each mapped to an observable verification. All verifications are
commands or test invocations against fixtures, checked by a human reading output.

| Defect (as evidenced) | Observable verification |
|---|---|
| Results past 200 unreachable: `take_page` truncates once, `has_more` ends the conversation (`core/protocol.rs` 84–88; limit cap 200 at 31; no cursor anywhere — audit finding, based-on doc section 1) | Fixture with 500 matches; loop search/neighbors pages via cursors to exhaustion; concatenated pages equal ground truth listing; repeated page fetch returns identical bytes |
| Trace truncates mid-breadth with no resume (`walk.rs` 135–137) | Wide fixture exceeding limit at depth 2; final resumed walk equals an untruncated run at same max_depth; no duplicate TracedNode across boundary; resume token rejected on mismatched request params |
| Impact work unbounded: depth-10 forced walk plus whole-tree scan (`impact.rs` 34, 65) | Instrumented counters printed in test prove stop at budget; scan budget halves `scan_path` visits on fixture; both knobs accept and reject values per caps |
| Resolve-symbol scans the working tree unbounded (`symbols.rs` 29) | Same scan-budget knob visible on resolve-symbol; counter reports capped visits; union result unchanged for sub-budget runs |
| Evidence hides which collection truncated (`evidence.rs` 60–63, OR merge 85) | Fixture where implementations and reviews both exceed limit; response shows true per-collection flags; each collection paginates to completion independently |
| Domain/Boundary unaddressable (`NodeType` lacks them, `query.rs` 45 requires membership; GetQuery at 42–49) | `sdk get --node-type domain <id>` returns the record; neighbors traversal crosses `Requirement.domain_id`; serde round-trip includes new variants; gap outputs unchanged (harness green) |
| Unlocked reads risk torn views (`queries.rs` 26–30 no lock; contrast `state_adapter.rs` 10) | Concurrency test interleaves a publication with reads; every observed response self-consistent, stamped serial matches snapshot contents; helper logs lock acquisitions |
| Metadata-only sweep misses same-size edits (no mtime movement within timestamp resolution) | Same-size mutation test (W2): journal-off run and uncovered-family run both detect the edit through hashing; a metadata-only comparator demonstrably misses it on the same fixture |
| Wildcard match arms erode the closed vocabularies | Source-scan gate fails when a `_` arm matches `RelationKind` or `NodeType` in a production crate; exhaustion proofs pin variant coverage (`edge_validation.rs` 54–123 pattern) |
| Projection writers race canonical publications (today's unlocked window, `materialize.rs` 27–37) | Interleaving test (W2): rebuild and catch-up serialize under the publication lock; single serial progression; crash injection at labeled points leaves the previous stamped state readable |

=== E. OUT OF SCOPE RESTATED ===

- **Write path untouched.** Change Set, plan, commit, approvals behave as today
  (`writers.rs` call chains). The only addition is journal emission inside the
  already-locked section, in the new `publication/journal.rs` module (W2). No approval
  ceremony changes.
- **Canonical JSONL stays sole truth.** `provenance.db` remains deletable and rebuildable
  (`docs/cache.md` 3–9); loss degrades speed, never correctness.
- **Non-goal 424 binds generated surfaces equally.** q82f tools expose the eight fixed
  operations. The relation vocabulary stays a closed parameterization with no predicate
  grammar; no MCP tool accepts free-form queries.
- **No silent protocol bumps.** v5 persists additively. Removal, rename, or semantic
  narrowing forces an explicit human version decision first (W3 contract statement).
- **Non-goal 429 honored.** Nothing treats the projection as canonical; the served-read
  reversal rests on the approved disposal of bead provenance-1wh, recorded upstream, not
  assumed here. The earlier shards-served authority position is superseded by that disposal.
- **No digest reimplementations.** Canonical serialization has one home after W1;
  `graph_digest` (`export.rs` 30–32) and the projection digest share it.
- Also untouched: search ranking quality, retirement semantics redesign, MCP server
  implementation itself, state-class taxonomy reconciliation, tool-count arbitrage
  (deferred per the disposal).
