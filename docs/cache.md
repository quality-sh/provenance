# Cache

`.provenance/cache/provenance.db` is generated from canonical JSONL. It
materializes graph records, domains, shaping records, source commit pins,
implementation and verification bindings, requirement reviews, assertions,
and derived proposal state, and it stamps that state with a projection
revision: one serial, one digest over every stored family, and the
projection instance the serial belongs to.

The database is the served read path. SDK queries answer from it inside one
publication guard, and every answer carries a freshness stamp naming the
domains the answer attests and the live constituents it does not - the git
diff behind `stale`, scanner sites from the working tree, and verification
runs stay live. `read.freshness_policy` picks how reads refresh the stamp:
`catch_up` (default) materializes what changed and serves locally,
`annotate_only` stamps without catching up, and `refuse_stale` refuses with
a typed error when the stamp cannot be made current.

It can be deleted and rebuilt with `provenance materialize`, and losing it
degrades speed, never correctness: canonical JSONL stays the sole write
target and durable truth. Serials mean nothing across projection instances,
so every stamp carries its instance identifier and clients must refuse
serial comparison across instances.

Writers record invalidation events in a journal beside the database. The
journal is a work hint for catch-up, never a freshness proof: every
full-freshness pass reads and hashes the complete canonical bytes of every
stored family, with the journal on or off.

Migrations are applied transactionally and record applied versions in SQLite.
Materialization runs the same lifecycle aggregate validator used by direct writes, swarm
landing, import, and `check` before clearing or loading cache tables. It copies canonical state
under the repository publication lock, then loads that coherent snapshot without holding a
synchronous filesystem lock across asynchronous SQLite work.

Typed SDK verification runs are also derived cache data, stored as JSONL under
`.provenance/cache/scopes/<scope>/verification-runs.jsonl`. They record local
or CI callback outcomes without changing Git-tracked canonical state. They may
be deleted with the rest of the cache; declarations in canonical state remain.
