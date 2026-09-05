# W3 operations re-back: implementation plan (revision 1)

Bead `provenance-1wh.2`. Read at `577ab96` (PR 180 merged); every file:line below was counted there. This document replaces
the W3 section of `docs/research/2026-08-27-qrspi-1wh-query-uniformity-plan.md` (branch
`opencode/provenance-20260827T223718Z-87cc1ac4`, lines 463-560), which assumed SQLite edge rows and a write journal. The
relation shapes cut (`docs/plans/2026-09-03-relation-shapes-cut.md`, branch `1wh-cut-plan`, section J) replaced edge rows
with owner fields plus a derived `relations` table, and `res_catch_up_hashes_scopes_no_journal` retired the journal. Section
B is the text that goes onto the bead. Settled and not reopened here: the served read path is the projection; freshness
annotates and never refuses; reads snapshot under the publication lock; the relation vocabulary is closed; protocol is 6.

## A. Scope and non-goals

The eight SDK operations (`get`, `search`, `neighbors`, `trace`, `impact`, `evidence`, `stale`, `resolve_symbol`;
`operations/queries.rs:35-105`) answer from the stamped projection in `provenance.db`, one operation at a time, each behind
its own differential gate. Every existing response field keeps its name, type, and order; new fields are additive and
absent when unused. Canonical JSONL stays the only write target (`docs/cache.md:3-10`). The wire adds a stamp, cursors,
budgets, and per-collection paging; it adds no predicate language, and the protocol stays at 6 (`protocol.rs:25`).

Non-goals: wiki and gaps stay on `RecordFront` (`cache/gaps/state_adapter.rs:22-32`, `cache/impact.rs:57-59`); the
`records::load` loader is not deleted (W5 owns that milestone); the `read.*` configuration file surface is W5 (bead
`provenance-1wh.3`); search ranking; ideation rows in `relations` (section H defers them); any MCP tool. Bead
`provenance-qrc.2` ("Migrate SDK graph queries to indexed traversal") describes the neighbors/trace/impact item of this plan
with pre-cut paths (`handlers/sdk/query/walk.rs` no longer exists); it is subsumed here and closes with this bead.

## B. Revised W3 text (the bead text)

*Goal.* Serve each of the eight operations from the stamped projection on the post-cut relation shapes. Preserve response
shapes. State what each stamp attests. Ship the contracted fixes with the re-back.

*Where the code stands.* `queries::open` builds a `StateStore` with no publication lock (`queries.rs:29-33`). Six
operations call `records::load`, which reads and sorts every node kind per call (`records.rs:12-73`; callers `get`
`records.rs:92`, `search` `records.rs:123`, `neighbors` `walk.rs:122`, `trace` `walk.rs:154`, `impact` `impact.rs:27`,
`resolve_symbol` `symbols.rs:53`). Traversals walk `RecordFront` through `related_nodes` and `flow_neighbors`
(`core/model/relations/front.rs:89-133,147-172`). `impact` and `resolve_symbol` call `provenance_scanner::scan_path(repo)`
unbounded (`impact.rs:67`, `symbols.rs:29`; walker at `provenance-scanner/src/walker.rs:78-95`). `evidence` cuts four
collections independently and OR-merges one `has_more` (`evidence.rs:60-63,85`). `trace` cuts mid-breadth
(`walk.rs:183-185`). No cursor exists; `take_page` truncates once (`protocol.rs:84-88`; limit cap 200 at :31, depth cap 10
at :37). The envelope carries `protocol_version`, `operation`, and the flattened result (`protocol/response.rs:16-33`).
Requests refuse unknown fields (`protocol/query.rs:41-175`). The projection holds eighteen families
(`cache/projection_families.rs:25-66`), a derived `relations` table with `idx_relations_out` and `idx_relations_in`
(`migrations/021_relations_table.sql:9-19`), the stamp tables (`018_projection_stamp.sql:45-64`), and nothing in
production reads `relations` yet. The node tables hold column subsets, not records: `requirements` has `scope_id, id,
statement, status, domain_id, fog` (`002:14-20`, `009:1`, `010:1`), `rules` has five columns (`003:17-27`, `015`, `016`);
none carries `retired`, `description`, `source_refs`, or the other fields `GraphNode` serializes whole
(`protocol/node.rs:16-27`). Section C adds the derived `nodes` table so an answer can be hydrated from the projection.

*Per-operation mapping.*

1. `get`, `search`: one row lookup and one ordered scan over `nodes` (C). `include_retired` reads the `retired` column.
   `search` keeps its text semantics (`node.rs:74-103`, `records.rs:126-130`) and the protocol-five default kinds
   (`records.rs:142-149`); it gains a keyset cursor (F).
