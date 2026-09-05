# W3 operations re-back: implementation plan (revision 6)

Bead `provenance-1wh.2`. Read at `577ab96` (PR 180 merged); every file:line below was counted there. This document replaces
the W3 section of `docs/research/2026-08-27-qrspi-1wh-query-uniformity-plan.md` (branch
`opencode/provenance-20260827T223718Z-87cc1ac4`, lines 463-560), which assumed SQLite edge rows and a write journal; the
relation shapes cut (`docs/plans/2026-09-03-relation-shapes-cut.md`) and `res_catch_up_hashes_scopes_no_journal` removed
both.

Revision 2 folded Ben's rulings of 2026-09-05 (four resolutions under `req_query_answers_carry_a_freshness_stamp`, PR 191:
`res_query_answers_stop_at_the_limit` with `boundary_query_answers_do_not_page`, `res_impact_follows_declared_flow`,
`res_projection_tables_mirror_record_types`, `res_stamp_names_projection_instance`). Revision 3 folded the two adversarial
reviews of revision 2 (choices between them in L); revision 4 the review of revision 3; revision 5 Ben's design review of
revision 4 (reads outside the guard over a pinned snapshot, a stamp honest by construction, A/B timings, a release gate, M);
revision 6 the review of revision 5. Section B is the bead text. Settled and not reopened: the served read path is the
projection; freshness annotates and never refuses; the freshness write runs under the publication lock; the vocabulary is
closed; protocol is 6.

## A. Scope and non-goals

The eight SDK operations (`get`, `search`, `neighbors`, `trace`, `impact`, `evidence`, `stale`, `resolve_symbol`;
`operations/queries.rs:35-105`) answer from the stamped projection in `provenance.db`, one operation at a time. Every
existing response field keeps its name, type, and order; new fields are additive. Canonical JSONL stays the only write
target (`docs/cache.md:3-10`). The wire adds a stamp, four per-list `has_more` flags on `evidence`, and a scan flag on
`impact`; no cursor, no resume token, no request budget (`boundary_query_answers_do_not_page`), no predicate language, no
protocol bump (`protocol.rs:25`); the 200 cap and `has_more` stay (F). Two served answers change on purpose, entry 1 of the
derivation history (D): one neighbor per (relation, direction, endpoint), and `seen` sets keyed by kind and id.

Non-goals: wiki and gaps stay on `RecordFront` (`cache/gaps/state_adapter.rs:22-32`, `cache/impact.rs:57-59`);
`records::load` deletion and the `read.*` configuration file are W5 (bead `provenance-1wh.3`); search ranking; ideation rows
in `relations` (H); a derive for threads, messages, or ideation families; any MCP tool. Bead `provenance-qrc.2` (indexed
traversal, pre-cut paths) is subsumed here and closes with this bead.

## B. Revised W3 text (the bead text)

*Goal.* Serve each of the eight operations from the stamped projection on the post-cut relation shapes. Preserve response
shapes. State what each stamp attests. Ship the contracted fixes with the re-back.

*Where the code stands.* `queries::open` builds a `StateStore` with no publication lock (`queries.rs:29-33`). Six operations
call `records::load`, which reads and sorts every node kind per call (`records.rs:12-73`). Traversals walk `RecordFront`
through `related_nodes` and `flow_neighbors` (`core/model/relations/front.rs:89-133,147-172`). `impact` and `resolve_symbol`
call `scan_path(repo)` unbounded (`impact.rs:67`, `symbols.rs:29`; `walker.rs:78-109`). `evidence` OR-merges four
independent cuts into one `has_more` (`evidence.rs:60-63,85`). The projection (eighteen families,
`cache/projection_families.rs:25-66`; the derived `relations` table, `021:9-19`; the stamp tables, `018:45-64`) has no
production reader yet, and its kind tables hold column subsets, not records: `requirements` has six columns (`002:14-20`,
`009:1`, `010:1`), `rules` five (`003:17-27`); none carries `retired`, `description`, `source_refs`, or the other fields
`GraphNode` serializes whole (`protocol/node.rs:16-27`). Section C widens them
(`res_projection_tables_mirror_record_types`).

*Per-operation mapping.*

1. `get`, `search`: one row lookup, and one ordered scan per wanted kind, over the mirrored kind tables (C).
   `include_retired` reads the `retired` column. `search` keeps its text semantics (`node.rs:74-103`, `records.rs:126-130`)
   and the protocol-five default kinds (`records.rs:142-149`); it stops at the limit with `has_more`, as today.
2. `neighbors`, `trace`: walk the derived `relations` table through `SqlFront` (C); filters keep the declared vocabulary
   (`relations.rs:73-83`, `walk.rs:198-208`). Ordering: node rank, canonical id, declaration order, direction
   (`front.rs:123-131`; trace by depth first, `walk.rs:174,210-215`); one neighbor per (relation, direction, endpoint). Both
   stop at the limit with `has_more`; the trace cut at `walk.rs:183-185` keeps its meaning.
3. `impact`: walks over `relations`, each declared relation in its flow direction (`decl.rs:15-19`, `front.rs:147-172`), up
   to ten steps (`impact.rs:37`), never a step no declaration gives (`res_impact_follows_declared_flow`): a resolution
   reaches the rules and the requirements that name it, not the requirements it answers nor their rules; a source reaches
   rules through the requirements that cite it. The file scan stops at a configured file count over a sorted walk and the
   answer says so (G); the `impact` command's dead `INDIRECT` entry goes (I).
4. `resolve_symbol`: stays hybrid. The scanner reads the named file alone (`symbols.rs:31-38`; G); binding matches and Rule
   records come from the projection.
