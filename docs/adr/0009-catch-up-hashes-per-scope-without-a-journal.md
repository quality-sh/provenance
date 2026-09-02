# ADR 0009: Catch-up hashes per scope and needs no journal

## Status

Proposed, 2026-09-02. Awaiting human disposal on bead provenance-1wh.

If accepted, this record supersedes two parts of the W2 section of the
1wh plan (`docs/research/2026-08-27-qrspi-1wh-query-uniformity-plan.md`):
the per-family byte-domain declaration and the catch-up journal with its
shared serial space. Every other W2 position stands.

## Context

W2 of provenance-1wh makes `provenance.db` catch up to canonical JSONL
incrementally. The delivered design (PR 174, head `f121b55`) works in two
layers:

1. A byte domain per projection family. Each of the 19 families declares
   the canonical files its rows derive from. Catch-up hashes each family's
   domain, compares the digest with the stored one, and re-derives only the
   families whose bytes moved.
2. A catch-up journal. Every writer appends an event after each committed
   shard write. A durable head record and one monotonic sequence space,
   shared with the projection revision serial, let catch-up drain the
   events since the last revision and force re-derivation of the named
   families.

Four adversarial review rounds made this design truthful. They also showed
where it is fragile.

The byte domain is a hand-written mirror of what the readers do. Nothing
in the type system ties the two together. It was wrong twice:

- Messages, edges, and five ideation families derive rows from files the
  declaration did not name: month shards, extra edge shards, and the
  ideation landings overlay. Catch-up stamped full freshness while a landed
  batch never reached the projection.
- Proposal cards compute `promotion_state` from assertions, dispositions,
  and legacy promotion decisions through `effective_proposal_state`. Those
  three files were absent from the declared domain. A journaled rejection
  left the projected card `asserted` under a fresh revision stamp.

Both misses produced the one failure the design exists to prevent: a
confidently stamped stale answer. A completeness gate and a derivation
audit table now guard the declaration. They make a third miss harder, not
impossible. Every future reader change is a chance to reopen the gap.

The journal is the source of a third of all review findings on the branch:
sequence reuse after a crash between commit and head fsync, allocation
that ignored the stored serial floor, a journal I/O error that failed a
canonical write which had already committed, prune before commit, a head
record inside the deletable events directory, and one declared open
boundary (events directory and head lost together while the database
lives can orphan hints below the serial).

The plan itself limits what the journal may do. Its truth rule says a pass
may claim full freshness only when it "read and hashed the complete
canonical bytes of every stored (scope, family)", and that "the journal is
a work hint" that "can never prove that unjournaled bytes are unchanged".
Step 3 of catch-up therefore hashes every family on every pass whether or
not the journal named it. The digest comparison already decides what to
re-derive. The journal changes no answer and saves no hashing. Today it
adds crash windows, durable state, and code, and returns nothing the
digest does not already return.

Canonical state in this repository is small. Every pass already reads and
hashes the complete canonical tree. No measurement shows that cost as a
problem.

## Decision

Catch-up hashes canonical bytes per scope, not per family, and the
projection keeps no journal.

### Unit of freshness

The unit is one scope. One additional global unit covers every canonical
file outside a scope directory, including the edges shards and the
manifest. A unit digest hashes, in sorted relative-path order, the path
and the complete bytes of every regular file under the unit in the
snapshot taken under the publication guard. The projection stores one
digest row per unit. The revision digest continues to derive from stored
rows, as W1 defines it.

### Catch-up pass

Under the publication guard, on the snapshot:

1. Run the aggregate validator for every scope. A refusal commits nothing.
2. Compute each unit's digest and compare it with the stored digest.
3. For a changed unit, delete that unit's rows from all 19 family tables
   and reload the unit from the snapshot. For a departed unit, delete its
   rows and its digest row. For a new unit, load it.
4. Leave an unchanged unit untouched: no reparse, no row write.
5. Commit the rows, the unit digest rows, and the new revision in one
   SQLite transaction. The revision serial is the stored serial plus one,
   scoped to the projection instance. Total cache loss restarts at one
   under a fresh instance id.

A pass that cannot read a unit's complete bytes fails and commits nothing.
Size, modification time, and any other metadata never substitute for the
hash.

### What is removed

The journal module, the head record, the shared sequence space, the
pre-commit serial reservation, the prune step, the writer-side emission
hook at the shard-write choke point, the file-to-family mapping, and the
per-family byte-domain declaration. The `cache.catchup_journal`
configuration knob planned for W5 is not built.

### What stays

The owned async publication guard and its capability helpers, snapshot
under the guard, the validator on every pass, baselines hashed from the
snapshot, the stamp with serial, digest, and instance id, departed-unit
cleanup, delete-then-insert replacement, and the equivalence tests that
compare catch-up with a total rebuild across all 19 tables.

The projection family remains the unit of tables and loaders. It no longer
carries an input-file declaration.

### Precision

A change to any file in a scope re-derives every family in that scope.
This is the accepted cost. W5 measures pass time under real serving load.
If a measurement shows the cost matters, finer grain returns as an
optimization guarded by the equivalence tests, not as a prerequisite for
serving.

## Alternatives considered

Keep the per-family domain and the completeness gate. Rejected: the gate
checks that declared files exist and that reference fields are declared;
it cannot see a reader that computes a field from another family's file.
That is the exact shape of the second miss.

Derive the domain from the readers by instrumenting file access during a
rebuild. Rejected for now: more machinery to prove correct, and a per-scope
hash makes the question moot at current sizes.

Keep the journal and drop only the shared sequence space. Rejected: the
journal still names families, so it still depends on the mapping this
record removes, and it still changes no answer.

## Consequences

Positive:

- No parallel description of reader behavior exists to fall out of sync.
  A derived field is covered by construction.
- Three of the five crash windows no longer exist. The serial has one
  writer, in one transaction, under one lock.
- The publication choke point no longer carries a second I/O path, so a
  journal failure can no longer touch a canonical write.
- The code that W3 and W5 build on is smaller and has fewer states.

Negative:

- Coarser re-derivation. One message post reparses its whole scope.
- Losing the per-family digest rows means the first pass after upgrade
  rebuilds the projection once. The migration that adds the unit digest
  table records this.

Plan deviations to record on the bead: the journal section, the serial
space section, the byte-domain declaration, trigger 1's "precise event"
detection (it becomes the hash, as triggers 2 and 3 already are), and the
W5 journal knob.

## Verification

The change is complete when:

- The equivalence property holds for every supported trigger: catch-up
  output equals a total rebuild, rows and digest, after writer commits,
  hand edits, imports, scope departure, and schema migration.
- A mutation that skips hashing any unit, or trusts size and modification
  time, turns at least one test red.
- A mutation that leaves a deleted record's rows in place turns a test red.
- The crash tests that remain (before commit, after commit) hold, and the
  journal-specific tests are deleted rather than skipped.
- No production code path reads or writes a journal, a head record, or a
  family byte domain.