2. `neighbors`, `trace`: walk the derived `relations` table (`idx_relations_out`, `idx_relations_in`) through `SqlFront`
   (C). Filters name relations from the declared vocabulary (thirteen names, `relations.rs:73-83`; refusal at
   `walk.rs:198-208`), not edge types; protocol 6 already carries this. Trace gains a resume token (F). Ordering: node rank,
   canonical id, declaration order, direction (`front.rs:123-131`; trace by depth first, `walk.rs:174,210-215`).
3. `impact`: walks downstream over `relations`, direction derived from each declaration's `flow` (`decl.rs:15-19`,
   `front.rs:147-172`), with the same depth cap (`impact.rs:37`). Impact is atomic and node-specific (owner ruling
   2026-09-05): a record's impact is what the declared flows reach from it; no synthetic hop is added, so a resolution does
   not reach its requirements' rules through `requirement_ids`. A visit budget bounds rows walked and a scan budget bounds
   `scan_path` (G). The `INDIRECT` filter of the `impact` command (`cache/impact.rs:41-47`) loses its dead `contradicts`
   entry (I).
4. `resolve_symbol`: stays hybrid. Scanned sites come from the working tree (`symbols.rs:31-38`); binding matches and Rule
   records come from the projection (`implementation_bindings`, `verification_bindings`, `nodes`). The scan budget applies.
5. `evidence`: `implementation_bindings`, `verification_bindings`, `reviews`, `review_required` come from the projection
   (`018:1-43`); verification runs stay cache JSONL (`state_store/verification_runs.rs:153-167`; `docs/cache.md:83-87`);
   the `stale` half stays a git diff (`evidence.rs:64-81`). Paging becomes truthful: per-collection `has_more` and cursors
   (F).
6. `stale`: git machinery only; near-zero re-back. It reads the diff, never the working tree (`stale.rs:11-14`), and keeps
   reading graph evidence from canonical shards (`stale.rs:36-40` through `cache/health.rs:59-66`). Its stamp attests
   nothing.

*Stamp.* Every answer carries `stamp` (D). Protocol stays 6; W3 adds no bump.

*Consistency.* One reader entry takes the publication guard, runs catch-up as the freshness step, and answers from the
projection at the stamped serial; live halves run inside the same guard scope (E).

*Gates.* Each operation flips only after its differential suite is green (J). No flag day. The flip order is get, search,
neighbors, trace, impact, evidence, resolve_symbol; stale last and mostly unchanged (K).

## C. `SqlFront` and the derived `nodes` table

*The trait stays the seam.* `RelationSource` has two methods, `outgoing` and `incoming`, both synchronous
(`front.rs:57-65`). `related_nodes` and `flow_neighbors` are the one traversal core; they sort after collecting
(`front.rs:123-131`), so a front supplies rows and the core owns the order. `SqlFront` is the second front. It implements
the same trait, so `related_nodes`, `flow_neighbors`, and the executors' filters (`walk.rs:86-103`) run unchanged over it.
No operation gets a private walk.

*Shape.* sqlx is async and the trait is not. `SqlFront` is therefore a fetched hop: `async fn SqlFront::hop(pool, scope,
frontier: &[(NodeType, StableId)]) -> HopRows` runs two indexed queries per hop, `WHERE scope_id = ? AND owner_type = ?
AND owner_id IN (...)` over `idx_relations_out` and `WHERE scope_id = ? AND target_type = ? AND target_id IN (...)` over
`idx_relations_in`, grouped by frontier kind, and `HopRows` implements `RelationSource` over the fetched rows keyed by
`(node_type, id)`. A lookup for an id outside the fetched frontier is an invariant violation: `debug_assert!` plus a test.
`neighbors` is one hop; `trace` and `impact` fetch one hop per depth from the reached frontier. Rejected alternative: load
the scope's whole `relations` table per request into an in-memory front. Simpler, but O(rows in scope) per request and no
index use; it would make W3 a slower copy of `RecordFront`. Rejected: `block_in_place` inside the synchronous trait, which
panics on the current-thread runtime every `#[tokio::test]` uses.