5. `evidence`: bindings, `reviews`, and `review_required` come from the projection (`018:1-43`, widened in C), reviews
   filtered to `cleared_at IS NULL` as today (`requirement_reviews.rs:144-152`); verification runs stay cache JSONL
   (`verification_runs.rs:153-167`); the `stale` half stays a git diff (`evidence.rs:64-81`). Paging becomes truthful: one
   `has_more` per list beside the top-level OR.
6. `stale`: git machinery only; near-zero re-back. It reads the diff, never the working tree (`stale.rs:11-14`), and keeps
   reading graph evidence from canonical shards (`stale.rs:36-40` through `cache/health.rs:59-66`). Its stamp attests
   nothing.

*Stamp.* Every answer carries `stamp` (`res_stamp_names_projection_instance`; D). Protocol stays 6; W3 adds no bump.

*Consistency.* One reader entry takes the publication guard around the freshness step only (catch-up by default), then
answers from a snapshot pinned in one SQLite read transaction at the serial the stamp names; a failed freshness step answers
at the stored serial and says so; readers never wait on a publication or on each other (E).

*Gates.* Each operation flips only after its differential suite is green (J). No flag day. The flip order is get, search,
neighbors, trace, impact, evidence, resolve_symbol; stale last and mostly unchanged (K). W3 ships with W5 (M).

## C. Mirrored kind tables, the `ProjectionRow` derive, and `SqlFront`

*The tables.* Migration `022_record_columns.sql` drops and recreates the eight kind tables and the three 018 tables
(`implementation_bindings`, `verification_bindings`, `requirement_reviews`) with one column per field of the Rust record
type, named as the field (`artifacts.rs:22-280`, `shaping.rs:114-179`, `services.rs:6-15`, `integrations.rs:132-182`),
primary key `(scope_id, id)` as today. List and struct fields are JSON text, as `topics.links` is today (`007:18`,
`graph_records.rs:108`): the nine list fields, `declaration_address`, and `source_ref` (today flattened as `source_id`,
`source_clause`, `007:6-7`). `confidence` is `REAL`. `requirement_reviews.before_text`/`after_text` become `before`/`after`
(`018:35-36`, `integrations.rs:160-161`); every table gains `schema_version`; the eight kind tables gain `retired` where the
type has it and one derived column `search_text` (the lowercased `searchable_text` pieces, `node.rs:74-103`, joined by
`\u{1}`). Generated SQL quotes identifiers (`before`, `after`, `key`, `field` are keywords). Indexes that still name a
column are recreated (`003:15,29`, `007:10,21-22,36-38`, `009:11`, `010:7-8`); `relations` is recreated with `target_type`
in the primary key, since `links` may name one id under two kinds (`021:16`). Catch-up rebuilds after a migration
(`catch_up.rs:55-57`). Rejected: `ADD COLUMN` per field, which keeps the drifted names.

*The derive.* `#[derive(ProjectionRow)]` in `provenance-macros/src/projection_row.rs` (new; `projection_row/shape.rs` if it
passes 300) on the eleven types, the mechanism of `Relations` (`lib.rs:105-111`, `relations.rs:1-60`). `provenance-core` has
no sqlx, so the derive emits `impl ProjectionRow for Kind` over a trait in `provenance-core/src/model/projection_row.rs`
(new): `const TABLE`, `const COLUMNS: &[&str]` (field names in declaration order), `fn row(&self) -> Vec<ColumnValue>`, `fn
from_row(&[ColumnValue]) -> anyhow::Result<Self>`, with `enum ColumnValue { Null, Integer(i64), Real(f64), Text(String) }`.
Encoding goes through `serde_json::to_value` per field: string to `Text`, number to `Integer` or `Real`, bool to `Integer` 0
or 1, null to `Null`, array or object to `Text` holding its JSON. Decoding reads the spelled type, as `Relations` reads
`StableId` (`relations.rs:14-28`): `Vec<_>`, `Option<Vec<_>>`, and `#[column(json)]` fields (`declaration_address`,
`source_ref`) parse their text, `bool` reads an integer, the rest become the JSON scalar the column holds, then
`serde_json::from_value::<Kind>`. The derive refuses a tuple struct and a field named `search_text` (trybuild,
`projection_row_refusals.rs`).

*Round-trip gate.* `provenance-core/src/model/tests/projection_row/{artifacts, shaping, integrations}.rs` (new): per type, a
fixture with every field filled (an empty field is hidden by `skip_serializing_if` and fails the completeness assertion;
`confidence` at `1.0` and at `0.95`) and one all-default fixture so every `None` path runs (`Null` decodes to JSON `null`,
`from_value` gives `None`, the `#[column(json)]` arm included); `row` then `from_row`, and `serde_json::to_string` of both
sides equal; a store-side twin decodes from SQLite. A drift test (`cache/tests/record_columns.rs`) asserts each table's
column name set equals `Kind::COLUMNS` (plus `search_text` on the eight kinds; a set comparison, so a later `ADD COLUMN`
passes), so a new struct field fails CI until its migration lands.

