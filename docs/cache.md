# Cache

`.provenance/cache/provenance.db` is the stamped projection of canonical
JSONL: graph records, domains, shaping records, collaboration records,
ideation records, implementation and verification bindings, requirement
reviews, source commit pins, proposal confidence, assertions, and derived
proposal state. Canonical JSONL is the only write target. The database is
never the source of truth for writes; read authority is delegated to the
stamped projection. Seven of the eight SDK query operations answer from it
(`get`, `search`, `neighbors`, `trace`, `impact`, `evidence`,
`resolve_symbol`); `stale` reads git and canonical shards and attests
nothing.

Each record kind has one table with one column for each field of its
Rust record type, named as the field; list and struct fields are JSON
text, and the eight kind tables carry one derived `search_text` column.
The three integration tables (`implementation_bindings`,
`verification_bindings`, `requirement_reviews`) mirror their types the
same way. A derive on each type writes the column list, the insert, and
the row decoder, so a new field cannot reach one side without the other.
The `relations` table holds one row per (owner, relation, target),
derived from the owner kinds' reference fields. Ideation targets and
thread parents do not enter it. The gate for that later change: the
write-time target check is merged, `provenance gaps` is clean on the
repository's own state, and the owner has named the relation, since the
relation vocabulary in `docs/cli.md` is closed.

Each materialization stores a revision stamp beside the rows: a monotonic
serial, a projection digest, and a projection instance id. The digest
covers every stored family, so two repositories that hold the same
canonical records produce the same digest. The instance id comes from OS
entropy when the database first materializes; serials compare only within
one instance. The database can be deleted and rebuilt with
`provenance materialize`; loss degrades speed, never correctness.

## Catch-up

Catch-up is the steady-state refresh. It keeps no journal. A pass runs
under the publication guard and reads canonical state in place. It
hashes the complete canonical bytes of each hash unit. There is
one unit per manifest scope, which is the scope's directory, and one
global unit, which is every regular canonical file under `state/` outside
`scopes/`: the manifest and the dictionary. A unit
digest frames the relative path and the bytes of every file in sorted
path order. It ignores the temporary `.tmp*` files an interrupted atomic
write leaves beside a shard. After reading the bytes, the hash lists the
files again. A changed file list fails the hash and names an added or removed
path. Catch-up and `refuse_stale` use this same check.

When both the global unit and a scope are unchanged, that scope is not
parsed or validated. The pass lists scopes from the exact manifest bytes
used in the global hash. A changed scope runs both the graph and ideation
validators and parses its families. Only families whose content digest moved are
rewritten. A changed global unit validates every scope and updates its
digest row. No family derives from the global unit. A changed scope unit also reloads the scope's `relations` rows for each owner
kind whose family moved. A departed scope loses its rows in the eighteen
tables, its `relations` rows, and its digest rows. A new scope loads. The projection keeps the content
digest and record count for each family and scope, so the revision digest
is reassembled from stored rows without parsing a shard.

After parsing a changed unit, the pass hashes it again. If the two hashes
match, the pass can use the parsed records and their digest. If an editor
that does not take the lock changes the bytes, the pass retries the unit.
After three failed attempts, the pass refuses and commits nothing. A read
then answers at the stored serial with `catch_up_failed` and the error
`canonical state changed during catch-up under <unit>`. A rebuild uses the
same hash, parse, and hash-again step in place.

A rebuild records `VALIDATION_VERSION` in `projection_validation`. A
catch-up pass rebuilds when the stored version differs from this build's
version, or when no version is stored. Validator changes that need to check
existing scopes increase this constant. `provenance materialize` and
`provenance check` always validate every scope.

A pass that changes rows, digests, or the unit set commits them together
with one new revision in one transaction. The new serial is the stored
serial plus one. A pass that changes nothing commits no revision row. A
lost database rebuilds at serial one under a fresh instance id.

Derived fields are covered by the scope hash. A proposal card's effective
promotion state reads assertions, dispositions, and legacy promotion
decisions of its own scope, and the scope hash covers every byte in that
scope. This relies on the scope-locality invariant: a scope's rows derive
only from files in that scope's directory or in the global unit. An
instrumented rebuild checks the invariant by recording every read and
asserting each one lies inside the hashed units.