*Node hydration.* The projection cannot answer a `GraphNode` today (B). Migration `022_nodes_table.sql` adds a derived
table: `nodes(scope_id, node_type, id, rank, retired, search_text, record, PRIMARY KEY (scope_id, node_type, id))`,
`idx_nodes_order(scope_id, rank, id)`, `idx_nodes_id(scope_id, id, rank)`. `record` is the record's JSON as the state store
serializes it; `rank` is `NodeType::rank` (`graph.rs:30-40`); `retired` follows `GraphNode::retired` (`node.rs:60-71`, false
for the five kinds that never retire); `search_text` is the lowercased `searchable_text` pieces joined by `\u{1}`. Like
`relations`, `nodes` is derived from the eight node families, has no digest row, and is rebuilt with its owner families:
`materialize.rs:71` and `catch_up.rs:211-237` gain a `nodes` reload beside `relation_rows`, with a `NODE_OWNERS` list of
eight beside `RELATION_OWNERS` (`catch_up.rs:198-206`), and `clear_cache` (`materialize.rs:97-107`) clears it. The same
migration adds a `record TEXT NOT NULL DEFAULT ''` column to `implementation_bindings`, `verification_bindings`, and
`requirement_reviews` (`018:1-43`), filled by `materialize/integration_records.rs`, so `evidence` and `resolve_symbol`
hydrate bindings and reviews whole (`schema_version` is not a column today). Catch-up rebuilds after a migration
(`catch_up.rs:55-57`), so an existing database converts itself on the first read. `kind_of` (`walk.rs:81-83`, the first
kind in rank order that holds an id) becomes one query over `idx_nodes_id`. Rejected alternative: a `record` column on each
of the eight node tables. It spreads hydration over eight loaders and cannot order `search` across kinds with one index.

*Ordering.* Neighbors: the core's sort (`front.rs:123-131`) runs over `HopRows` exactly as over `RecordFront`; SQL does
no ordering. Trace: depth, then rank, then id (`walk.rs:174`). Search and get: `ORDER BY rank, id` over `idx_nodes_order`,
which equals `records::load`'s sort (`records.rs:67-71`). `search` prefilters in SQL with `instr(search_text, ?) > 0` over
the joined string (a superset, because a needle can span two pieces) and applies the exact per-piece `contains` in Rust
(`records.rs:126-130`) before counting toward the page.

*Equivalence gate between the fronts.* A property test in `core/model/relations/tests/` (new) and its store twin build one
scope, materialize it, and assert `related_nodes` over `RecordFront` equals `related_nodes` over `SqlFront` for every
record in the scope; the same for `flow_neighbors` in both directions. It runs over the seeded store
(`operations/queries/tests.rs:16-68`), the gap fixtures (`cache/gaps/tests/fixtures.rs`), and the repository's own state.

*Files (500-line cap, `AGENTS.md:20`).* New `crates/provenance-store/src/cache/read/` with `mod.rs` (pool and stamp
read), `front.rs` (`SqlFront`, `HopRows`), `nodes.rs` (hydration, `kind_of`, ordered scan), `search.rs` (prefilter and
keyset page). `materialize/node_rows.rs` (new, the `nodes` loader, beside `relation_rows.rs`). `walk.rs` (215) keeps
`neighbors` and `trace`; token handling goes to `queries/trace_token.rs` before `walk.rs` grows.

## D. Stamp semantics

*Wire shape.* `QueryResponse` (`response.rs:16-33`) gains one field, serialized after `operation`:

```
"stamp": { "serial": 41, "digest": "sha256:...", "instance_id": "uuid", "derivation": 1,
           "policy": "catch_up", "attested": ["relations", "nodes"], "live": [] }
```

`serial` and `digest` are the latest `projection_revision` row (`018:50-54`; written at `stamp.rs:56-67`); `instance_id`
is the `projection_instance` row (`018:45-48`, `stamp.rs:24-27`). `policy` is the freshness policy the reader ran (E).
`attested` names the projection tables behind the answer, in the family words of `projection_families.rs:69-90` plus the
derived words `relations` and `nodes`. `live` names the constituents the stamp does not cover, from a closed list:
`canonical` (read from canonical shards, not the projection), `scanned_sites` (working-tree scan), `verification_runs`
(cache JSONL), `diff` (git). A stamp never implies freshness for anything it does not list. The TypeScript `QueryEnvelope`
(`protocol.ts:212-215`) gains `stamp?: Stamp`; the engine dispatches generically (`engine.ts:20-28,47-53`) and needs no
per-operation change.

*Open question 7, instance id on the wire: yes.* The stored id already exists and `docs/cache.md:15-17` already says
serials compare only within one instance; a client that compares serials without the id compares across a rebuild from
total cache loss (`docs/cache.md:45`) and orders stamps wrongly. Putting the id on the wire costs one string and makes the
existing rule checkable by the client. Alternative: leave the id off and state the rule in the client contract only. It
is cheaper by one field and cannot be checked; not recommended.