*Loaders and readers.* `materialize/graph_records.rs` (178) and `integration_records.rs` (65) become one generic
`materialize/record_rows.rs`: `load_kind::<K: ProjectionRow>(tx, scope, records, search_text: impl Fn(&K) ->
Option<String>)` builds the insert from the trait (kind callers pass `GraphNode::searchable_text` lowercased, integration
callers `|_| None`); `materialize.rs:67-72` and `catch_up.rs:227` call it. `crates/provenance-store/src/cache/read/` (new):
`rows.rs` (a `SqliteRow` into `Vec<ColumnValue>` by `COLUMNS`, choosing `Integer` or `Real` by storage class through
`ValueRef::type_info()`, never by trying `i64` first; then `K::from_row`), `records.rs` (the `Table<K>` methods of E:
`record`, `by_ids`, `kind_of` as up to eight primary-key lookups in rank order replacing `walk.rs:81-83`, `search` as `WHERE
scope_id = ? AND instr(search_text, ?) > 0 [AND retired = 0] ORDER BY id`), `front.rs` (`SqlFront`). Search visits kinds in
rank order (`graph.rs:30-40`, the sort of `records.rs:67-71`); the `instr` prefilter is a superset (a needle can span two
pieces) and the exact per-piece `contains` (`records.rs:126-130`) runs in Rust before a row counts.

*Retired records.* `walk.rs:29-60` builds the front from `records::load(.., include_retired)` (`records.rs:66`), so today a
retired record contributes no outgoing rows and is never an endpoint, a retired origin named with an explicit `node_type`
still answers its live `in` neighbours, and `kind_of` returns `None` for a retired origin. `relations` carries no retired
marker and the hop query has no filter, so the executors reproduce this: with `include_retired` false, out-rows of a retired
origin are dropped, `kind_of` skips retired rows, and every endpoint is marked in `seen` first and then checked against its
kind table's `retired` column (the `by_ids` hydration lookup reads it) before it counts: the order `walk.rs:166-171` and
`impact.rs:41-48` keep today, so a second path to a retired or dangling node is skipped and a diamond over a retired
requirement answers as today (fixture in J).

*`SqlFront`.* `RelationSource` has two synchronous methods (`front.rs:57-65`); `related_nodes` and `flow_neighbors` sort
after collecting (`front.rs:123-131`), so a front supplies rows and the core owns the order. sqlx is async, so `SqlFront` is
a fetched hop: `async fn SqlFront::hop(relations: &Relations<'_>, frontier: &[(NodeType, StableId)]) -> HopRows` (the
snapshot handle of E) runs two indexed queries per hop and per chunk of 500 frontier ids (`impact` has no breadth cut,
`impact.rs:37-60`; SQLite bounds bind parameters): `WHERE scope_id = ? AND owner_type = ? AND owner_id IN (...)` over
`idx_relations_out` and the mirror over `idx_relations_in`. `HopRows` implements `RelationSource` over the fetched rows,
interning each name to the `&'static str` the trait returns (`front.rs:59-64`) through `declaration_for(owner, name).name`
or `LINKS` (`front.rs:17,69-74`) and refusing an undeclared name, so the trait is not edited; `related_nodes`,
`flow_neighbors`, and the executors' filters (`walk.rs:86-103`) run unchanged over it, and no operation gets a private walk.
A lookup outside the fetched frontier is an invariant violation (`debug_assert!` plus a test). `neighbors` is one hop;
`trace` and `impact` fetch one hop per depth, and both `seen` sets (`walk.rs:166`, `impact.rs:28,41`, id-only today) are
keyed by kind and id under entry 1. Rejected: a whole-table load per request; `block_in_place` (panics on the current-thread
runtime).

*Duplicate references.* `relations` is one row per (owner, relation, target) (`021:16`, `INSERT OR IGNORE` at
`relation_rows.rs:57-58`), while `outgoing_of` and `incoming_of` (`front.rs:250-266,283-289`) yield one row per stored
reference: a requirement citing one source under two clauses answers two `cites` neighbours today. Decision: one neighbor
per (relation, direction, endpoint), deduped in core `related_nodes` after its sort, so both fronts, wiki, gaps, `prime`
(`cache/prime.rs:152`), and the `impact` command (`cache/impact.rs:111`) agree; the two-clause fixture asserts `prime` lists
the source once. Derivation entry 1 (D).

*Front equivalence gate.* A property test materializes one scope and asserts `related_nodes` and `flow_neighbors` (both
ways) agree over `RecordFront` and `SqlFront` for every record, over the whole corpus of J.

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
projection tables behind the answer, in the family words of `projection_families.rs:69-90` plus `relations`; `live` names
what the stamp does not cover, from a closed list: `canonical` (canonical shards), `scanned_sites` (working-tree scan),
`verification_runs` (cache JSONL), `diff` (git). A stamp never implies freshness for anything it does not list; E makes both
lists true by construction. The engine dispatches generically (`engine.ts:20-28,47-53`), so only `protocol.ts` changes (J).

*Derivation version.* `pub const READ_DERIVATION: u32` in `operations/stamp.rs` (new), with a numbered history in its doc
comment. PR 1 ships `0` (today's semantics); PR 2 bumps to `1` with entry 1: one neighbor per (relation, direction,
endpoint) on the served operations, `prime`, and the `impact` command, and the trace and impact `seen` sets keyed by kind
and id. It bumps when reader logic changes an answer for the same rows; not for a migration (the serial moves; the digest
only with canonical bytes, `projection_digest.rs:32-52`) nor for a live-half fix. A golden test
(`operations/queries/tests/golden.rs`) answers a fixed request set over a frozen corpus from
`cache/tests/fixtures/golden.rs` (every kind and relation, retired records, over-limit lists, the two-clause citation and a
cross-kind `links` pair, so entry 1 is pinned; never `.provenance/state`, which moves on most PRs), digests the answers with
`stamp` and `freshness_error` removed, and compares to a committed file keyed by `READ_DERIVATION`, regenerated only in a
commit that bumps the constant.

*Per-operation table.* The array cells hold literal wire strings; the third column says what they cover. A pre-flip row (K)
reads `attested: []` and `live: ["canonical"]` plus its own live words.

