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
write leaves beside a shard.

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
publication guard selects an unlocked hash over the same scope list. A changed
or unreadable unit refuses. Other guard failures refuse with their error.
The policy writes no revision and answers from the transaction used to check
the stored digests. Like `annotate_only`, it refuses an absent projection,
old migrations or validation, and a half-migrated projection.
These permission fixtures run
on Unix. A Linux test also uses an isolated read-only mount when user and
mount namespaces are available. The tests do not reproduce these
permissions on Windows.

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
