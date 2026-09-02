# ADR 0009: Catch-up hashes per scope and needs no journal

## Status

Proposed, revision 2, 2026-09-02. Awaiting human disposal on bead
provenance-1wh.

Revision 2 folds in an adversarial review (verdict: accept with
amendments). The amendments changed the unit-to-table mapping, kept the
per-family content-digest rows, adopted per-family re-derivation on a
changed scope, ruled on the serial for a pass that changes nothing, and
completed the list of plan and bead positions this record supersedes. A
second independent review is pending; a further revision folds it in if
it adds anything.

If accepted, this record supersedes these parts of the W2 section of the
1wh plan (`docs/research/2026-08-27-qrspi-1wh-query-uniformity-plan.md`)
and of the rulings on bead provenance-1wh:

- The per-family byte-domain declaration.
- The catch-up journal, the durable head record, and the shared serial
  space.
- Trigger 1's "precise event" detection. It becomes the hash, as triggers
  2 and 3 already are.
- Dispatch criterion 7 of 2026-09-01 (journal events keyed by declared
  family) and criterion 8 (serial normalization precedes journal
  allocation).
- W5 stage 2 "journal default ON" and the `cache.catchup_journal` knob.
- Plan section E's out-of-scope clause "journal emission inside the
  already-locked section".
- The acceptance-checklist rows "Serial restart after pruning" and
  "Metadata-only comparison, journal on with a drained window".

Every other W2 position stands.

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

### The byte domain is a mirror nobody checks

The byte domain is a hand-written match over families. The readers that
derive rows are separate functions. Nothing in the type system ties the
two together. Two families (messages, edges) reuse the readers' own
discovery functions; the other seventeen are declared by hand. The
declaration was wrong twice:

- Messages, edges, and five ideation families derive rows from files the
  declaration did not name: month shards, extra edge shards, and the
  ideation landings overlay. Catch-up stamped full freshness while a landed
  batch never reached the projection.
- Proposal cards compute `promotion_state` from assertions, dispositions,
  and legacy promotion decisions through `effective_proposal_state`. Those
  three files were absent from the declared domain. A journaled rejection
  left the projected card `asserted` under a fresh revision stamp.

Both misses produced the one failure the design exists to prevent: a
confidently stamped stale answer. The tests that now guard the declaration
compare it with a second hand-written list, and the per-family
invalidation test had to be taught by hand that cards derive from
dispositions. They make a third miss harder, not impossible. A reader
that computes a field from another family's file is invisible to them.

### The journal changes no answer and adds work

The plan limits what the journal may do. Its truth rule says a pass may
claim full freshness only when it "read and hashed the complete canonical
bytes of every stored (scope, family)", and that "the journal is a work
hint" that "can never prove that unjournaled bytes are unchanged". Step 3
of catch-up therefore hashes every family on every pass whether or not
the journal named it. The digest comparison already decides what to
re-derive.

The review probed this at `f121b55`. The journal's only effect on the
sweep is that a drained family is re-derived even when its digest matches.
A no-op write that leaves the shard byte-identical still produces one
drained event, one re-derived family, and one row write, with every stored
row and digest identical before and after. The journal never changes an
answer. It can only add work.

Of the 28 findings across the four review rounds on PR 174, 13 were
journal-specific: sequence reuse after a crash between commit and head
fsync, allocation that ignored the stored serial floor, a journal I/O
error that failed a canonical write which had already committed, prune
before commit, a head record inside the deletable events directory, and
one declared open boundary (events directory and head lost together while
the database lives can orphan hints below the serial). The two
rejection-grade findings both came from the byte-domain declaration.

### Cost today

Canonical state in this repository is 1.1 MB in 17 files, one scope,
1,307 projected rows. Release-profile measurements at `f121b55`:

| Step                                        | Cost      |
|---------------------------------------------|-----------|
| Guard and snapshot copy                     | 1.3 ms    |
| Hash every family's bytes                   | 6.8 ms    |
| Aggregate validator, per scope              | ~11 ms    |
| Parse and canonicalize all 19 families      | 32 ms     |
| Unchanged pass, total                       | 22-27 ms  |
| Re-derive one family                        | 25-62 ms  |
| Re-derive every family in the scope         | 105-128 ms|