| Operation | `attested` | `live` | What the two lists cover |
|---|---|---|---|
| get | the kind table read | | `found`, `node` |
| search | the kind tables searched | | `nodes` |
| neighbors | `relations`, the kind tables read | | `neighbors` |
| trace | `relations`, the kind tables read | | `nodes` |
| impact | `relations`, every kind table the walk read (`impact.rs:30-48`), `rules`, `implementation_bindings`, `verification_bindings` | `scanned_sites` | rule identities and binding rows attested; scanner sites in `implementations` and `verifications` live (`sites.rs:26-33,51-62`) |
| evidence | `implementation_bindings`, `verification_bindings`, `requirement_reviews` | `verification_runs`, `canonical`, `diff` | the three lists and `review_required` attested; `verification_runs`, `latest_verification_run`, and `stale` live (`stale.rs:36-40`) |
| stale | | `canonical`, `diff` | the whole answer |
| resolve_symbol | `implementation_bindings`, `verification_bindings`, `rules` | `scanned_sites` | binding matches and Rule records attested; the named file's sites live (`symbols.rs:31-38`) |

## E. Reader entry

Readers do not hold the publication guard while they answer. The guard covers the freshness write only; the answer is read
from a snapshot pinned inside one SQLite read transaction, so readers never queue behind a publication or behind each other.