Every projection write, rebuild and catch-up alike, holds an owned
publication guard. The lock belongs to an open file description rather
than a thread, acquisition waits on the blocking pool, and the guard stays
held from the first hash through commit. No canonical publication can
interleave with a projection write.

## Read path

A query read takes the guard for its freshness step only. Under the
default `catch_up` policy it opens the pool inside the guard, runs one
catch-up pass, and drops the guard; under `annotate_only` it takes no
guard and refuses a database that is absent, behind on migrations or
validation version, or half-migrated (a revision beside no family digests).
It then answers
from a snapshot pinned inside one `SQLite` read transaction, whose first
read is the stored revision, so every row read later is at that serial.
The database runs in WAL mode, so a reader never blocks a writer and a
writer never blocks a reader; the `-wal` and `-shm` files sit beside the
database. A projection table is readable only through the snapshot's
handles, which record the table's family word in the stamp's `attested`
list; a live part (canonical shards, the working-tree scan, the run
file, git) only through a handle that records its word in `live`. A
failed freshness step answers at the stored serial with the policy word
`catch_up_failed` and the error text beside the answer.

The settings in `.provenance/settings.json` survive a cache delete. The
file is tracked beside `state/` and `cache/`. Each query loads
`read.freshness_policy` and `read.scan_limit` before it opens the projection.
The `--freshness` flag takes precedence over the file; the file takes
precedence over `catch_up`. There is no environment variable. The scan
default is 5000 source files. Unknown keys and invalid values cause a typed
settings refusal, with no answer. A settings edit changes no state digest.
The stamp names the policy that ran; `scan_cut` reports a scan limit reached.

A completed read waits until the cache pool has no connections. A connection
that returns during shutdown is closed before the answer leaves the process.
SQLite can then finish the checkpoint and remove the -wal and -shm files.

A checkout whose cache directory this process cannot write can still read
its stored projection. WAL needs a writable `-shm` file beside the database.
A finished read removes the `-wal` and `-shm` files, so the reader opens the
database as an immutable image after a permission failure. SQLite then
reads the file without a lock. Other open errors do not select this fallback.
The scope list comes from an unlocked read of `manifest.json`. A publication
renames the old manifest away before it renames the new one into place, so an
unlocked reader sees one complete version or, for the width of those two
renames, no file at all. The reader waits out a missing file and retries, so
it never turns that window into a refusal. A lock file that cannot be opened
for writing does not prevent this schema check.

Under `catch_up`, the stamp says `catch_up_failed` and `freshness_error`
names the failed step. This word remains the policy outcome for a failed
catch-up step. Under `annotate_only`, the policy word is unchanged. The
answer is at the serial the file holds. Both `PermissionDenied` and
`ReadOnlyFilesystem` are permission failures. The immutable open is shared
by all three policies. Under `refuse_stale`, a permission failure on the
publication guard selects a shared lock on the existing lock file, opened for
reading. This lock excludes publication from before the scope list is read
until all unit hashes are complete. If the shared lock cannot be taken, or a
publication awaits recovery, the read refuses. A changed or unreadable unit
also refuses. Other guard failures refuse with their error.
The policy writes no revision and answers from the transaction used to check
the stored digests. Like `annotate_only`, it refuses an absent projection,
old migrations or validation, and a half-migrated projection.
These permission fixtures run
on Unix. A Linux test also uses an isolated read-only mount when user and
mount namespaces are available. It checks the mount options and attempts
file creation and a database write open before either read. If the mount
cannot prevent writes, the test prints a skip reason. The tests do not
reproduce these permissions on Windows.

## Long-running server handoff

Each call needs a new `ReadContext`. It holds one transaction at one serial
and is consumed by `stamp::seal`. A server must not keep a context between
calls: an open transaction retains its WAL snapshot and prevents a complete
checkpoint. These are the criteria from section J of the W5 release gate
plan, revision 3.

1. **Entry.** All eight functions in
   `crates/provenance-store/src/operations/queries.rs` accept an explicit
   `ReadPolicy`. For each call, load `Settings::load` from
   `.provenance/settings.json`, then call `ReadPolicy::resolve` once with
   those settings and the request's optional freshness override. Pass the
   resolved policy to the operation. Settings errors refuse before a read.