*Derivation version.* `pub const READ_DERIVATION: u32 = 1` in `operations/stamp.rs` (new). It bumps when reader logic
changes an answer for the same projection rows: traversal order, a filter's semantics, hydration, cursor encoding, budget
accounting. It does not bump for a migration (the digest and serial move then) or for a fix in a live half (not attested).
A golden test pins it: `operations/queries/tests/golden.rs` answers a fixed request set over the seeded store and the
repository's own state, digests the answers with the stamp removed, and compares to a committed file keyed by
`READ_DERIVATION`. A change in served bytes fails the test until the constant bumps and the file regenerates. Cursor
tokens carry the derivation (F), so a page minted under older logic is refused rather than continued.

*Per-operation table.* Before an operation flips (K), its row reads `attested: []`, `live: ["canonical"]`, which is
truthful and shows the flip state on the wire.

| Operation | Attested fields (tables) | Live constituents |
|---|---|---|
| get | `found`, `node` (`nodes`) | none |
| search | `limit`, `has_more`, `next_cursor`, `nodes` (`nodes`) | none |
| neighbors | `id`, `limit`, `has_more`, `next_cursor`, `neighbors` (`relations`, `nodes`) | none |
| trace | `id`, `max_depth`, `limit`, `has_more`, `next_cursor`, `nodes` with depths (`relations`, `nodes`) | none |
| impact | the reached rule identities and page (`relations`, `nodes`); binding rows in `implementations` and `verifications` (`implementation_bindings`, `verification_bindings`) | `scanned_sites` in the same two lists (`sites.rs:26-33,51-62`) |
| evidence | `implementation_bindings`, `verification_bindings`, `reviews`, `review_required`, `limit`, `paging` (`implementation_bindings`, `verification_bindings`, `requirement_reviews`) | `verification_runs`, `latest_verification_run` (`verification_runs`); `stale` (`canonical`, `diff`) |
| stale | none | `canonical` (`stale.rs:36-40`), `diff` |
| resolve_symbol | binding matches and the Rule records (`implementation_bindings`, `verification_bindings`, `nodes`) | `scanned_sites` (`symbols.rs:31-38`) |

## E. Reader entry

One helper, `operations/reader.rs` (new): `async fn answer<R>(repo, scope, policy, run) -> (R, Stamp)`.

1. Take the guard: `publication::publication_guard(layout).await` (`publication/guard.rs:68-83`). Under read-only
   validation it holds no lock (`guard.rs:70-72`).
2. Freshness step under the policy. `catch_up`: `catch_up_with_guard(&guard, layout)` (`catch_up.rs:37`, made
   `pub(crate)`). It runs migrations, rebuilds when the database has no revision or a migration applied, snapshots the
   state tree, validates, hashes every unit, re-derives changed families, and commits one revision or none
   (`catch_up.rs:41-123`). `annotate_only`: no catch-up; the stamp reports the stored serial. `refuse_stale`: reserved,
   refused as unimplemented in W3 with a typed error; W5 implements it.
3. Open the pool (`cache.rs:34-39`), read the stamp rows, and run `run(&ReadContext)` where the context carries the pool,
   the scope, the snapshot layout, the budgets, and the policy. All projection reads of one answer happen while the guard
   is held, so no projection write can interleave (`docs/cache.md:55-59`).
4. Live halves run inside the same scope: `scan_path` reads the working tree (`impact.rs:67`, `symbols.rs:29`); git reads
   the object store (`stale.rs:30-43`); verification runs read cache JSONL (`verification_runs.rs:153-167`). Trap, stated
   at `guard.rs:7-15`: a synchronous `with_repository_publication` section entered while the guard is held blocks on the
   lock. `stale::disturbed` enters one through `cache::graph_evidence` (`stale.rs:36`, `health.rs:65`) with the repository
   layout. Under the reader entry it must take the snapshot layout (`snapshot.layout()`, the rule at `guard.rs:14-15`), so
   `disturbed` gains a layout parameter and the entry passes the snapshot's. The guard interleaving test (J) pins this.
5. Drop the guard; print.

*Policy and knobs.* `operations/read_policy.rs` (new): `enum FreshnessPolicy { CatchUp, AnnotateOnly, RefuseStale }`,
`struct ReadPolicy { freshness, visit_budget, scan_budget }`, defaults `CatchUp`, 10000, 5000. The names reserved for the
configuration file are `read.freshness_policy`, `read.visit_budget`, `read.scan_budget`. No configuration file reader
exists anywhere in the tree today (no `config` module in `provenance-cli/src` or `provenance-store/src`); W3 ships the
struct and its defaults as the seam, and bead `provenance-1wh.3` wires the file. There is no journal knob.

*Async.* `queries::get` and the other seven become `pub async fn`; `handlers/sdk/query.rs:24-64` and `handlers/sdk.rs:13`
become async (the binary is already on tokio, `main.rs:16-21`). The CLI adapter stays thin.