*WAL, checked.* `open_cache` sets no journal mode (`cache.rs:34-39`), and sqlx 0.8.6 sets none by default (sqlx-sqlite
`options/mod.rs:177-181`: "Don't set `journal_mode` unless the user requested it"), so the pool runs SQLite's DELETE mode
today, under which a reader's shared lock blocks a writer's commit and a pending writer blocks new readers. `open_cache`
gains `.journal_mode(SqliteJournalMode::Wal)` (persistent in the file; `-wal` and `-shm` files sit beside it in the
gitignored cache directory; sqlx's 5 s `busy_timeout` stays, `:201`). In WAL a `BEGIN` read transaction pins the database as
of its first read; a writer appends to the log without disturbing it, a checkpoint cannot overwrite a page an open reader
needs, and readers never block anyone. Test: `the_cache_pool_runs_in_wal_mode`.

*Steps.* `operations/reader.rs` (new): `pub async fn answer<R>(repo, scope, policy, run: impl for<'c> FnOnce(&'c
ReadContext) -> LocalFuture<'c, R>) -> anyhow::Result<Stamped<R>>`, with `type LocalFuture<'c, R> = Pin<Box<dyn
Future<Output = anyhow::Result<R>> + 'c>>` written by hand (no `futures` crate exists in the workspace and none is added).
The future is `!Send` because `ReadContext` holds `RefCell`s; it is awaited on the calling task and never spawned
(`handlers::dispatch` runs on the `#[tokio::main]` task, `main.rs:16-21`; `#[tokio::test]` is current-thread). If `Send` is
wanted later, `tokio::sync::Mutex` replaces both `RefCell`s without a derivation bump.

1. Freshness under the guard. `catch_up`: take `publication_guard` (`guard.rs:68-83`), open the pool inside it
   (`cache.rs:34-39`; the guard already serialises every projection write, `docs/cache.md:55-59`, so two first reads on a
   fresh clone cannot both create the file or both switch it to WAL), run `catch_up_with_guard(&guard, &pool, layout)`
   (`catch_up.rs:37`, taking the reader's pool instead of opening its own at `:41`; re-exported from `materialize.rs` as
   `pub(crate) use catch_up::{catch_up_with_guard, CatchUpReport}`), the pass of `catch_up.rs:41-123` with its rebuild on a
   missing revision or an applied migration; then drop the guard. `annotate_only`: no guard, no catch-up; the pool opens
   with `create_if_missing(false)` and an absent file refuses (step 2). `refuse_stale`: reserved, refused as unimplemented
   with a typed error; W5 implements it.
2. When the freshness step fails (a validator refusal at `catch_up.rs:63-66` or `materialize.rs:52-55`, an I/O error) and
   the database holds a revision, the read goes on at the stored serial with `policy: "catch_up_failed"` and the error text
   in an additive `freshness_error`. An unwritable cache directory is the one case WAL changes: a WAL file cannot be read
   without a writable `-shm` unless the connection is `immutable`, so the reader reopens with
   `SqliteConnectOptions::immutable(true)` for the snapshot and stamps `catch_up_failed`; a read-only checkout thus answers
   at the serial it holds. With no revision, under any policy, the read refuses naming `provenance materialize`; under
   `annotate_only` so does a database behind on migrations (`applied_migrations`, `migrations.rs:170`). Tests:
   `a_read_answers_at_the_stored_serial_when_catch_up_refuses`, `a_read_with_no_database_refuses_and_names_materialize`.
3. `ReadSnapshot::open(&pool, scope)`: `BEGIN` on one pooled connection; the first statement reads the latest
   `projection_revision` row and the `projection_instance` row (`018:45-54`), which pins the snapshot, so every row read
   later in the transaction is at that serial by SQLite's own rule. Run `run(&ctx)`.
4. `Stamp::of(ctx, policy, freshness_error)` consumes the context and its snapshot; print.

*Types* (`operations/reader.rs`, `operations/stamp.rs`; every field private unless marked):

```
pub struct ReadSnapshot { tx: RefCell<Transaction<'static, Sqlite>>, scope: ScopeId, serial: i64, digest: String,
                          instance_id: String, attested: RefCell<BTreeSet<&'static str>> }
impl ReadSnapshot { pub fn table<K: ProjectionRow>(&self) -> Table<'_, K>;   // records K::TABLE in attested
                    pub fn relations(&self) -> Relations<'_>; }             // records "relations"
pub struct Table<'s, K> { .. }      // record(id), by_ids(ids), search(needle, include_retired), kind lookups
pub struct Relations<'s> { .. }     // hop(frontier) -> HopRows; SqlFront::hop takes &Relations, not a pool
pub struct ReadContext { snapshot: ReadSnapshot, live: RefCell<BTreeSet<Live>>, repo: Utf8PathBuf, scan_limit: usize }
pub enum Live { Canonical, ScannedSites, VerificationRuns, Diff }
impl ReadContext { pub fn snapshot(&self) -> &ReadSnapshot;
                   pub fn live(&self, what: Live) -> LiveHandle<'_>; }     // records the word in live
pub struct LiveHandle<'c> { .. }    // scan_tree, scan_file (ScannedSites); runs (VerificationRuns);
                                    // disturbed (Diff); store (Canonical): each method exists on its word only
pub struct Stamp { pub serial, pub digest, pub instance_id, pub derivation, pub policy, pub attested, pub live }
impl Stamp { pub fn of(ctx: ReadContext, policy: FreshnessPolicy, error: Option<String>) -> Self }  // the one way
```

sqlx executes on `&mut` the connection, so every `Table` and `Relations` method takes `tx.borrow_mut()` for the span of one
statement and never across two handles at once, which is how the executors read (sequentially). `Stamp::of` is the only
constructor and takes the context by value, so nothing reads after stamping and the stamp can describe only the snapshot the
rows came from. A projection table is readable only through `snapshot.table::<K>()` or `snapshot.relations()`, each of which
records its family word, so `attested` is derived from the handles handed out and cannot omit a table that was read; a live
half is reachable only through `ctx.live(..)`, which records its word: no handle, no scan, no diff, no run file, no
canonical read. A trybuild case (`read_after_stamp.rs`) pins that a table handle cannot outlive `Stamp::of`. The handles
guard omission, not excess, so an executor takes each handle at its use site (`Live::Diff` inside `base.map`, one `Table`
per kind inside `kind_of`'s loop), and a source-scan test in the pattern of `provenance-core/tests/relation_completeness.rs`
over `operations/queries/*.rs` (oracle excluded) refuses `open_cache`, `StateStore::new`, `ProvenanceLayout::new`,
`scan_path`, `scan_file`, `list_verification_runs`, and `git::`, since all are `pub` today (`cache.rs:34`, `stale.rs:8`);
with that gate the sentence above is a fact. An unflipped operation reads canonical through `ctx.live(Live::Canonical)`.

*Consistency.* The stamp's serial names the exact snapshot the answer reflects; a later publication is a later serial. Under
`catch_up` the snapshot is at or after the serial catch-up committed, since the guard dropped after the commit; another
process may publish and catch up between the drop and the `BEGIN`, and the stamp then says so. The only serialization left
is the freshness write (`docs/cache.md:55-59`); under `annotate_only` no lock is taken. The lock trap of `guard.rs:7-15`
shrinks to catch-up itself; `stale::disturbed` runs outside the guard, so its lock through `cache::graph_evidence`
(`stale.rs:36`, `health.rs:65`) is the brief one it takes today, and `graph_evidence_under_guard` is not needed. A test
asserts the lock is free while `run` executes.

*Policy and knobs.* `operations/read_policy.rs` (new): `enum FreshnessPolicy { CatchUp, AnnotateOnly, RefuseStale }`,
`struct ReadPolicy { freshness, scan_limit }`, defaults `CatchUp` and 2000 files (Ben measured 1.1 s for 657 files, so 2000
bounds the scan near 3.5 s). Reserved configuration names: `read.freshness_policy`, `read.scan_limit`; W3 ships the struct
as the seam, W5 the file. No journal knob, no visit budget, no request-side knob. The eight `queries::*` functions,
`handlers/sdk/query.rs:24-64`, and `handlers/sdk.rs:13` become async.

*Cost.* Under `catch_up` every read still copies the state tree (`catch_up.rs:59`), hashes every canonical byte
(`catch_up.rs:86-97`), and runs both validators first; W5 owns that (M.1). The answer takes no lock.

## F. Limits and truthful cut flags

There is no paging (`res_query_answers_stop_at_the_limit`, `boundary_query_answers_do_not_page`). Every operation keeps
`limit` (50 by default, 200 at most, `protocol.rs:28,31`) and `has_more` from `take_page` (`protocol.rs:84-88`); no request
gains a cursor and no response a next page. `evidence` gains four additive booleans, `implementation_bindings_has_more`,
`verification_bindings_has_more`, `verification_runs_has_more`, `reviews_has_more`, each the cut flag of its own `take_page`
call (`evidence.rs:60-63`); the top-level `has_more` keeps its OR meaning (`evidence.rs:85`); `EvidenceResponse`
(`protocol.ts:364`) gains the four.

## G. The scan limit

`scan_limit` lives on `ReadPolicy` (E), not on a request. `provenance-scanner/src/walker/bounded.rs` (new; `walker.rs` is
417) gains `scan_path_bounded(path, max_files) -> anyhow::Result<(Vec<FileScan>, bool)>`. Today's walk sorts after the loop
(`walker.rs:85-109`), so a cut in walk order would differ between clones; the bounded walk uses
`WalkDir::sort_by_file_name()` and counts only files that pass `Language::from_extension` (`walker.rs:98`); the cut set is
the first `max_files` language files in that order, and the bool says whether it stopped. `impact` calls it through the
`ScannedSites` handle and its response gains one additive field, `scan_cut: bool`; `true` says the scanned sites are a lower
bound. `resolve_symbol` does not walk the tree: it calls `scan_file` (`walker.rs:122`) on `repo.join(file)` alone, so it
never meets the limit, cannot miss the named file, and carries no `scan_cut`; a named file with no known extension
(`walker.rs:98-100` skips it today) or one that cannot be read yields no scanned sites, not an error; bindings still answer.

## H. Dangling-target existence validation for ideation targets

`IdeationTarget { artifact_type, artifact_id }` (`core/model/ideation.rs:276-281`) names seven kinds (`IdeationTargetType`,
`ideation.rs:14-29`: the six graph kinds plus domain; no boundary). Contributions and synthesis packets write it unchecked
in `create_contribution`, `upsert_contribution`, `create_synthesis_packet`, and `upsert_synthesis_packet`
(`state_store/ideation_writers.rs:8,16,127,135`; inputs at `inputs.rs:271,289`). The existence index covers four kinds
(`canonical_artifacts.rs:17-51,78-89`) and serves only dispositions (`proposal_writers.rs:363`,
`ideation_batches.rs:119,232`).

*Plan.* Extend `CanonicalArtifactIndex` to every `NodeType`, keyed by the serde word; add `ensure_target_exists(&Ideation
Target)` mapping `IdeationTargetType` onto `NodeType`; call it in the four writer functions only, so a new dangling target
is refused at write time. It is not added to `validate_graph_scope` (`graph_validation.rs:23-45`): that validator runs on
every catch-up pass and rebuild (`catch_up.rs:63-66`, `materialize.rs:52-55`) and cannot tell a new reference from an old
one, so one old dangling target would refuse every read. Existing state is reported: a gap pass
`cache/gaps/ideation_targets.rs` (new) emits `GapKind::DanglingReference` (`cache/gaps/model.rs:18`) with the wording of
`dangling.rs:52-56`, "target points at missing <kind> <id>", over contributions and synthesis packets, which `GraphRecords`
(`state_adapter.rs:22-32`) gains as two lists. `IdeationTargetType` gains `Boundary` so the superset is complete; this is PR
4 (K).

*Exposure: deferred.* Ideation and thread rows do not enter `relations` in this bead (cut plan L12); the gate: this
validation merged, `provenance gaps` clean on the repository's own state, and an owner decision on the relation name (the
vocabulary of `docs/cli.md:138-142` is closed).

## I. PR 180 leftovers assigned to W3

1. `INDIRECT` dead entry (`cache/impact.rs:41-47`): `contradicts` is `flow = none` (`shaping.rs:176`), so `flow_neighbors`
   never yields it (`front.rs:168`). Delete it; a test asserts every `INDIRECT` name has a declaration with a flow.
2. Cut plan L3: `is_resolved` (`cache/gaps/contradiction.rs:37-48`) lets a rejected resolution settle the pair. Fix: settled
   only when the named resolution exists and its status is not `Rejected` (`kinds.rs:94-108`); `supersedes` unchanged. RED:
   a question naming a rejected resolution still reports `UnresolvedContradictsPair`.
3. Cut plan L5: the neighbors order (`front.rs:123-131`) is frozen by the rank-order pin test and the golden file (D).
4. `contradicts` on the filter: a question owns it now (`state-format.md:32-34`), so `neighbors` of a requirement answers
   the question `in` and the other requirement is one more hop. One sentence in `docs/cli.md` after line 146; no code.

## J. Test strategy

*Oracle.* Before any flip, today's executors are copied into `operations/queries/tests/oracle/{records, walk, impact,
evidence, symbols}.rs` under `#[cfg(test)]`, `super::` imports rewritten to `crate::` paths so `bindings`, `sites`, and
`stale` stay the production modules; each carries a two-line header naming what it preserves and the commit that may delete
it.