2. **Cost.** `cache::catch_up_state` refreshes the projection independently
   of a query. A server can use `annotate_only` between scheduled refreshes
   to avoid the per-call freshness hash. Pool open and query costs remain.
   `cache/tests/in_place_behavior.rs::an_unchanged_pass_parses_no_shard_but_the_manifest`
   checks that an unchanged catch-up reads no shard other than the manifest.
   The remaining freshness cost includes hashing all state bytes, the guard,
   and the pool open. Section C.1 of the plan records the repository and
   synthetic-tree costs before the in-place change. The release procedure
   in [release.md](release.md) records the new measurements.
3. **Refusal: pending stage K.3.** This tree returns
   `ReadRefusal::RefuseStaleUnimplemented`. The required typed stale refusal
   must include the stored serial and digest and the moved units. The
   `1wh-w5-refuse-stale` change supplies that behavior and its tests.
   The release cannot claim this criterion until that change is included.
4. **Process state.** `ReadFuture` is `Send`. The mutable probe state in
   `test_probes.rs` exists only under `cfg(test)`. Held publication access
   belongs to each `StateStore` (`state_store/access.rs`), with no shared
   process-wide set. The source scan
   `tests/read_process_state.rs::operations_do_not_change_the_process_environment`
   rejects environment mutation names throughout `operations.rs` and
   `operations/`, including imports under another name.
5. **Blocking sections.** Publication guard acquisition runs on the
   blocking pool (`publication/guard.rs::publication_guard`). The close
   lock never runs there: publication waiters can occupy every blocking
   worker while the closing read still holds the publication guard, so a
   close lock wait that needed a worker could never run. Each close lock
   attempt is a non-blocking `flock` on a runtime worker
   (`cache.rs::acquire_close_lock`), retried after a 1 ms sleep. The table
   below names synchronous work that remains on runtime workers. Moving
   this work to `spawn_blocking` is the first server task, outside W5.
6. **Concurrency and cleanup.** Each `reader::answer` opens a separate pool
   with one connection (`cache.rs::connect`) and a separate snapshot.
   Catch-up calls serialize on the publication file lock. The tests in
   `operations/queries/tests/concurrent.rs` keep both snapshots open at a
   barrier and check that both answers return the stored record count at
   the same serial. `concurrent_answers_finish_and_remove_the_wal_files`
   orders the closes. `answers_that_close_together_remove_the_wal_files`
   starts both closes together. Both tests run in the default suite and
   check that the -wal and -shm files are absent after both answers finish.
   `close_cache` closes the pool under `provenance.db.close.lock` in the
   cache directory and holds that lock until the pool holds no connection,
   which is after the physical SQLite teardown has finished. This orders
   closes across independent pools. Without that order, both SQLite closes
   can fail to get an exclusive database lock before either releases its
   shared lock, so both skip cleanup. The lock file stays in place for
   other callers. It is separate from the publication lock and contains no
   records. Immutable reads do not create or acquire it. A close lock
   failure returns an error after the pool closes.
   The lock belongs to a detached close task, not to the reader: a read
   cancelled mid-close, or one that panics there, cannot release the lock
   before its teardown finishes, so the order holds in both cases
   (`cache/tests/close_order_behavior.rs`). The close itself waits for the
   lock by retrying a non-blocking attempt every 1 ms, so the wait never
   needs a blocking worker and cannot deadlock behind publication waiters.
7. **Discovery.** `queries::served` calls `discover_repository` for each
   call. A server supplies a fixed absolute repository path through the
   `Option<Utf8PathBuf>` argument of each operation. It need not change
   the process working directory.
8. **Contract.** `packages/provenance/src/protocol.ts` defines `Stamp`,
   `QueryEnvelope`, the four evidence-list `has_more` flags, and `scan_cut`.
   A server preserves these fields and `freshness_error`. Until the contract
   layer defines a refusal envelope, a refusal is error text, with the
   stage K.3 stale details still pending on this tree.

Paths in this table are relative to `crates/provenance-store/src/`.
The hash estimates are from plan section C.1 and are not release limits.