*Cost, stated plainly.* Reads serialize against publications and against each other: a long publish delays a query, and
two queries queue. Under `catch_up` every read copies the state tree (`snapshot_state_under_guard`, `catch_up.rs:59`,
`guard.rs:94-99`) and hashes every canonical byte (`catch_up.rs:86-97`); on this repository that is 731 records. A fresh
clone with no database rebuilds on the first read. No latency figure exists in the tree; the guard test suite prints
none. Reversal touches one site: `reader.rs` is the only caller of the guard on the read path, and an operation not yet
flipped answers over canonical inside the same helper, so a policy of `annotate_only` plus an unflipped operation is
today's behaviour with a stamp.

## F. Cursors

*Token.* `operations/cursor.rs` (new): a `Cursor { operation, derivation, params_digest, serial, instance_id, after }`
serialized as canonical JSON (`canonical_digest::canonical_bytes`, `canonical_digest.rs:15`) and encoded URL-safe base64
without padding (new workspace dependency `base64`; the workspace has `sha2`, `uuid`, `serde_json`, no encoder). Requests
gain `cursor: Option<String>`; responses gain `next_cursor: Option<String>`, absent when `has_more` is false. `params_digest`
is `sha256` over the request with `cursor`, `limit`, and `protocol_version` removed, so a token continues one request.

*Refusal.* A token that does not decode, names another operation, another `params_digest`, or another `derivation` is
refused: "cursor was not minted for this request" naming which part mismatched. A token minted at another serial or
instance is accepted and the page answered; the stamp shows the move, and the page is bit-for-bit reproducible only at the
serial that minted it. Freshness annotates, never refuses.

*Keysets.* `search`: `after = (rank, id)`; the scan continues `WHERE (rank, id) > (?, ?)` over `idx_nodes_order`, in
chunks of `limit + 1`, applying the Rust text filter, until `limit + 1` matches. `neighbors`: `after = (rank, id,
relation, direction)` because two relations can reach one record (`requirement_ids` and `links`); the sorted hop list is
skipped past the watermark. `resolve_symbol` and `impact`: `after = rule id` (both order by rule id, `symbols.rs:54-63`,
`impact.rs:61-65`). `get` and `stale` take no cursor.

*Trace resume token.* `after = (depth, rank, id)`. Resume re-runs the walk from the origin to `max_depth` (ten hops at
most, one indexed hop per depth) and emits nodes after the watermark in (depth, rank, id) order; the token carries no seen
set, so its size is fixed. The mid-breadth cut at `walk.rs:183-185` goes; `reached` is walked to completion and paged by
watermark. A token whose `params_digest` disagrees on `id`, `node_type`, `direction`, `relations`, `max_depth`, or
`include_retired` is refused as above. Module: `queries/trace_token.rs`.

*Open question 4, evidence cursors: per-collection inline tokens.* Request gains `cursors: { implementation_bindings?,
verification_bindings?, verification_runs?, reviews? }`; response gains `paging: { <collection>: { has_more, next_cursor? }
}` for the four collections, and the top-level `has_more` keeps its OR meaning (`evidence.rs:85`) for old readers.
Keysets: bindings and reviews by id (`bindings.rs:21,24`, `evidence.rs:56`); runs by `(started_at desc, id desc)`
(`evidence.rs:43-48`). Reason: the four collections exhaust independently, and a client paging reviews must not re-fetch
finished bindings; a composite token advances every collection in lockstep and cannot say "reviews only". Alternative:
one composite `cursor` holding four watermarks. It keeps the request shape uniform with the other operations, and that is
its only merit; not recommended.

*Reproduction.* Cursor-exhaustion tests (J) page each operation to the end on a fixture larger than the limit, assert the
union equals the unpaginated answer at limit 200, that no record repeats across pages, and that a repeated page request
returns identical bytes.

## G. Budgets

*Open question 2: both.* `visit_budget` and `scan_budget` are optional request fields on `impact` and `resolve_symbol`
(`scan_budget` only on the latter) and capped by `ReadPolicy` (E). A request above its cap is refused like `ensure_limit`
(`protocol.rs:66-72`). Absent, the cap applies.

*What each bounds.* `visit_budget`: relation rows read during the impact walk, counted in `HopRows` as they are handed to
the core; when the count would pass the budget, the current depth finishes its fetched rows and no further hop is fetched.
`scan_budget`: files the scanner reads. `provenance-scanner` gains `scan_path_bounded(path, max_files) -> (Vec<FileScan>,
bool)` beside `scan_path` (`walker.rs:78-83`); the walk stops after `max_files` regular files in the walker's order.