*Differential harness.* `operations/queries/tests/differential.rs`: for each operation and a request set, run the oracle and
the served executor over the same store with `scan_limit` at `usize::MAX`, serialize both to `serde_json::Value`, strip the
additive fields (`stamp`, `freshness_error`, the four evidence flags, `scan_cut`), and assert equality; on the two-clause
and cross-kind fixtures it asserts the documented entry-1 delta instead. Corpus: the seeded store
(`queries/tests.rs:16-68`), the CLI and gap fixtures, new fixtures in a sibling `cache/tests/fixtures/` module
(`fixtures.rs` is 315): retired records that both reference and are referenced (`include_retired` both ways), the retired
diamond, a two-clause citation, over-limit lists, two over-long evidence lists, a cleared review; and a `tempfile::TempDir`
copy of the repository's own `.provenance/state` (`StateSnapshot`, `publication.rs:17-19`). No test opens the workspace root
through the reader: it would write `provenance.db` into the checkout and take the real publication lock, and parallel test
binaries would contend on both.

*A/B timings, in the same harness.* For each operation and request over the seeded store and the tempdir copy of the
repository's state, the harness times the oracle and the served executor (five runs after one warm-up, median wall time by
`Instant`), the served side under `annotate_only` so the freshness write is not in the number, and prints one row per case,
`operation request oracle_ms served_ms ratio`, one summary row per operation, and one `catch_up_ms` row per corpus; the rows
go to test stdout (`cargo test -p provenance-store differential -- --nocapture`; CI shows them on any failure). The scan is
outside every timing: both sides take it through the `ScannedSites` handle, fed from one scan taken before the clock starts,
so `impact` and `resolve_symbol` are timed on graph and binding work only, and the scan is timed once on its own; the
`impact` and `symbols` oracle copies gain a `scans: &[FileScan]` parameter in place of `scan_path(repo)` (`impact.rs:67`,
`symbols.rs:29`), the pre-taken scan is the tree scan, and the served `resolve_symbol` side is handed the one file's entry
from it, so both sides see the same sites. Ceilings, generous so the gate catches an order of magnitude and not noise: a
served case at most ten times its oracle median or 50 ms, whichever is larger, and never over 500 ms (Ben's 1.1-4 s for
`impact` today is the scan; the graph work is tens of milliseconds); the standalone scan at most 5 s; `catch_up_ms` over
this repository's copy at most 2 s. The ordinary `cargo test` on the three-OS matrix (`ci.yml:74`, parallel test threads on
shared runners) only prints; the ceilings assert when `PROVENANCE_AB_GATE=1`, set in one dedicated single-threaded ubuntu
job (`cargo test -p provenance-store differential -- --test-threads=1 --nocapture`, nine runs per case when gating) and in
the release run M.6, never in the matrix.