The hash is not the marginal cost. Parsing and row writes are. The
unchanged-pass floor is dominated by the validator and the snapshot copy,
which every design here keeps.

## Decision

Catch-up hashes canonical bytes per scope, re-derives per family by
content digest, and the projection keeps no journal.

### Units

The unit of byte freshness is one scope. One additional global unit covers
every canonical file outside a scope directory: `manifest.json`, the edges
shards under `edges/`, and `dictionary.json`. A unit digest hashes, in
sorted order, the relative path and the complete bytes of every regular
file under the unit in the snapshot taken under the publication guard. The
projection stores one byte digest per unit.

A scope unit owns the 18 scoped family tables, filtered by `scope_id`. The
global unit owns the `edges` table whole. A scope change never deletes
edge rows, and a departed scope keeps its edge rows, because a total
rebuild loads the whole edges shard regardless of the manifest and
catch-up must stay equivalent to it.

### Per-family content digests stay

The projection keeps one content digest and record count per (family,
scope), as W1 defines them. These rows derive from parsed records, not
from a declaration of inputs, so they are not a mirror. The W1 revision
digest continues to derive from them unchanged. Only the per-family shard
byte digest, size, and modification-time columns lose their purpose.

### Catch-up pass

Under the publication guard, on the snapshot:

1. Run the aggregate validator for every scope. A refusal commits nothing.
2. Compute each unit's byte digest and compare it with the stored one.
3. For an unchanged unit, do nothing: no reparse, no row write.
4. For a changed scope unit, parse all 19 families of that scope from the
   snapshot and compute their content digests. Delete and reinsert only
   the families whose content digest moved. Store the new unit digest and
   the new content digests.
5. For a changed global unit, reload the edges table whole.
6. For a departed scope, delete its rows from the 18 scoped tables and its
   digest rows. For a new scope, load it.
7. If any step changed a row, a digest, or the unit set, commit the rows,
   the digests, and one new revision in one SQLite transaction. The
   revision serial is the stored serial plus one, scoped to the projection
   instance.
8. If nothing changed, commit no revision row. The serial and the revision
   digest stay as they were.

A pass that cannot read a unit's complete bytes fails and commits nothing.
Size, modification time, and any other metadata never substitute for the
hash.

Rule 8 is new. At `f121b55` every pass inserts a revision row, so the
serial moves on passes that change nothing. Under the W3 `catch_up` read
policy every read runs a pass, so a revision-bound cursor from page one
would be invalid by page two. With rule 8 the serial names a change, and
a cursor bound to it survives a pass that found none. Rule 8 also removes
the unbounded growth of the revision table that W2 deferred to W5.

Total cache loss restarts the serial at one under a fresh instance id. The
first materialization of an instance is serial one; the W1 assertions
that W2 relaxed (because journaled writer events consumed sequences before
the first materialization) are restored.

### What is removed

The journal module, the head record, the shared sequence space, the
pre-commit serial reservation, the prune step, the writer-side emission
hook at the shard-write choke point, the file-to-family mapping used by
the journal, the per-family byte-domain declaration, and the shard byte
digest, size, and modification-time columns. The `cache.catchup_journal`
configuration knob planned for W5 is not built. `CatchUpReport` loses
`events_drained`; `families_hashed` becomes `units_hashed`.

The path-to-family mapping in `merge/validation.rs` (`ShardFamily`) is a
separate concern of the merge validator and stays.

### What stays

The owned async publication guard and its capability helpers, snapshot
under the guard, the validator on every pass, baselines hashed from the
snapshot, the stamp with serial, digest, and instance id, departed-unit
cleanup, delete-then-insert replacement per family, and the equivalence
tests that compare catch-up with a total rebuild across all 19 tables.

The projection family remains the unit of tables, loaders, and content
digests. It no longer carries an input-file declaration.

