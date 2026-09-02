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

## Catch-up

Catch-up is the steady-state refresh. It keeps no journal. A pass runs
under the publication guard on a snapshot of canonical state, runs the
same aggregate validator a rebuild runs (a refusal commits nothing), and
hashes the complete canonical bytes of each hash unit: one unit per
manifest scope (the scope's directory) and one global unit (every regular
canonical file under `state/` outside `scopes/`: the manifest, the edge
shards, the dictionary). A unit digest frames the relative path and the
bytes of every file, in sorted path order, and ignores the temporary
`.tmp*` files an interrupted atomic write leaves beside a shard.

An unchanged unit costs nothing more. A changed scope unit parses that
scope's families again and deletes and inserts only the families whose
content digest moved. A changed global unit reloads the edges table
whole; edge rows belong to the global unit and are never deleted on a
scope change or a scope departure. A departed scope loses its rows in the
eighteen scoped tables and its digest rows; a new scope loads. The
projection keeps the content digest and record count for each family and
scope, so the revision digest recipe from W1 is unchanged and is
reassembled from stored rows without parsing a shard.

A pass that changes rows, digests, or the unit set commits them together
with one new revision — serial equal to the stored serial plus one — in
one transaction. A pass that changes nothing commits no revision row: the
serial and the digest stand. A lost database rebuilds at serial one under
a fresh instance id.

The derived-field rule is covered by construction: a proposal card's
effective promotion state reads assertions, dispositions, and legacy
promotion decisions of its own scope, and the scope hash covers every byte
in that scope. The scope-locality invariant that this relies on — a
scope's rows derive only from files in that scope's directory or in the
global unit — is guarded by an instrumented rebuild whose recorded reads
must lie inside the hashed units.

Every projection write — rebuild and catch-up — holds an owned publication
guard: the lock belongs to an open file description rather than a thread,
acquisition waits on the blocking pool, and the guard stays held across
the asynchronous SQLite transaction from snapshot through commit, so no
canonical publication can interleave with a projection write.

## What each family's derivation reads

The file-to-family facts behind the scope-locality invariant.

| Family | Derivation | Files read |
|---|---|---|
| sources, domains, requirements, boundaries, topics, questions, resolutions, rules | `read_jsonl(<shard>)` | own shard |
| edges | `read_edge_shards` | every `edges/*.jsonl` (global unit) |
| threads | `read_jsonl(threads.jsonl)` | own shard |
| messages | `read_message_shards` | every `threads/YYYY-MM.jsonl` in the scope |
| implementation_bindings, verification_bindings | `read_jsonl(<shard>)` | own shard |
| requirement_reviews | `list_requirement_reviews`, direct line parse | own shard |
| contributions, synthesis_packets, assertion_records | `read_jsonl` plus the `landings.jsonl` overlay | own shard, `ideation/landings.jsonl` |
| dispositions | `read_jsonl`, legacy reader, landings overlay | own shard, `ideation/promotion_decisions.jsonl`, `ideation/landings.jsonl` |
| proposal_cards | `project_proposal_cards`: validator (validity only), then `effective_proposal_state(card, assertions, dispositions)` | `proposal_cards.jsonl`, `landings.jsonl`, `assertions.jsonl`, `dispositions.jsonl`, `promotion_decisions.jsonl`; the validator also reads `contributions.jsonl`, `synthesis_packets.jsonl`, and the manifest's actor list |

Every file in the table lies in the derived scope's directory or in the
global unit. Migrations are applied transactionally and record applied
versions in SQLite. Materialization runs the same lifecycle aggregate
validator used by direct writes, swarm landing, import, and `check` before
clearing or loading cache tables.

Typed SDK verification runs are also derived cache data, stored as JSONL under
`.provenance/cache/scopes/<scope>/verification-runs.jsonl`. They record local
or CI callback outcomes without changing Git-tracked canonical state. They may
be deleted with the rest of the cache; declarations in canonical state remain.
Verification runs stay outside the projection and outside its digest.