*Observable.* A cut is never silent. Both responses gain `budgets: { visit: { limit, used, exhausted }, scan: { limit, used,
exhausted } }` (impact carries both; resolve_symbol carries `scan`). `exhausted: true` says the answer is a lower bound.
Tests read `used` from the response, not from a log.

## H. Dangling-target existence validation for ideation targets

`IdeationTarget { artifact_type, artifact_id }` (`core/model/ideation.rs:276-281`) names seven kinds
(`IdeationTargetType`, `ideation.rs:14-29`: the six graph kinds plus domain; no boundary). Contributions and synthesis
packets write it unchecked (`state_store/ideation_writers.rs:32,53,151,166`; inputs at `state_store/inputs.rs:271,289`).
The existence index covers four kinds (`state_store/canonical_artifacts.rs:17-51`, key at :78-89) and is called only for
dispositions' `canonical_artifact` (`proposal_writers.rs:363`, `ideation_batches.rs:119,232`).

*Plan.* Extend `CanonicalArtifactIndex` to every `NodeType` (eight kinds), keyed by the serde word; add
`ensure_target_exists(&IdeationTarget)` mapping `IdeationTargetType` onto `NodeType`; call it at the four writer sites and
from `validate_graph_scope` (`state_store/graph_validation.rs`, the cut's sibling validator) so `check`, `materialize`, and
catch-up refuse a new dangling target. Existing state is reported, not refused: a gap pass `cache/gaps/ideation_targets.rs`
(new) emits `GapKind::DanglingReference` (`cache/gaps/model.rs:18`) with the wording of `dangling.rs:52-56`, "target
points at missing <kind> <id>", over contributions and synthesis packets, which `GraphRecords` (`state_adapter.rs:22-32`)
gains as three lists. `IdeationTargetType` gains `Boundary` so the superset is complete.

*Exposure: deferred.* Ideation and thread rows do not enter `relations` in this bead (cut plan L12). The gate for the
later bead: this validation merged; `provenance gaps` reports no dangling target on the repository's own state; and an
owner decision on the relation name the rows carry, because the thirteen-name vocabulary printed at `docs/cli.md:138-142`
is closed and a new name is a vocabulary change. Until then, served reads never reach an ideation record through a walk.

## I. PR 180 leftovers assigned to W3

1. `INDIRECT` dead entry (`cache/impact.rs:41-47`). `contradicts` is declared `flow = none` (`shaping.rs:176`), so
   `flow_neighbors` never yields it (`front.rs:168`) and the filter never sees it. Decision: delete the entry; add a test
   that every `INDIRECT` name resolves through `declaration_for` to a declaration whose flow is not `None`, so the list
   cannot hold a dead name again. Alternative: keep it as documentation; rejected, the declaration table is the document.
2. Cut plan L3: `is_resolved` (`cache/gaps/contradiction.rs:37-48`) treats any `resolution_id` as settling the pair,
   including a rejected resolution. Fix: the pair is settled only when the named resolution exists in the scope and its
   status is not `Rejected` (`ResolutionStatus`, `core/model/artifacts/kinds.rs:94-108`); `supersedes` unchanged.
   RED test: a question naming a rejected resolution still reports `UnresolvedContradictsPair`.
3. Cut plan L5: the neighbors order is rank, id, declaration order, direction (`front.rs:123-131`). The cursor watermark
   freezes it (F); a rank-order pin test and the golden file (D) make a reorder a visible derivation bump.
4. `contradicts` shape on the filter: pre-cut the relation joined two requirements; now a question owns it
   (`state-format.md:32-34`), so `neighbors` of a requirement with `relations: ["contradicts"]` answers the question `in`,
   and the other requirement is one more hop. Documentation only: one sentence in `docs/cli.md` after line 146. No code.

## J. Test strategy

*Oracle.* Before any flip, today's executors are copied verbatim into `operations/queries/tests/oracle/{records, walk,
impact, evidence, symbols}.rs` under `#[cfg(test)]`, each with a two-line header naming the operation it preserves and
"deleted in the last W3 commit". They read canonical shards, as today.

*Differential harness.* `operations/queries/tests/differential.rs`: for each operation and a request set, run the oracle
and the served executor over the same store, serialize both to `serde_json::Value`, strip the additive fields (`stamp`,
`next_cursor`, `paging`, `budgets`), and assert equality. Corpus: the seeded store (`queries/tests.rs:16-68`), the CLI
fixtures (`tests/query_support/fixtures.rs:28-42`), the gap fixtures, and the repository's own `.provenance/state` (the
in-tree tests already export it). Each operation flips only when its rows are green. The oracle and the harness are
deleted together in the last commit of PR 3; from then the golden file (D) is the drift alarm.