*Derive, tables, order, limits.* The round-trip and drift tests (C); trybuild refusals; the catch-up dumps compare the
widened tables and `relations` (`catch_up_behavior.rs:44-56`, `relation_rows_behavior.rs:11-22`); the front equivalence
property (C). An order-stability property inserts and retires records between two identical requests and asserts the
survivors keep their order. Over-limit scopes assert `has_more` and a page of exactly `limit`; the two-list evidence fixture
asserts its two flags (F); a repository past `scan_limit` asserts `scan_cut` on `impact`, that two runs cut the same file
set, and that sub-limit runs match `scan_path` (G); `resolve_symbol` on a file past the cut still answers.

*Stamp truthfulness.* The types of E replace the per-operation vocabulary tests of revision 4 with the trybuild case, the
source-scan gate, `a_table_handle_puts_its_word_in_attested`, and `evidence_without_a_base_lists_no_diff`. What remains, one
test per live constituent, checks what types cannot: mutate the constituent alone and assert the answer moves while serial
and digest stand still: a scanner annotation in the working tree (`scanned_sites`); a run appended to
`verification-runs.jsonl` (`verification_runs`); a commit touching a bound file (`diff`); a canonical edit under
`annotate_only` (`canonical`).

*Guard interleaving.* The `test_probes` pattern (`test_probes.rs:23-40`; `materialize_guard_behavior.rs:22-38`): the lock is
held during catch-up and free while `run` executes; a canonical write started during a read does not wait; a `ReadSnapshot`
A opened at serial N, then `catch_up_with_guard` on the same pool (its own connection; the default pool holds ten) with a
changed shard committing N+1, then A's `table` reads still matching N and a new `ReadSnapshot` reading N+1; `evidence` with
`base` returns; `evidence_without_a_base_lists_no_diff`.

*TypeScript.* `protocol.ts` gains `Stamp`, `stamp?`, `freshness_error?`, the four evidence booleans, and `scan_cut?`; the
type tests compile an old-shape response against the new types.

## K. Delivery

Four PRs: reader and stamp; the derive and migration (zero behaviour change, reviewed alone); the flips and leftovers;
section H. Each is green at every commit (fmt, clippy, the Rust and TypeScript suites) and deslopped before ready.

*PR 1, branch `1wh-w3-reader`: reader entry and stamp; nothing served from SQL yet.*
1. Oracle copies and the differential harness over the current executors (green against themselves).
2. `read_policy.rs` and `stamp.rs` with `READ_DERIVATION`; RED: `the_stamp_carries_the_stored_instance_id`.
3. `reader.rs` (`ReadSnapshot`, `ReadContext`, `Stamp::of`); WAL in `open_cache`; the eight operations go async and answer
   over `ctx.live(Live::Canonical)` with their pre-flip stamps; `catch_up_with_guard` takes the pool. RED:
   `the_cache_pool_runs_in_wal_mode`, `every_answer_carries_a_stamp_at_the_stored_serial`,
   `the_publication_lock_is_free_while_a_read_answers`, `a_canonical_write_does_not_wait_for_a_read`,
   `a_read_that_started_before_a_publication_answers_at_its_serial`, `a_table_handle_puts_its_word_in_attested`,
   `read_after_stamp` (trybuild), `no_query_module_bypasses_the_handles`, `evidence_without_a_base_lists_no_diff`,
   `a_second_opener_survives_the_wal_switch`, `a_read_only_checkout_answers_at_its_serial`,
   `a_canonical_edit_advances_the_serial_under_catch_up`, `a_read_answers_at_the_stored_serial_when_catch_up_refuses`,
   `a_read_with_no_database_refuses_and_names_materialize`; the A/B rows print from the first commit, and the gated job
   joins `ci.yml`.
4. Golden builder and file, `READ_DERIVATION = 0` with its key test, TypeScript `Stamp` types. Green: every existing
   response byte unchanged apart from `stamp`.

*PR 2, branch `1wh-w3-record-columns`: the derive and the mirrored tables; no flip.*
1. `ColumnValue` and the `ProjectionRow` trait in core; the derive in `provenance-macros/src/projection_row.rs`; trybuild
   refusals RED first (`a_tuple_struct_is_refused`, `a_field_named_search_text_is_refused`).
1. The derive on the eleven types; round-trip tests RED first, one per type, plus `a_round_confidence_stays_a_float`;
   `#[column(json)]` on `declaration_address` and `source_ref`.
3. Migration 022, `record_rows.rs` replacing the two loaders, catch-up reload; RED:
   `every_kind_table_mirrors_its_record_columns`, `materialize_writes_search_text_from_searchable_text`,
   `catch_up_equals_rebuild_over_the_widened_tables`, `a_link_under_two_kinds_keeps_both_relation_rows`.