| Section | Where | What it blocks |
|---|---|---|
| In-place hash of every unit under the guard | `cache/materialize/catch_up.rs::catch_up_with_guard`, through `validation.rs::UnitReader` and `units.rs` in the same directory | A runtime worker while hashing all state bytes under the guard: about 2 ms for this repository, about 100 ms at 38 MB in the plan's measurements |
| Validation and parse of a changed scope under the guard | `cache/materialize/validation.rs::UnitReader::scope`; `UnitReader::global` validates all scopes when the global unit changes | A runtime worker during validation and parse; an unchanged scope skips this work only in an incremental pass with an unchanged global unit |
| Full rebuild under the guard | `cache/materialize/catch_up.rs::catch_up_with_guard` and `rebuild` | A runtime worker during hashing, validation, and parse of all scopes when there is no stored revision, a migration was applied, or the validation version changed; unchanged canonical bytes do not skip this work |
| Manifest read retries without the guard | `operations/reader/freshness.rs::ensure_current_schema`, through `cache/materialize/units.rs::scope_ids` and `read_through_rename` | A runtime worker during synchronous file reads and parse; an absent manifest causes seven sleeps of 5, 10, 15, 20, 25, 30, and 35 ms before the eighth attempt, for 140 ms of sleep |
| Settings read for each request | `settings.rs::Settings::load`, called by the CLI handler or server before the query | The calling thread during the synchronous read and parse of `.provenance/settings.json` |
| Repository discovery for each query | `operations/queries.rs::served`, through `operations.rs::discover_repository` and `canonical_repository` | A runtime worker during synchronous filesystem checks and path canonicalization while locating the repository |
| `LiveHandle::graph_evidence` | `operations/reader/live.rs`, through `cache/health.rs::graph_evidence` | A runtime worker waiting in `flock` while another holder has the publication lock, then during canonical file reads and parse |
| Reads through `LiveHandle::store()` | `operations/reader/live.rs`, through `state_store/access.rs` | A runtime worker waiting in `flock` for canonical reads through the plain store, then during synchronous file reads and parse |
| `LiveHandle::runs` | `operations/reader/live.rs`, through `StateStore::list_verification_runs` | A runtime worker waiting on the verification run file's lock, then during the file read and parse |
| Close lock attempt for each cache close | `cache.rs::close_cache`, through `acquire_close_lock` | A runtime worker during the open of the lock file and one non-blocking `flock` attempt; a held lock is retried after a 1 ms sleep on a runtime timer, never on a blocking worker |
| Live source scans | `operations/reader/live.rs::LiveHandle::scan_tree` and `scan_file` | A runtime worker during synchronous directory traversal, source file reads, and scans |
| Live git reads | `operations/reader/live.rs::LiveHandle::resolve_range` and `disturbed`, through `stale/git.rs` | A runtime worker while git subprocesses run and their output is parsed |

The server must account for these costs when it moves live reads off
runtime workers.

## What each family's derivation reads

| Family | Derivation | Files read |
|---|---|---|
| sources, domains, requirements, boundaries, topics, questions, resolutions, rules | `read_jsonl(<shard>)` | own shard |
| relations | derived from the seven owner kinds' reference fields; no digest row | the scope's own shards |
| threads | `read_jsonl(threads.jsonl)` | own shard |
| messages | `read_message_shards` | every `threads/YYYY-MM.jsonl` in the scope |
| implementation_bindings, verification_bindings | `read_jsonl(<shard>)` | own shard |
| requirement_reviews | `list_requirement_reviews`, direct line parse | own shard |
| contributions, synthesis_packets, assertion_records | `read_jsonl` plus the `landings.jsonl` overlay | own shard, `ideation/landings.jsonl` |
| dispositions | `read_jsonl`, legacy reader, landings overlay | own shard, `ideation/promotion_decisions.jsonl`, `ideation/landings.jsonl` |
| proposal_cards | `project_proposal_cards`: validator (validity only), then `effective_proposal_state(card, assertions, dispositions)` | `proposal_cards.jsonl`, `landings.jsonl`, `assertions.jsonl`, `dispositions.jsonl`, `promotion_decisions.jsonl`; the validator also reads `contributions.jsonl`, `synthesis_packets.jsonl`, and the manifest's actor list |

Every file in the table lies in the derived scope's directory or in the
global unit.

Migrations are applied transactionally and record applied versions in
SQLite. Materialization runs the same lifecycle aggregate validator used
by direct writes, swarm landing, import, and `check`. A failed validation
commits no projection rows.

Typed SDK verification runs are also derived cache data, stored as JSONL under
`.provenance/cache/scopes/<scope>/verification-runs.jsonl`. They record local
or CI callback outcomes without changing Git-tracked canonical state. They may
be deleted with the rest of the cache; declarations in canonical state remain.
Verification runs stay outside the projection and outside its digest.