*Order stability.* A property test inserts and retires records between two identical requests and asserts the surviving
records keep their relative order in `search`, `neighbors`, and `trace`.

*Cursor exhaustion.* Per F: a scope with 500 matching records (a builder in the pattern of `provenance-cli/src/wiki/fixtures_scale.rs`) paged to the
end for `search`, `neighbors`, `trace`, `impact`, `resolve_symbol`, and each `evidence` collection; union, no duplicate,
identical bytes on repeat; forged and mismatched tokens refused with the named part.

*Stamp truthfulness, one test per live constituent.* Mutate the constituent alone and assert the attested fields and the
stamp's serial and digest stand still: add a scanner annotation in the working tree (`scanned_sites`); append a run to
`verification-runs.jsonl` (`verification_runs`); make a commit that touches a bound file (`diff`); for `stale`, edit a
canonical shard and assert the answer changes while the stamp lists `canonical`. A second test per flipped operation
edits a canonical shard and asserts the serial advances by one under `catch_up` and stands still under `annotate_only`.

*Guard interleaving.* The `test_probes` pattern (`test_probes.rs:23-40`; `materialize_guard_behavior.rs:22-38`): arm a
probe inside the reader entry that asserts the lock is held, then a probe that starts a canonical write on another thread
and asserts it waits until the answer prints; a third runs `evidence` with `base` under the guard and asserts it returns
(the `stale.rs:36` trap in E).

*Fronts.* The `RecordFront` versus `SqlFront` equivalence property (C), and the catch-up dumps compare the `nodes` table
as they compare `relations` (`catch_up_behavior.rs:44-56`, `relation_rows_behavior.rs:11-22`).

*TypeScript.* `protocol.ts` gains `Stamp`, `stamp?`, `cursor?`, `next_cursor?`, `cursors?`, `paging?`, `budgets?`; the
type tests compile an old-shape response against the new types, and the runtime suite reads one stamped answer.

*RED first, per commit.* Named in K.

## K. Delivery