4. `cache/read/{rows, records, front}.rs`; the dedupe in `related_nodes` and the front equivalence property RED first;
   `READ_DERIVATION` to `1` with entry 1 and the golden file regenerated in the same commit;
   `prime_lists_a_twice_cited_source_once`. Green: no served byte change except entry 1.

*PR 3, branch `1wh-w3-flips`: the seven flips, the leftovers, oracle removal.* After step 8 `records::load` and `find` move
under `#[cfg(test)]` (only the `records` oracle calls them) until W5 deletes them.
1. Flip `get`; RED: differential rows, `get_reads_a_retired_record_only_when_asked` over the `retired` column.
2. Flip `search`; RED: differential rows, `search_visits_kinds_in_rank_order_and_stops_at_the_limit`.
3. Flip `neighbors` and `trace`; RED: differential rows, the order pin, `trace_stops_at_the_limit_and_says_has_more`,
   `a_retired_origin_still_answers_its_live_in_neighbours`, `a_diamond_over_a_retired_node_answers_as_today`.
4. `scan_path_bounded` and `scan_limit`; flip `impact`; RED: differential rows, `impact_says_when_the_scan_was_cut`,
   `a_cut_scan_reads_the_same_files_twice`, `an_indirect_name_is_a_declared_relation_with_a_flow` (I.1),
   `a_resolution_reaches_the_rules_that_name_it_only`.
5. Flip `evidence`; RED: differential rows, `evidence_reports_which_list_was_cut`, `a_cleared_review_is_not_open`.
6. Flip `resolve_symbol` over `scan_file`; RED: differential rows, `resolve_symbol_reads_the_named_file_only`, and the two
   bindings-only cases (`_on_an_unscanned_extension_`, `_on_a_missing_file_`).
7. Stamp truthfulness tests per live constituent (J); `is_resolved` fix (I.2) RED:
   `a_rejected_resolution_does_not_settle_a_contradiction`.
8. Delete the walk, impact, evidence, and symbols oracles; docs (`docs/cli.md`: stamp, `freshness_error`, the evidence
   flags, `scan_cut`, the `contradicts` sentence; `docs/cache.md:3-10,63-73`: the read path, WAL, the widened tables);
   section B onto the bead; close `provenance-qrc.2`.

*PR 4, branch `1wh-w3-ideation-targets`: section H alone.* RED: `a_contribution_naming_a_missing_target_is_refused`,
`an_existing_dangling_target_is_a_gap_not_a_refusal`, `a_read_over_an_old_dangling_target_still_answers`.

*File-cap splits before growth.* The new modules named in C, G, and J, plus `operations/queries/tests/{reader, stamp,
limits, order}.rs` (`queries/tests.rs` is 175 and keeps its five). `walker.rs` (417) is not grown; `graph_records.rs` and
`integration_records.rs` are deleted; `response.rs` (116) gains the stamp, `freshness_error`, the four flags, and
`scan_cut`, with `Stamp` in `protocol/stamp.rs`; `protocol.ts` (399) splits into `protocol/query.ts` past 500.

## L. Risks and open points

1. The derive reads spelled types; a struct field without `#[column(json)]`, or a type alias, round-trips wrong; the
   round-trip tests are the gate.
2. A decoder that tries `i64` first would print `confidence` `1.0` as `1`: `REAL` affinity, storage-class decoding, fixtures
   at `1.0` and `0.95`.
3. Migration 022 drops and recreates eleven tables and `relations`; catch-up rebuilds after it, and nothing outside
   `materialize/`, `migrations.rs`, and `tests/` queries those tables today.
4. `search` prefilter equality (C): the prefilter is a superset, the Rust filter decides, and `differential.rs` pins it
   until W5.
5. The WAL switch cannot wait on `busy_timeout` (sqlx's own comment, `options/mod.rs:178-180`): the first `open_cache` on a
   DELETE-mode file another process holds open fails at once with `SQLITE_BUSY`. Default: `open_cache` retries the connect a
   bounded number of times on `SQLITE_BUSY` during the switch (idempotent once the file is WAL); test
   `a_second_opener_survives_the_wal_switch`.

*Rulings against the tree.* "One column for each field" disagrees with three tables today, and 022 aligns them (`018:35-36`,
`007:6-7`, no `schema_version`). The scan timing in the limit ruling is Ben's measurement. No review finding contradicts a
binding resolution.

## M. Release gate

W3 does not ship in a release on its own; W3 and W5 (bead `provenance-1wh.3`) ship together, so that bead gates the release
although it is P2 while `provenance-1wh.2` is P1; the bead text records the dependency, and raising it to P1 is the owner's
call. Before a release W5 lands:

1. Freshness without the tree copy: hash each scope in place under the guard and rebuild only the families that moved (today
   every `catch_up` read copies the tree and hashes every byte, E).
2. The scan default tuned from a measurement over more than one repository.
3. The `read.*` configuration file, wiring `ReadPolicy`.
4. Deletion of `records::load`, `records::find`, the `records` oracle, and `differential.rs`'s canonical side.
5. `refuse_stale` implemented as a typed refusal naming the gap between stamps.
6. The A/B rows (J) meeting their ceilings under `PROVENANCE_AB_GATE=1` on the release commit, numbers in the release notes.
7. `catch_up_failed` confirmed or renamed as the `policy` word for a failed freshness step (E.2).
8. Read-only checkouts under WAL: the `immutable` fallback of E.2 kept or replaced, and documented in `docs/cache.md`.
