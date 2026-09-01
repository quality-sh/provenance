# Cache

`.provenance/cache/provenance.db` is the stamped projection of canonical
JSONL: graph records, domains, shaping records, collaboration records,
ideation records, implementation and verification bindings, requirement
reviews, source commit pins, proposal confidence, assertions, and derived
proposal state. Canonical JSONL is the only write target. The database is
never the source of truth for writes; read authority is delegated to the
stamped projection, and the SDK query operations move onto it stage by
stage.

Each materialization stores a revision stamp beside the rows: a monotonic
serial, a projection digest, and a projection instance id. The digest
covers every stored family, so two repositories that hold the same
canonical records produce the same digest. The instance id comes from OS
entropy when the database first materializes; serials compare only within
one instance. The database can be deleted and rebuilt with
`provenance materialize`; loss degrades speed, never correctness.

Catch-up is the steady-state refresh. Writers that go through the shared
shard-mutation seam journal a hint under `.provenance/cache/journal/`;
writes that bypass that seam leave no hint and are found by the hash sweep
instead. A catch-up pass validates the aggregate, drains the hints, hashes
the complete byte domain of every stored family — every file that family's
readers read, month shards, edge shards, and the ideation landings overlay
included — and reparses only the families whose bytes moved. The journal
is never proof: a lost, gapped, or absent journal changes only speed,
because the hash sweep runs either way. Every projection write — rebuild
and catch-up — runs under the repository publication lock from snapshot
through commit.

Migrations are applied transactionally and record applied versions in SQLite.
Materialization runs the same lifecycle aggregate validator used by direct
writes, swarm landing, import, and `check` before clearing or loading cache
tables. Projection writes hold an owned publication guard: the lock belongs
to an open file description rather than a thread, acquisition waits on the
blocking pool, and the guard stays held across the asynchronous SQLite
transaction from snapshot through commit, so no canonical publication can
interleave with a projection write.

Typed SDK verification runs are also derived cache data, stored as JSONL under
`.provenance/cache/scopes/<scope>/verification-runs.jsonl`. They record local
or CI callback outcomes without changing Git-tracked canonical state. They may
be deleted with the rest of the cache; declarations in canonical state remain.
Verification runs stay outside the projection and outside its digest.