Three PRs, each green at every commit (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features
-- -D warnings`, `cargo test --workspace`, `npm test`), each with the deslop pass before ready: plain short comments, no
review residue, PR body in plain English with ledgers under one collapsed block.

*PR 1, branch `1wh-w3-reader`: reader entry and stamp; nothing served from SQL yet.*
1. Oracle copies and the differential harness over the current executors (green against themselves).
2. `read_policy.rs`, `stamp.rs` with `READ_DERIVATION`, `cursor.rs` (encode, decode, refusal); unit tests RED first:
   `a_cursor_minted_for_another_request_is_refused_by_name`, `a_cursor_from_another_derivation_is_refused`.
3. `reader.rs`; the eight operations become async and answer through it over canonical, stamped `attested: []`, `live:
   ["canonical"]`; `stale::disturbed` takes the layout. RED: `every_answer_carries_a_stamp_at_the_stored_serial`,
   `a_read_holds_the_publication_lock_while_it_answers`, `a_canonical_write_waits_for_a_read_to_finish`,
   `evidence_with_a_base_answers_under_the_guard`, `a_canonical_edit_advances_the_serial_under_catch_up`.
4. Golden file and `READ_DERIVATION` test; TypeScript `Stamp` types. Green: identical test count for the eight operations
   plus the new ones; every existing response byte unchanged apart from `stamp`.

*PR 2, branch `1wh-w3-nodes`: `nodes`, `SqlFront`, get through trace.*
1. Migration 022, `node_rows.rs`, record columns, catch-up reload, `clear_cache`; RED:
   `materialize_derives_one_node_row_per_record`, `catch_up_reloads_nodes_when_an_owner_family_moves`,
   `a_binding_row_hydrates_to_the_record_it_came_from`.
2. `cache/read/{front, nodes, search}.rs`; the front equivalence property RED over `RecordFront` versus `SqlFront`.
3. Flip `get`; RED: differential rows for get, `get_reads_a_retired_record_only_when_asked` over the `retired` column.
4. Flip `search` with the keyset cursor; RED: differential rows, `search_pages_to_exhaustion_without_a_duplicate`.
5. Flip `neighbors` with its cursor; RED: differential rows, order pin, `neighbors_pages_a_record_reached_twice_once_each`.
6. `trace_token.rs`, flip `trace`; RED: differential rows, `a_resumed_trace_equals_an_uncut_walk`,
   `a_trace_token_with_another_depth_is_refused`. Stamp rows for the four flip to their attested tables.

*PR 3, branch `1wh-w3-evidence`: impact, evidence, resolve_symbol, leftovers, oracle removal.*
1. `scan_path_bounded`, budgets in `ReadPolicy` and the requests; RED: `a_scan_budget_reports_exhaustion_and_its_count`.
2. Flip `impact` with the visit budget and cursor; RED: differential rows, `impact_stops_at_the_visit_budget_and_says_so`,
   `an_indirect_name_is_a_declared_relation_with_a_flow` (I.1).
3. Flip `evidence` with per-collection paging; RED: differential rows,
   `evidence_reports_which_collection_was_cut`, `each_evidence_collection_pages_to_completion_alone`.
4. Flip `resolve_symbol`; RED: differential rows, `resolve_symbol_pages_rules_by_id`.
5. Stamp truthfulness tests per live constituent (J); `is_resolved` fix (I.2) RED:
   `a_rejected_resolution_does_not_settle_a_contradiction`.
6. Ideation target validation (H); RED: `a_contribution_naming_a_missing_target_is_refused`,
   `an_existing_dangling_target_is_a_gap_not_a_refusal`, `check_refuses_a_dangling_ideation_target`.
7. Delete the oracle and the harness; docs: `docs/cli.md` (stamp, cursors, budgets, paging, the `contradicts` sentence),
   `docs/cache.md` (the `nodes` derived table in the family table at line 63-73, the read path paragraph at 3-10);
   this file's section B onto the bead as its text; close `provenance-qrc.2` as subsumed.

*File-cap splits before growth.* `walk.rs` (215): token code in `trace_token.rs`. `evidence.rs` (94): the four keysets in
`queries/evidence_paging.rs`. `catch_up.rs` (273): the `nodes` reload is one function beside the `relations` one, no more.
`protocol/query.rs` (175) and `response.rs` (116): the stamp, cursor, paging, and budget types go in `protocol/stamp.rs`
and `protocol/paging.rs`, re-exported from `protocol.rs`. `protocol.ts` (399): new types append; split into
`protocol/query.ts` if it passes 500.

## L. Risks and open points

1. Per-read tree copy and full hash under `catch_up` (E). Default: accept in W3, correctness first; W5 measures and, if a
   read on this repository exceeds one second, moves catch-up to hash in place under the guard and snapshot only when a
   unit moved.
2. The synchronous lock trap (`guard.rs:7-12`). Any live half that calls `with_repository_publication` with the
   repository layout deadlocks under the reader entry. Default: the interleaving test runs every operation with every
   optional field set; `stale::disturbed` is the one known site and takes the snapshot layout.
3. `search` text equality across the SQL prefilter (C). Default: the prefilter is a superset by construction and the Rust
   filter decides; the differential harness and the golden file pin it.
4. `nodes.record` doubles storage for the eight kinds. Default: accepted; the database is derived and rebuildable.
5. Cursor tokens are unsigned. A forged token with a matching `params_digest` yields a valid page from a watermark the
   client chose; nothing leaks that a request without the token could not read. Default: no signature.
6. Trace resume re-walks from the origin on every page. Default: accepted; ten hops of indexed fetches, no seen set on
   the wire. If page latency matters, W5 may add a per-serial memo.
7. `refuse_stale` is reserved and refused as unimplemented. Default: W5 implements it; the enum member exists so the
   stamp's `policy` word is stable.
8. Impact under the atomic ruling versus the flow walk. The walk is multi-hop over declared flows and stays so; the ruling
   forbids synthetic hops, not depth. Default: as written in B.3; if the owner meant one hop, the change is the loop bound
   at `impact.rs:37` and one line of this plan.
9. Ideation exposure deferred (H). Default: the gate named in H; no walk reaches an ideation record in this bead.
10. Budget defaults (10000 rows, 5000 files) are guesses. Default: request fields override downward; W5 tunes the caps
    after a measurement over this repository.
11. `IdeationTargetType` gains `Boundary`. A `Boundary` target is then writable through the ideation commands. Default:
    accepted, the superset is what the existence check keys on; the parse list (`ideation.rs:32-43`) grows by one.
12. The flip of an operation is a code path, not a runtime switch. Default: each flip is one commit that reverts cleanly;
    the stamp's `attested: []` row is how a reverted operation is seen on the wire.

*Old citations not verifiable at `577ab96`.* `idx_edges_scope_type_from` and `005_report_indexes.sql:1-2` (dropped by 021);
`edge_rank` at `walk.rs:173-185` (deleted); `handlers/sdk/query/walk.rs` named by `provenance-qrc.2` (the executors live
under `provenance-store/src/operations/queries/`); `dangling.rs 7-15` as the `DanglingReference` precedent (the kind is at
`gaps/model.rs:18`, the generic pass at `dangling.rs:36-65`); the old plan's three ideation call sites check dispositions'
`canonical_artifact`, not `IdeationTarget`, whose write sites are in `ideation_writers.rs`.