### Precision

A change to any file in a scope reparses every family in that scope
(32 ms here) and rewrites only the families whose records changed. The
row writes, which dominate a full re-derive, are paid only where records
moved. At ten and one hundred times the current canonical size the
reparse extrapolates to roughly 0.3 s and 3 s per changed scope, against
an unchanged-pass floor of roughly 0.25 s and 2.5 s. W5 measures pass
time under real serving load against the baseline above. If a
measurement shows the reparse matters, finer grain returns as an
optimization guarded by the equivalence tests, not as a prerequisite for
serving.

## Alternatives considered

Keep the per-family domain and its guard tests. Rejected: the guard
compares two hand-written lists. It cannot see a reader that computes a
field from another family's file, which is the exact shape of the second
miss.

Derive the domain by instrumenting file reads during a rebuild. Rejected:
one family's reader bypasses the shared read path, so the instrumentation
would have to sit at the filesystem layer; and a recorded domain is data
captured at the last derivation, so a new binary that reads a new file
inherits the old domain until a rebuild. That is the same miss class,
deferred, and it needs a rebuild-on-binary-version rule to close.

Hash per scope and re-derive the whole scope. This was revision 1 of this
record. Superseded by the decision above: it deleted the content-digest
rows the revision digest is built from, and at one hundred times the
current size it would pay every row write in a scope for every change,
roughly 11 s per read after write under the W3 `catch_up` policy.

Keep the journal as an optional accelerator with no serial coupling.
Rejected: it still needs a file-to-family mapping, and the review showed
the hint can only add work.

## Consequences

Positive:

- No parallel description of reader behavior exists to fall out of sync.
  A derived field is covered by construction. A change to the canonical
  layout needs no change to freshness detection.
- Three of the five crash windows no longer exist. The serial has one
  writer, in one transaction, under one lock.
- The publication choke point no longer carries a second I/O path, so a
  journal failure can no longer touch a canonical write.
- The serial names a change. Cursors bound to it survive no-op passes.
- The code that W3 and W5 build on is smaller and has fewer states.

Negative:

- A changed scope reparses all of its families, even when one record
  moved. Row writes stay per family.
- The first pass after upgrade rebuilds the projection once, because an
  applied migration routes catch-up to a total rebuild.

## Tests

Journal-specific tests are deleted, not skipped: the journal unit tests,
the emission tests, the serial-space tests, and the crash tests at the
journal-append, head-advance, and prune points.

These properties keep their meaning without a journal and are re-homed
rather than deleted:

- A writer commit reaches the projection on the next pass, detected by
  the hash.
- Two consecutive passes over unchanged state write zero rows and commit
  no revision.
- A crash after commit leaves consistent readable state; the next pass
  finds nothing to do.
- A concurrent rebuild and catch-up under the guard produce a single
  serial progression.
- Total cache loss restarts at serial one inside a fresh instance.
- Database lost with the canonical tree intact rebuilds at serial one.

## Verification

The change is complete when:

- The equivalence property holds for every supported trigger: catch-up
  output equals a total rebuild, rows and digest, after writer commits,
  hand edits, imports, scope departure, and schema migration.
- A mutation that skips hashing any unit, or trusts size and modification
  time, turns at least one test red.
- A mutation that deletes edge rows on a scope change turns a test red.
- A mutation that leaves a deleted record's rows in place turns a test
  red.
- A mutation that commits a revision row on a pass that changed nothing
  turns a test red.
- The W1 `serial == 1` assertions are restored and green.
- No production code path reads or writes a journal, a head record, or a
  family byte domain.

## Recorded for later

Two items outside this decision surfaced in the same review and are
recorded here so they are not lost with the journal tests:

- Plan trigger 4 says a cache that fails to open or migrate routes to a
  total rebuild. At `f121b55` the open failure returns an error. This
  belongs with the W5 reader policy.
- The read-only bypass returns a lockless guard by design, matching the
  synchronous path. A materialization under read-only validation would run
  unlocked. The guard's documentation should say so.
