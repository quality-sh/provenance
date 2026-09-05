# W3 operations re-back: implementation plan (revision 3)

Bead `provenance-1wh.2`. Read at `577ab96` (PR 180 merged); every file:line below was counted there. This document replaces
the W3 section of `docs/research/2026-08-27-qrspi-1wh-query-uniformity-plan.md` (branch
`opencode/provenance-20260827T223718Z-87cc1ac4`, lines 463-560), which assumed SQLite edge rows and a write journal. The
relation shapes cut (`docs/plans/2026-09-03-relation-shapes-cut.md`, branch `1wh-cut-plan`, section J) replaced edge rows
with owner fields plus a derived `relations` table, and `res_catch_up_hashes_scopes_no_journal` retired the journal.

Revision 2 folded Ben's rulings of 2026-09-05, four resolutions under `req_query_answers_carry_a_freshness_stamp` on branch
`1wh-w3-rulings` (PR 191): `res_query_answers_stop_at_the_limit` (no paging; one `has_more` per evidence list; a configured
scan limit, not a request field; boundary `boundary_query_answers_do_not_page`), `res_impact_follows_declared_flow` (flow
direction, ten steps, no synthetic step), `res_projection_tables_mirror_record_types` (one column per record field, a
derive, a round-trip gate; revision 1's whole-record JSON column rejected), and `res_stamp_names_projection_instance`
(instance id on the wire). Revision 3 folds the two adversarial reviews of revision 2 (Fable: one blocker, five major,
fourteen minor; GLM: one blocker, five major, eight minor; every finding taken, the choices between them in L). Section B is
the bead text. Settled and not reopened: the served read path is the projection; freshness annotates and never refuses;
reads snapshot under the publication lock; the vocabulary is closed; protocol is 6.

## A. Scope and non-goals

The eight SDK operations (`get`, `search`, `neighbors`, `trace`, `impact`, `evidence`, `stale`, `resolve_symbol`;
`operations/queries.rs:35-105`) answer from the stamped projection in `provenance.db`, one operation at a time, each behind
its own differential gate. Every existing response field keeps its name, type, and order; new fields are additive and absent
or false when unused. Canonical JSONL stays the only write target (`docs/cache.md:3-10`). The wire adds a stamp, four
per-list `has_more` flags on `evidence`, and a scan flag on `impact`. It adds no cursor, no resume token, no request budget
(`boundary_query_answers_do_not_page`), no predicate language, and no protocol bump (`protocol.rs:25`). The 200 cap and
`has_more` stay exactly as today (`protocol.rs:31,84-88`). Two served answers change on purpose and are listed as the first
entries of the derivation history (D): one neighbor per (relation, direction, endpoint), and a trace `seen` set keyed by
kind and id.

Non-goals: wiki and gaps stay on `RecordFront` (`cache/gaps/state_adapter.rs:22-32`, `cache/impact.rs:57-59`); the
`records::load` loader is not deleted (W5 owns that milestone); the `read.*` configuration file surface is W5 (bead
`provenance-1wh.3`); search ranking; ideation rows in `relations` (section H defers them); threads, messages, and the
ideation families gain no derive (no served operation reads them); any MCP tool. Bead `provenance-qrc.2` ("Migrate SDK graph
queries to indexed traversal") describes this plan's neighbors/trace/impact item with pre-cut paths; it is subsumed here and
closes with this bead.

## B. Revised W3 text (the bead text)

*Goal.* Serve each of the eight operations from the stamped projection on the post-cut relation shapes. Preserve response
shapes. State what each stamp attests. Ship the contracted fixes with the re-back.

*Where the code stands.* `queries::open` builds a `StateStore` with no publication lock (`queries.rs:29-33`). Six operations
call `records::load`, which reads and sorts every node kind per call (`records.rs:12-73`; callers `get` `records.rs:92`,
`search` `records.rs:123`, `neighbors` `walk.rs:122`, `trace` `walk.rs:154`, `impact` `impact.rs:27`, `resolve_symbol`
`symbols.rs:53`). Traversals walk `RecordFront` through `related_nodes` and `flow_neighbors`
(`core/model/relations/front.rs:89-133,147-172`). `impact` and `resolve_symbol` call `provenance_scanner::scan_path(repo)`
unbounded (`impact.rs:67`, `symbols.rs:29`; walker at `provenance-scanner/src/walker.rs:78-109`). `evidence` cuts four lists
independently and OR-merges one `has_more` (`evidence.rs:60-63,85`). The projection holds eighteen families
(`cache/projection_families.rs:25-66`), a derived `relations` table with `idx_relations_out` and `idx_relations_in`
(`migrations/021_relations_table.sql:9-19`), and the stamp tables (`018_projection_stamp.sql:45-64`); nothing in production
reads `relations` yet. The kind tables hold column subsets, not records: `requirements` has `scope_id, id, statement,
status, domain_id, fog` (`002:14-20`, `009:1`, `010:1`), `rules` has five columns (`003:17-27`, `015`, `016`); none carries
`retired`, `description`, `source_refs`, or the other fields `GraphNode` serializes whole (`protocol/node.rs:16-27`).
Section C widens them to mirror the record types (`res_projection_tables_mirror_record_types`).

*Per-operation mapping.*

1. `get`, `search`: one row lookup, and one ordered scan per wanted kind, over the mirrored kind tables (C).
   `include_retired` reads the `retired` column. `search` keeps its text semantics (`node.rs:74-103`, `records.rs:126-130`)
   and the protocol-five default kinds (`records.rs:142-149`); it stops at the limit with `has_more`, as today.
2. `neighbors`, `trace`: walk the derived `relations` table (`idx_relations_out`, `idx_relations_in`) through `SqlFront`
   (C). Filters name relations from the declared vocabulary (thirteen names, `relations.rs:73-83`; refusal at
   `walk.rs:198-208`), not edge types; protocol 6 already carries this. Ordering: node rank, canonical id, declaration
   order, direction (`front.rs:123-131`; trace by depth first, `walk.rs:174,210-215`). One neighbor per (relation,
   direction, endpoint). Both stop at the limit with `has_more`; the trace cut at `walk.rs:183-185` keeps its meaning.
3. `impact`: walks over `relations`, each declared relation in its flow direction (`decl.rs:15-19`, `front.rs:147-172`), up
   to ten steps (`impact.rs:37`), never a step no declaration gives (`res_impact_follows_declared_flow`): a resolution
   reaches the rules and the requirements that name it, not the requirements it answers nor their rules; a source reaches
   rules through the requirements that cite it. The file scan stops at a configured file count over a sorted walk and the
   answer says so (G). The `INDIRECT` filter of the `impact` command (`cache/impact.rs:41-47`) loses its dead `contradicts`
   entry (I).
4. `resolve_symbol`: stays hybrid. The request names one file, so the scanner reads that file alone (`symbols.rs:31-38`; G);
   binding matches and Rule records come from the projection (`implementation_bindings`, `verification_bindings`, `rules`).
5. `evidence`: `implementation_bindings`, `verification_bindings`, `reviews`, `review_required` come from the projection
   (`018:1-43`, widened in C), the reviews filtered to `cleared_at IS NULL` as `open_requirement_reviews` does today
   (`requirement_reviews.rs:144-152`); verification runs stay cache JSONL (`verification_runs.rs:153-167`;
   `docs/cache.md:83-87`); the `stale` half stays a git diff (`evidence.rs:64-81`). Paging becomes truthful: one `has_more`
   per list beside the top-level OR (`res_query_answers_stop_at_the_limit`).
6. `stale`: git machinery only; near-zero re-back. It reads the diff, never the working tree (`stale.rs:11-14`), and keeps
   reading graph evidence from canonical shards (`stale.rs:36-40` through `cache/health.rs:59-66`). Its stamp attests
   nothing.

*Stamp.* Every answer carries `stamp` with `serial`, `digest`, `instance_id`, `derivation`, `policy`, `attested`, `live`
(`res_stamp_names_projection_instance`; D). Protocol stays 6; W3 adds no bump.

*Consistency.* One reader entry takes the publication guard, runs catch-up as the freshness step, and answers from the
projection at the stamped serial; a failed freshness step answers at the stored serial and says so; live halves run inside
the same guard scope (E).

*Gates.* Each operation flips only after its differential suite is green (J). No flag day. The flip order is get, search,
neighbors, trace, impact, evidence, resolve_symbol; stale last and mostly unchanged (K).

## C. Mirrored kind tables, the `ProjectionRow` derive, and `SqlFront`

*The tables.* Migration `022_record_columns.sql` drops and recreates the eight kind tables (`sources`, `requirements`,
`resolutions`, `rules`, `topics`, `questions`, `domains`, `boundaries`) and the three 018 tables (`implementation_bindings`,
`verification_bindings`, `requirement_reviews`) with one column per field of the Rust record type, named as the field
(`Source` at `artifacts.rs:22-70`, `Requirement` :81-131, `Resolution` :168-220, `Rule` :223-280, `Boundary`, `Topic`,
`Question` at `shaping.rs:114-179`, `Domain` at `services.rs:6-15`, the three integration types at
`integrations.rs:132-182`). Primary key `(scope_id, id)` as today. List and struct fields are JSON text, as `topics.links`
is today (`007:18`, `graph_records.rs:108`): the nine list fields, `declaration_address`, and `source_ref` (today flattened
as `source_id`, `source_clause`, `007:6-7`). `confidence` is declared `REAL` so `1.0` stays a float.
`requirement_reviews.before_text` and `after_text` become `before` and `after` (`018:35-36`, `integrations.rs:160-161`);
every table gains `schema_version`, the eight kind tables gain `retired` where the type has it and one derived column
`search_text` (the lowercased `searchable_text` pieces, `node.rs:74-103`, joined by `\u{1}`); the three 018 tables carry no
`search_text`. Identifiers are quoted in generated SQL (`before`, `after`, `key`, `field` are SQLite keywords). The indexes
that still name a column are recreated (`003:15,29`, `007:10,21-22,36-38`, `009:11`, `010:7-8`). The same migration
recreates `relations` with `target_type` in the primary key, because `links` may name one id under two kinds (`021:16`).
Catch-up rebuilds after a migration (`catch_up.rs:55-57`), so an existing database converts on the first read. Rejected:
`ALTER TABLE ... ADD COLUMN` per missing field; it keeps the drifted names and the flattened `source_ref`.

*The derive.* `#[derive(ProjectionRow)]` in `provenance-macros/src/projection_row.rs` (new; `relations.rs` is 320 lines and
stays separate; the field-type reading goes in `projection_row/shape.rs` if the file passes 300) on the eleven record types,
the same mechanism as `Relations` (`lib.rs:105-111`, `relations.rs:1-60`). `provenance-core` has no sqlx
(`provenance-core/Cargo.toml`), so the derive emits nothing that names it. It emits `impl ProjectionRow for Kind` over a
trait in `provenance-core/src/model/projection_row.rs` (new): `const TABLE: &str`, `const COLUMNS: &[&str]` (field names in
declaration order), `fn row(&self) -> Vec<ColumnValue>`, `fn from_row(&[ColumnValue]) -> anyhow::Result<Self>`, with `enum
ColumnValue { Null, Integer(i64), Real(f64), Text(String) }`. Encoding goes through `serde_json::to_value` per field, so no
type needs its own code: a JSON string is `Text`, a number `Integer` or `Real`, a bool `Integer` 0 or 1, null `Null`, an
array or object `Text` holding its JSON. Decoding reads the field's spelled type, as `Relations` reads `StableId`
(`relations.rs:14-28`): `Vec<_>`, `Option<Vec<_>>`, and a field marked `#[column(json)]` (the struct-typed
`declaration_address` and `source_ref`) parse their text; `bool` reads an integer; everything else becomes the JSON scalar
its column holds; the map then goes through `serde_json::from_value::<Kind>`. The derive refuses a tuple struct and a field
named `search_text`; refusals live in `provenance-core/tests/projection_row_refusals.rs` beside
`relation_derive_refusals.rs` (trybuild).

*Round-trip gate.* `provenance-core/src/model/tests/projection_row/{artifacts, shaping, integrations}.rs` (new): for each of
the eleven types, a fixture with every field filled (an empty field is hidden by `skip_serializing_if` and fails the
completeness assertion, as in the cut's serde-walking test), `confidence` at `1.0` in one fixture and `0.95` in another, and
one all-default fixture per type beside it, so the `None` path of every optional field runs (`Null` decodes to JSON `null`
and `from_value` gives `None`; a `#[column(json)]` field has that arm too); `row` then `from_row`, and
`serde_json::to_string` of both sides equal. A store-side twin materializes the fixtures and decodes from SQLite. A drift
test in `cache/tests/record_columns.rs` (new) asserts the column name set of each table equals `Kind::COLUMNS` plus
`search_text` on the eight kinds (a set comparison, so a later `ADD COLUMN` migration passes), so a new struct field fails
CI until the next migration adds its column; the derive covers the insert and the decoder.

*Loaders.* `materialize/graph_records.rs` (178, eight hand-written inserts, `:28,45,63,92,108,125,145,166`) and
`integration_records.rs` (65) are replaced by one generic `materialize/record_rows.rs`: `load_kind::<K: ProjectionRow>(tx,
scope, records, search_text: impl Fn(&K) -> Option<String>)` builds `INSERT INTO "{TABLE}" ("{COLUMNS}"[, search_text])
VALUES (...)` from the trait and binds each `ColumnValue`; the eight kind callers pass `GraphNode::searchable_text`
lowercased, the three integration callers pass `|_| None`. `materialize.rs:67-72` calls it eleven times; the catch-up loader
(`family_rows.rs:load_rows`, called at `catch_up.rs:227`) reaches the same function.

*Decoder and readers.* `crates/provenance-store/src/cache/read/` (new): `mod.rs` (stamp read), `rows.rs` (a `SqliteRow` into
`Vec<ColumnValue>` by `COLUMNS`, choosing `Integer` or `Real` by the value's storage class through `ValueRef::type_info()`,
never by trying `i64` first; then `K::from_row`), `records.rs` (`record::<K>(pool, scope, id)`, `records_by_ids`, `kind_of`
as up to eight primary-key lookups in rank order replacing `walk.rs:81-83`, and `search_kind` as `WHERE scope_id = ? AND
instr(search_text, ?) > 0 [AND retired = 0] ORDER BY id`), `front.rs` (`SqlFront`). Wiki and gaps read through `StateStore`
and `GraphRecords` (`state_adapter.rs:22-32`) and never touch the trait.

*Search.* Kinds are visited in rank order (`graph.rs:30-40`), which equals `records::load`'s sort (`records.rs:67-71`). The
`instr` prefilter is a superset (a needle can span two pieces), and the exact per-piece `contains` (`records.rs:126-130`)
runs in Rust before a row counts toward `limit + 1`.

*Retired records.* `walk.rs:29-60` builds the front from `records::load(.., include_retired)` (`records.rs:66`), so today a
retired record contributes no outgoing rows and is never an endpoint, a retired origin named with an explicit `node_type`
still answers its live `in` neighbours, and `kind_of` returns `None` for a retired origin. `relations` carries no retired
marker (`021:9-19`) and the hop query has no filter, so the executors reproduce this: with `include_retired` false, the
out-rows of a retired origin are dropped, `kind_of` skips retired rows, and every endpoint is checked against its kind
table's `retired` column (the `records_by_ids` hydration lookup already reads it) before it counts toward the page. The
fixture in J exercises every case both ways.

*`SqlFront`.* `RelationSource` has two synchronous methods (`front.rs:57-65`); `related_nodes` and `flow_neighbors` sort
after collecting (`front.rs:123-131`), so a front supplies rows and the core owns the order. sqlx is async, so `SqlFront` is
a fetched hop: `async fn SqlFront::hop(pool, scope, frontier: &[(NodeType, StableId)]) -> HopRows` runs two indexed queries
per hop and per chunk of 500 frontier ids (`impact` has no breadth cut, `impact.rs:37-60`, and SQLite bounds bind
parameters), `WHERE scope_id = ? AND owner_type = ? AND owner_id IN (...)` over `idx_relations_out` and the mirror over
`idx_relations_in`; `HopRows` implements `RelationSource` over the fetched rows, interning each fetched relation name to the
`&'static str` the trait returns (`front.rs:59-64`) through `declaration_for(owner, name).name` or `LINKS`
(`front.rs:17,69-74`) and refusing a name with no declaration, so the trait is not edited. `related_nodes`,
`flow_neighbors`, and the executors' filters (`walk.rs:86-103`) run unchanged over it; no operation gets a private walk. A
lookup for an id outside the fetched frontier is an invariant violation (`debug_assert!` plus a test). `neighbors` is one
hop; `trace` and `impact` fetch one hop per depth, and trace's `seen` set (`walk.rs:166`) is keyed by kind and id. Rejected:
loading the whole `relations` table per request (no index use); `block_in_place` in the synchronous trait (panics on the
current-thread runtime `#[tokio::test]` uses).

*Duplicate references.* `relations` is one row per (owner, relation, target) (`021:16`, `INSERT OR IGNORE` at
`relation_rows.rs:57-58`), while `outgoing_of` and `incoming_of` (`front.rs:250-266,283-289`) yield one row per stored
reference, so a requirement citing one source under two clauses answers two `cites` neighbours today and one from
`relations`. Decision: one neighbor per (relation, direction, endpoint) is the served meaning; `related_nodes` dedupes in
core after its sort, so `RecordFront`, `SqlFront`, wiki, and gaps agree. This is derivation history entry 1 (D). The front
equivalence property carries a two-clause citation fixture.

*Front equivalence gate.* A property test materializes one scope and asserts `related_nodes` and `flow_neighbors` (both
ways) agree over `RecordFront` and `SqlFront` for every record; corpus: the seeded store
(`operations/queries/tests.rs:16-68`), the gap fixtures, the retired and two-clause fixtures (J), and the repository's own
state.

## D. Stamp semantics

*Wire shape* (`res_stamp_names_projection_instance`). `QueryResponse` (`response.rs:16-33`) gains one field after
`operation`:

```
"stamp": { "serial": 41, "digest": "sha256:...", "instance_id": "uuid", "derivation": 1,
           "policy": "catch_up", "attested": ["relations", "requirements"], "live": [] }
```

`serial` and `digest` are the latest `projection_revision` row (`018:50-54`; `stamp.rs:56-67`); `instance_id` is the
`projection_instance` row (`018:45-48`, `stamp.rs:24-27`), and serials compare only within one instance
(`docs/cache.md:15-17`). `policy` is the freshness policy the reader ran, or `catch_up_failed` (E). `attested` names the
projection tables behind the answer, in the family words of `projection_families.rs:69-90` plus `relations`. `live` names
what the stamp does not cover, from a closed list: `canonical` (read from canonical shards), `scanned_sites` (working-tree
scan), `verification_runs` (cache JSONL), `diff` (git). A stamp never implies freshness for anything it does not list. The
TypeScript `QueryEnvelope` (`protocol.ts:212-215`) gains `stamp?: Stamp`; the engine dispatches generically
(`engine.ts:20-28,47-53`).

*Derivation version.* `pub const READ_DERIVATION: u32 = 1` in `operations/stamp.rs` (new), with a numbered history in its
doc comment; entry 1: one neighbor per (relation, direction, endpoint), and trace's `seen` keyed by kind and id. It bumps
when reader logic changes an answer for the same projection rows: traversal order, a filter's semantics, decoding, scan
accounting. It does not bump for a migration (the serial moves then; the digest moves only when canonical bytes do,
`projection_digest.rs:32-52`) or for a fix in a live half. A golden test (`operations/queries/tests/golden.rs`) answers a
fixed request set over a frozen corpus built by `cache/tests/fixtures/golden.rs` (every kind, every relation, retired
records, over-limit lists; never `.provenance/state`, which moves on most PRs), digests the answers with the stamp removed,
and compares to a committed file keyed by `READ_DERIVATION`; the file regenerates only in a commit that bumps the constant,
and a test asserts the key equals it.

*Per-operation table.* The two array cells hold the literal wire strings and nothing else: `attested` is family words plus
`relations`, `live` is from the closed four-word list. The third column says which answer fields each covers. Before an
operation flips (K) its row reads `attested: []` and `live: ["canonical"]` plus its own live words.

| Operation | `attested` | `live` | What the two lists cover |
|---|---|---|---|
| get | the kind table read | | `found`, `node` |
| search | the kind tables searched | | `nodes` |
| neighbors | `relations`, the kind tables read | | `neighbors` |
| trace | `relations`, the kind tables read | | `nodes` |
| impact | `relations`, every kind table the walk read (`impact.rs:30-48`), `rules`, `implementation_bindings`, `verification_bindings` | `scanned_sites` | rule identities and binding rows attested; scanner sites in `implementations` and `verifications` live (`sites.rs:26-33,51-62`) |
| evidence | `implementation_bindings`, `verification_bindings`, `requirement_reviews` | `verification_runs`, `canonical`, `diff` | the three lists and `review_required` attested; `verification_runs` and `latest_verification_run` live; `stale` live (`stale.rs:36-40`) |
| stale | | `canonical`, `diff` | the whole answer |
| resolve_symbol | `implementation_bindings`, `verification_bindings`, `rules` | `scanned_sites` | binding matches and Rule records attested; the named file's sites live (`symbols.rs:31-38`) |

## E. Reader entry

One helper, `operations/reader.rs` (new): `async fn answer<R>(repo, scope, policy, run) -> (R, Stamp)`.

1. Take the guard: `publication::publication_guard(layout).await` (`publication/guard.rs:68-83`). Under read-only validation
   it holds no lock (`guard.rs:70-72`).
2. Open the pool once (`cache.rs:34-39`). Freshness step. `catch_up`: `catch_up_with_guard(&guard, &pool, layout)`
   (`catch_up.rs:37`, taking the reader's pool instead of opening its own at `:41`, re-exported from `materialize.rs` as
   `pub(crate) use catch_up::{catch_up_with_guard, CatchUpReport}` because `mod catch_up` is private): migrations, a rebuild
   when the database has no revision or a migration applied, snapshot, validation, every unit hashed, changed families
   re-derived, one revision or none (`catch_up.rs:41-123`). `annotate_only`: no catch-up; the stamp reports the stored
   serial. `refuse_stale`: reserved, refused as unimplemented with a typed error; W5 implements it.
3. When the freshness step fails (a validator refusal at `catch_up.rs:63-66` or `materialize.rs:52-55`, an I/O error, an
   unwritable cache directory) and the database holds a revision, the reader answers at the stored serial with `policy:
   "catch_up_failed"` and the error text in an additive `freshness_error` field; freshness annotates and never refuses. When
   the database holds no revision, under any policy, there is nothing to answer from and the read refuses, naming
   `provenance materialize`; under `annotate_only` the same refusal covers a database whose migrations are behind
   (`open_cache` runs none, `cache.rs:34-39`, and `annotate_only` writes nothing), checked through `applied_migrations`
   (`migrations.rs:170`). Tests: `a_read_answers_at_the_stored_serial_when_catch_up_refuses`,
   `a_read_with_no_database_refuses_and_names_materialize`.
4. Read the stamp rows; run `run(&ReadContext)` with the pool, the guard, the scope, the scan limit, and the policy. Every
   projection read of one answer happens under the guard, so no projection write interleaves (`docs/cache.md:55-59`).
5. Live halves run in the same scope: `scan_path_bounded` and `scan_file` read the working tree (`impact.rs:67`,
   `symbols.rs:29`); git reads the object store (`stale.rs:30-43`); verification runs read cache JSONL under their own lock
   (`verification_runs.rs:157-159`). Trap, stated at `guard.rs:7-15`: a synchronous `with_repository_publication` section
   entered while the guard is held blocks on a second file description of the same lock (`guard.rs:37-45`).
   `stale::disturbed` enters one through `cache::graph_evidence` (`stale.rs:36`, `health.rs:65`). Fix: `cache/health.rs`
   gains `pub fn graph_evidence_under_guard(_guard: &PublicationGuard, layout, scope, include_retired)` calling the existing
   `graph_evidence_locked` (`health.rs:68`) with no lock, the guard type as the proof (the pattern of
   `snapshot_state_under_guard`, `guard.rs:94-99`); `disturbed` takes `&PublicationGuard`. No second snapshot: catch-up's
   own snapshot is dropped on return (`catch_up.rs:59`, `publication.rs:17-19`) and `annotate_only` takes none.
6. Drop the guard; print.

*Policy and knobs.* `operations/read_policy.rs` (new): `enum FreshnessPolicy { CatchUp, AnnotateOnly, RefuseStale }`,
`struct ReadPolicy { freshness, scan_limit }`, defaults `CatchUp` and 2000 files. Ben measured the scan at about 1.1 s for
657 files on this repository (1.7 ms per file), so 2000 files bounds the scan near 3.5 s; W5 tunes it. The names reserved
for the configuration file are `read.freshness_policy` and `read.scan_limit`. No configuration file reader exists in the
tree; W3 ships the struct as the seam and bead `provenance-1wh.3` wires the file. No journal knob, no visit budget, no
request-side knob.

*Async.* `queries::get` and the other seven become `pub async fn`; `handlers/sdk/query.rs:24-64` and `handlers/sdk.rs:13`
become async (the binary is on tokio, `main.rs:16-21`). The CLI adapter stays thin.

*Cost, stated plainly.* Reads serialize against publications and against each other. Under `catch_up` every read copies the
state tree (`catch_up.rs:59`, `guard.rs:94-99`), hashes every canonical byte (`catch_up.rs:86-97`), and runs the two
validators; a fresh clone rebuilds on the first read. Reversal touches one site: `reader.rs` is the only caller of the guard
on the read path, and an unflipped operation answers over canonical inside the same helper.

## F. Limits and truthful cut flags

There is no paging (`res_query_answers_stop_at_the_limit`, `boundary_query_answers_do_not_page`). Every operation keeps
`limit` (50 by default, 200 at most, `protocol.rs:28,31`) and `has_more` from `take_page` (`protocol.rs:84-88`). No request
gains a cursor; no response gains a next page; no token module exists. `evidence` gains four additive booleans,
`implementation_bindings_has_more`, `verification_bindings_has_more`, `verification_runs_has_more`, `reviews_has_more`, each
the cut flag of its own `take_page` call (`evidence.rs:60-63`); the top-level `has_more` keeps its OR meaning
(`evidence.rs:85`). A test pins that a fixture where two lists exceed the limit sets exactly those two flags. The TypeScript
`EvidenceResponse` (`protocol.ts:364`) gains the four optional booleans.

## G. The scan limit

`scan_limit` lives on `ReadPolicy` (E) and is not a request field. `provenance-scanner/src/walker/bounded.rs` (new;
`walker.rs` is 417) gains `scan_path_bounded(path, max_files) -> anyhow::Result<(Vec<FileScan>, bool)>`. Today's walk is
`readdir` order with a sort after the loop (`walker.rs:85-109`), so a cut in walk order would scan different files on two
clones of one commit. The bounded walk uses `WalkDir::sort_by_file_name()`, so its order is deterministic, and counts only
files that pass `Language::from_extension` (`walker.rs:98`); the cut set is the first `max_files` language files in that
order, and the bool says whether it stopped. `impact` (`impact.rs:67`) calls it and its response gains one additive field,
`scan_cut: bool`, false when the scan finished; `true` says the scanned sites are a lower bound. `resolve_symbol` does not
walk the tree: the request names one file, so it calls `scan_file` (`walker.rs:122`) on `repo.join(file)` alone; one file
never meets the limit, the answer cannot miss the file the caller asked about, and `resolve_symbol` carries no `scan_cut`
field. Nothing else bounds the graph walk: depth ten (`impact.rs:37`, `TRACE_MAX_DEPTH`) over indexed rows is the bound.
Tests read `scan_cut` from the response, not a log.

## H. Dangling-target existence validation for ideation targets

`IdeationTarget { artifact_type, artifact_id }` (`core/model/ideation.rs:276-281`) names seven kinds (`IdeationTargetType`,
`ideation.rs:14-29`: the six graph kinds plus domain; no boundary). Contributions and synthesis packets write it unchecked
in `create_contribution`, `upsert_contribution`, `create_synthesis_packet`, and `upsert_synthesis_packet`
(`state_store/ideation_writers.rs:8,16,127,135`; inputs at `inputs.rs:271,289`). The existence index covers four kinds
(`state_store/canonical_artifacts.rs:17-51`, key at :78-89) and is called only for dispositions' `canonical_artifact`
(`proposal_writers.rs:363`, `ideation_batches.rs:119,232`).

*Plan.* Extend `CanonicalArtifactIndex` to every `NodeType` (eight kinds), keyed by the serde word; add
`ensure_target_exists(&IdeationTarget)` mapping `IdeationTargetType` onto `NodeType`; call it in the four writer functions
only, so a new dangling target is refused at write time. It is not added to `validate_graph_scope`
(`graph_validation.rs:23-45`): that validator runs on every catch-up pass and every rebuild (`catch_up.rs:63-66`,
`materialize.rs:52-55`) and cannot tell a new reference from an old one, so one old dangling target would refuse every read.
Existing state is reported: a gap pass `cache/gaps/ideation_targets.rs` (new) emits `GapKind::DanglingReference`
(`cache/gaps/model.rs:18`) with the wording of `dangling.rs:52-56`, "target points at missing <kind> <id>", over
contributions and synthesis packets, which `GraphRecords` (`state_adapter.rs:22-32`) gains as two lists.
`IdeationTargetType` gains `Boundary` so the superset is complete. The bead description names this work ("dangling-target
existence validation"); it lands as its own PR 4 (K) so PR 3 stays flips plus leftovers.

*Exposure: deferred.* Ideation and thread rows do not enter `relations` in this bead (cut plan L12). The gate for the later
bead: this validation merged; `provenance gaps` reports no dangling target on the repository's own state; and an owner
decision on the relation name the rows carry, because the thirteen-name vocabulary printed at `docs/cli.md:138-142` is
closed and a new name is a vocabulary change. Until then, served reads never reach an ideation record through a walk.

## I. PR 180 leftovers assigned to W3

1. `INDIRECT` dead entry (`cache/impact.rs:41-47`). `contradicts` is declared `flow = none` (`shaping.rs:176`), so
   `flow_neighbors` never yields it (`front.rs:168`) and the filter never sees it. Decision: delete the entry; add a test
   that every `INDIRECT` name resolves through `declaration_for` to a declaration whose flow is not `None`.
2. Cut plan L3: `is_resolved` (`cache/gaps/contradiction.rs:37-48`) treats any `resolution_id` as settling the pair,
   including a rejected resolution. Fix: the pair is settled only when the named resolution exists in the scope and its
   status is not `Rejected` (`ResolutionStatus`, `core/model/artifacts/kinds.rs:94-108`); `supersedes` unchanged. RED test:
   a question naming a rejected resolution still reports `UnresolvedContradictsPair`.
3. Cut plan L5: the neighbors order is rank, id, declaration order, direction (`front.rs:123-131`), one row per (relation,
   direction, endpoint). With no cursor, the order is frozen by the rank-order pin test and the golden file (D): a reorder
   is a visible derivation bump.
4. `contradicts` shape on the filter: pre-cut the relation joined two requirements; now a question owns it
   (`state-format.md:32-34`), so `neighbors` of a requirement with `relations: ["contradicts"]` answers the question `in`,
   and the other requirement is one more hop. Documentation only: one sentence in `docs/cli.md` after line 146. No code.

## J. Test strategy

*Oracle.* Before any flip, today's executors are copied into `operations/queries/tests/oracle/{records, walk, impact,
evidence, symbols}.rs` under `#[cfg(test)]`, `super::` imports rewritten to `crate::` paths (`walk.rs:1-13`,
`impact.rs:10-11`, `evidence.rs:8`, `symbols.rs:9-10` name siblings) so `bindings`, `sites`, and `stale` are the production
modules, not copies; each carries a two-line header naming the operation it preserves and the commit that may delete it.
They read canonical shards.

*Differential harness.* `operations/queries/tests/differential.rs`: for each operation and a request set, run the oracle and
the served executor over the same store with `scan_limit` at `usize::MAX`, serialize both to `serde_json::Value`, strip the
additive fields (`stamp`, `freshness_error`, the four evidence flags, `scan_cut`), and assert equality; on the two-clause
and cross-kind fixtures it asserts the documented derivation-1 delta instead. Corpus: the seeded store
(`queries/tests.rs:16-68`), the CLI fixtures (`tests/query_support/fixtures.rs:28-42`), the gap fixtures, new fixtures in a
sibling `cache/tests/fixtures/` module (`fixtures.rs` is 315): a retired source, requirement, and rule that both reference
and are referenced (exercised with `include_retired` both ways; the live state has one retired rule), a two-clause citation,
over-limit lists, two over-long evidence lists, a cleared review; and the repository's own `.provenance/state`, opened
through `StateStore` at the workspace root (no test on main reads it today). Each operation flips only when its rows are
green. The `records` oracle (get, search) and `differential.rs` stay until the bead that deletes `records::load`; the walk,
impact, evidence, and symbols oracles leave in the last commit of PR 3 once the front equivalence property and the
per-operation tests stand.

*Derive and tables.* The round-trip tests per type and the column drift test (C); trybuild refusals for the derive; the
catch-up dumps compare the eleven widened tables and `relations` as today (`catch_up_behavior.rs:44-56`,
`relation_rows_behavior.rs:11-22`); the front equivalence property (C).

*Order stability.* A property test inserts and retires records between two identical requests and asserts the surviving
records keep their relative order in `search`, `neighbors`, and `trace`.

*Limit truthfulness.* A scope with more matches than the limit for each operation asserts `has_more` and a page of exactly
`limit`; the evidence fixture with two over-long lists asserts the two flags (F); a repository with more language files than
`scan_limit` asserts `scan_cut` on `impact`, that two runs cut the same file set, and that sub-limit runs answer the same
union as `scan_path` (G); `resolve_symbol` on a file past the cut still answers its rule.

*Stamp truthfulness, one test per live constituent.* Mutate the constituent alone and assert the attested fields and the
stamp's serial and digest stand still: add a scanner annotation in the working tree (`scanned_sites`); append a run to
`verification-runs.jsonl` (`verification_runs`); make a commit that touches a bound file (`diff`); for `stale`, edit a
canonical shard and assert the answer changes while the stamp lists `canonical`. A second test per flipped operation edits a
canonical shard and asserts the serial advances by one under `catch_up` and stands still under `annotate_only`.

*Guard interleaving.* The `test_probes` pattern (`test_probes.rs:23-40`; `materialize_guard_behavior.rs:22-38`): a probe
inside the reader entry asserts the lock is held; a probe starts a canonical write on another thread and asserts it waits
until the answer prints; a third runs `evidence` with `base` under the guard and asserts it returns (the `stale.rs:36` trap
in E).

*Homes.* `operations/queries/tests/{reader, stamp, limits, order}.rs` (`queries/tests.rs` is 175 and keeps its five).

*TypeScript.* `protocol.ts` gains `Stamp`, `stamp?`, `freshness_error?`, the four evidence booleans, and `scan_cut?` on
`ImpactResponse` only; the type tests compile an old-shape response against the new types, and the runtime suite reads one
stamped answer.

## K. Delivery

Four PRs: the derive and migration are a self-contained, zero-behaviour-change unit that reviews best alone; the flips and
leftovers are PR 3; section H is PR 4. Each PR is green at every commit (`cargo fmt --all --check`, `cargo clippy
--workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace`, `npm test`) and gets the deslop pass
before ready.

*PR 1, branch `1wh-w3-reader`: reader entry and stamp; nothing served from SQL yet.*
1. Oracle copies and the differential harness over the current executors (green against themselves).
2. `read_policy.rs` and `stamp.rs` with `READ_DERIVATION`; RED: `the_stamp_carries_the_stored_instance_id`.
3. `reader.rs`; the eight operations become async and answer through it over canonical, stamped `attested: []` and the
   pre-flip `live` words; `graph_evidence_under_guard` and `disturbed(&PublicationGuard, ..)`; `catch_up_with_guard` takes
   the pool. RED: `every_answer_carries_a_stamp_at_the_stored_serial`, `a_read_holds_the_publication_lock_while_it_answers`,
   `a_canonical_write_waits_for_a_read_to_finish`, `evidence_with_a_base_answers_under_the_guard`,
   `a_canonical_edit_advances_the_serial_under_catch_up`, `a_read_answers_at_the_stored_serial_when_catch_up_refuses`,
   `a_read_with_no_database_refuses_and_names_materialize`.
4. Golden builder, golden file, `READ_DERIVATION` key test; TypeScript `Stamp` types. Green: the eight operations' test
   count unchanged plus the new ones; every existing response byte unchanged apart from `stamp`.

*PR 2, branch `1wh-w3-record-columns`: the derive and the mirrored tables; no flip.*
1. `ColumnValue` and the `ProjectionRow` trait in core; the derive in `provenance-macros/src/projection_row.rs`; trybuild
   refusals RED first (`a_tuple_struct_is_refused`, `a_field_named_search_text_is_refused`).
2. The derive on the eleven types; round-trip tests RED first, one per type, the fixtures filling every field
   (`a_requirement_round_trips_through_its_row`, and so on; `a_round_confidence_stays_a_float`); `#[column(json)]` on
   `declaration_address` and `source_ref`.
3. Migration 022 (eleven tables and `relations`), `record_rows.rs` replacing `graph_records.rs` and
   `integration_records.rs`, catch-up reload; RED: `every_kind_table_mirrors_its_record_columns` (drift),
   `materialize_writes_search_text_from_searchable_text`, `catch_up_equals_rebuild_over_the_widened_tables`,
   `a_link_under_two_kinds_keeps_both_relation_rows`.
4. `cache/read/{rows, records, front}.rs`; the dedupe in `related_nodes` with its fixture RED; the front equivalence
   property RED over `RecordFront` versus `SqlFront`. Green: no served byte changes except the derivation-1 dedupe.

*PR 3, branch `1wh-w3-flips`: the seven flips, the leftovers, oracle removal.* After step 8 `records::load` and `find`
(`records.rs:12,75`) have only the `records` oracle as caller; they move under `#[cfg(test)]` until W5 deletes them, so
`clippy -D warnings` stays green.
1. Flip `get`; RED: differential rows, `get_reads_a_retired_record_only_when_asked` over the `retired` column.
2. Flip `search`; RED: differential rows, `search_visits_kinds_in_rank_order_and_stops_at_the_limit`.
3. Flip `neighbors` and `trace`; RED: differential rows, the order pin, `trace_stops_at_the_limit_and_says_has_more`,
   `a_retired_origin_still_answers_its_live_in_neighbours`.
4. `scan_path_bounded` and `scan_limit`; flip `impact`; RED: differential rows, `impact_says_when_the_scan_was_cut`,
   `a_cut_scan_reads_the_same_files_twice`, `an_indirect_name_is_a_declared_relation_with_a_flow` (I.1),
   `a_resolution_reaches_the_rules_that_name_it_only`.
5. Flip `evidence`; RED: differential rows, `evidence_reports_which_list_was_cut`, `a_cleared_review_is_not_open`.
6. Flip `resolve_symbol` over `scan_file`; RED: differential rows, `resolve_symbol_reads_the_named_file_only`.
7. Stamp truthfulness tests per live constituent (J); `is_resolved` fix (I.2) RED:
   `a_rejected_resolution_does_not_settle_a_contradiction`.
8. Delete the walk, impact, evidence, and symbols oracles; keep `records` and `differential.rs`; docs: `docs/cli.md` (stamp,
   `freshness_error`, the evidence flags, `scan_cut`, the `contradicts` sentence), `docs/cache.md` (the widened tables in
   the family table at lines 63-73, the read path at 3-10); section B of this file onto the bead; close `provenance-qrc.2`
   as subsumed.

*PR 4, branch `1wh-w3-ideation-targets`: section H alone.* RED: `a_contribution_naming_a_missing_target_is_refused`,
`an_existing_dangling_target_is_a_gap_not_a_refusal`, `a_read_over_an_old_dangling_target_still_answers`.

*File-cap splits before growth.* Named above: `projection_row/shape.rs`, `walker/bounded.rs`, `tests/projection_row/
{artifacts, shaping, integrations}.rs`, `cache/tests/fixtures/`, `queries/tests/{reader, stamp, limits, order}.rs`.
`walker.rs` (417) is not grown: the bounded walk is its own module. `graph_records.rs` and `integration_records.rs` are
deleted. `evidence.rs` (94), `catch_up.rs` (273), and `protocol/query.rs` (175) do not grow past a few lines; `response.rs`
(116) gains the stamp, `freshness_error`, the four flags, and `scan_cut`, with the `Stamp` type in `protocol/stamp.rs`.
`protocol.ts` (399): new types append; split into `protocol/query.ts` if it passes 500.

## L. Risks and open points

1. Per-read tree copy and full hash under `catch_up` (E). Default: accept in W3; W5 measures and, if a read here exceeds one
   second, moves catch-up to hash in place under the guard and snapshot only when a unit moved.
2. The synchronous lock trap (`guard.rs:7-12`). Any live half that calls `with_repository_publication` deadlocks under the
   reader entry. Default: the interleaving test runs every operation with every optional field set; `stale::disturbed` is
   the one known site and goes through `graph_evidence_under_guard`.
3. The derive reads spelled types, as `Relations` does. A struct-typed field without `#[column(json)]`, or a type alias,
   round-trips wrong. Default: the per-type round-trip test with every field filled is the gate, and a new struct-typed
   field fails it before it reaches a database.
4. `f64` fields (`confidence`) pass through JSON and SQLite REAL; a decoder that tries `i64` first would print `1.0` as `1`.
   Default: `REAL` affinity in 022, storage-class decoding in `rows.rs`, and fixtures at `1.0` and `0.95`.
5. Migration 022 drops and recreates eleven tables and `relations`. Default: the projection is rebuildable, catch-up
   rebuilds after the migration (`catch_up.rs:55-57`), and no `sqlx::query` outside `materialize/`, `migrations.rs`, and
   `tests/` reads those tables today.
6. `search` text equality across the SQL prefilter (C). Default: the prefilter is a superset by construction and the Rust
   filter decides; `differential.rs` with the `records` oracle stays in CI until `records::load` leaves, and pins it.
7. The scan default (2000 files) rests on one measurement. Default: W5 tunes it; `scan_cut` makes a low value visible.
8. Impact: the strict one-step reading was rejected (`res_impact_follows_declared_flow`); the walk stays multi-step over
   declared flows with no synthetic step, which is what `impact.rs:37-60` does today.
9. `refuse_stale` is reserved and refused as unimplemented. Default: W5 implements it; the enum member keeps the word.
10. `catch_up_failed` is a fourth `policy` word and names an outcome, not a policy. Default: keep it, with `freshness_error`
    carrying the reason; W5 may rename it with the configuration surface, under a derivation bump.
11. Ideation exposure deferred (H) and `IdeationTargetType` gaining `Boundary`. Default: the gate named in H; the parse list
    (`ideation.rs:32-43`) grows by one.
12. Where the two reviews differ. Duplicate references: Fable dedupes in `related_nodes` (core), GLM in the fetched hop
    rows; core wins, so the two fronts agree by construction and wiki and gaps see one meaning. Retired records: GLM joins
    `retired` in the hop; the executor-side filter wins, the hop stays two index reads and the endpoint check reuses the
    hydration lookup. Section H: GLM asks for a bead split or a cited decision; the bead description names the work, and PR
    4 keeps PR 3 reviewable. Oracle imports: `crate::` paths, no closure copy.
13. A flip is a code path, not a runtime switch. Default: each flip is one commit that reverts cleanly; the stamp's
    `attested: []` row is how a reverted operation is seen on the wire.

*Rulings against the tree.* `res_projection_tables_mirror_record_types` names "one column for each field"; three tables
disagree today and 022 aligns them: `requirement_reviews.before_text`/`after_text` versus `before`/`after` (`018:35-36`,
`integrations.rs:160-161`), `boundaries.source_id`/`source_clause` versus `source_ref` (`007:6-7`), and no table carries
`schema_version`. `res_query_answers_stop_at_the_limit` cites a scan of about one second for 657 files; no timing test
exists in the tree, so that is Ben's measurement, not a pinned fact. No review finding contradicts a binding resolution.

*Old citations not verifiable at `577ab96`.* `idx_edges_scope_type_from` (dropped by 021); `edge_rank` (deleted);
`handlers/sdk/query/walk.rs`; `dangling.rs 7-15` (now `gaps/model.rs:18` and `dangling.rs:36-65`); the three ideation call
sites, which check dispositions' `canonical_artifact`, not `IdeationTarget`.
