# Adversarial review: ADR 0009 and the W2 catch-up code (GLM-5.3-flash)

- Reviewer: GLM-5.3-flash agent dispatched through workflowd (route review, run agent-run/2348f999fab6b580, report commit 5310050)
- Dispatched from Claude Code session https://claude.ai/code/session_01NQ2v24jQRt4G9Yzu2LFMcc on 2026-09-02
- Reviewed: ADR 0009 revision 1 (commit d000bb8) and the W2 code at f121b55

The report below is the agent's committed report, reproduced verbatim.

---

# ADR 0009 review — catch-up hashes per scope, no journal

Adversarial architecture review of `docs/adr/0009-catch-up-hashes-per-scope-without-a-journal.md`
(branch `1wh-adr-0009-scope-hash-catch-up`) against the delivered W2 code at
f121b55 (draft PR 174, branch `1wh-w2-incremental-catch-up`) and the governing
plan `docs/research/2026-08-27-qrspi-1wh-query-uniformity-plan.md`
(branch `opencode/provenance-20260827T223718Z-87cc1ac4`).
All file:line evidence is at f121b55 unless noted.

## Verdict

**ACCEPT WITH AMENDMENTS**, with these amendments, each expanding text that is
currently wrong, unspecified, or silently load-bearing:

1. **Fix the row-replacement rule.** Decision §Catch-up pass step 3 says
   "delete that unit's rows from all 19 family tables and reload the unit."
   That is wrong for the edges family: edge rows carry `scope_id`
   (migrations/002_sources_requirements_edges.sql:22-31, PK `(scope_id, id)`)
   but are derived from the *global* edge shards, so a scope unit's rows do not
   include its edges rows and a scope unit cannot reload them from scope bytes.
   State the actual mapping: a scope unit replaces the 18 scoped families for
   that scope; the global unit replaces the edges family.
2. **Specify the unit digest framing and the unit set.** The unit digest must
   frame the *relative path* (not the basename as the existing helper does,
   `projection_families.rs:301-318`) — `implementations/binding.jsonl` and
   `verifications/binding.jsonl` share a basename (shards.rs:61-70), and
   basename framing cannot see their contents swap. Filter write residue
   (`jsonl.rs:99` puts the temp file beside the shard) and define the unit set
   as: manifest scopes plus every regular file under `state/` outside
   `scopes/` (that set is wider than "the edges shards and the manifest":
   `state/dictionary.json` exists, dictionary_reference.rs:11).
3. **Say what happens to the revision digest's definition.** "The revision
   digest continues to derive from stored rows, as W1 defines it" contradicts
   "one digest row per unit" unless the pass keeps carrying per-unit *content*
   digests forward for unchanged units. If the revision digest instead hashes
   raw unit file bytes, the documented W1 property "two repositories that hold
   the same canonical records produce the same digest" (docs/cache.md:9-12)
   dies, because the digest becomes shard-split dependent. Pick one and record
   it; the cheap option is unit decision-digest (file bytes) + unit
   content-digest (carried forward) in the new table, which preserves the W1
   recipe modulo a one-time value change.
4. **State scope-locality as a guarded invariant.** The ADR removes a
   hand-maintained mirror and replaces it with an unstated hand-maintained
   rule: *no family's derivation may read outside its scope directory or the
   global unit*. Nothing guards it. Add the cheap guard: instrument one
   rebuild per CI run (record every state path the loaders read) and assert
   the recorded read-set is inside the units the pass hashed.
5. **Record the open-question-7 dependency.** Removing the journal removes the
   accidental protection where a surviving journal/head floors the serial above
   a lost database's history (journal.rs:130-139; test
   `a_lost_database_with_a_live_journal_reseeds_above_the_tail`). After this
   ADR, deleting `provenance.db` alone restarts the serial at 1 under a fresh
   instance id, and cross-instance serial comparison safety rests entirely on
   the plan's still-open question 7. Say so on the bead.

## Findings (most severe first)

1. **Decision step 3 as written loses or corrupts edges rows.** Edges is a
   global family whose rows are per-scope keyed. "Delete that unit's rows from
   all 19 family tables" for a scope unit would delete that scope's edges rows
   with no way to reload them from scope bytes (the loader table maps Edges to
   the global loader only, family_rows.rs:43-45 and state_store.rs:166-168);
   the only scope-filtered edge reader (`closed_edges`, state_store.rs:243-245,
   readers.rs:397-430) parses with `Fields::Closed` while the projection
   loader parses `Fields::Open` (readers.rs:359-371), so a row with an extra
   field would make catch-up fail where rebuild succeeds. Conversely, for the
   global unit, "all 19 family tables" is 18 tables too many. Amendment 1.

2. **The unit hash helper that exists today is unsafe for units.**
   `domain_bytes` frames by `path.file_name()` only
   (projection_families.rs:309-311). Reused for per-scope units, a content
   swap between `implementations/binding.jsonl` and
   `verifications/binding.jsonl` produces the identical framed stream — a
   confidently stamped stale answer of exactly the kind this design exists to
   prevent. The ADR's text ("the path and the complete bytes") is correct; the
   code it would reuse is not. Amendment 2 plus a swap mutation test.

3. **The revision digest contradiction (amendment 3).** The W1 recipe walks
   `ProjectionFamily::ALL × scopes` over *derived record* bytes
   (projection_digest.rs:33-65) and is reassembled from stored content digests
   (projection_digest.rs:74-122). Unit digest rows over raw files cannot
   reproduce it; the ADR asserts both. Unamended, the implementer either
   silently changes the digest domain (breaking docs/cache.md:9-12 and the W1
   sensitivity tests' meaning) or quietly keeps per-family rows (failing the
   ADR's own verification bullet that no code reads a family byte domain).

4. **The scope-locality invariant is load-bearing and unguarded (amendment 4).**
   Verified by tracing every `list_*` (state_store.rs:148-352) and loader
   (family_rows.rs:37-89; graph_records.rs; collaboration_records.rs;
   integration_records.rs): every scoped family reads only files under its
   scope directory today — including the three that were once got wrong
   (month shards under `threads/`, readers.rs:320-345; landings overlay,
   state_store.rs:263-267; assertions/dispositions/promotions feeding cards,
   state_store.rs:305-306). Nothing enforces this for the next reader. The
   ADR's own argument against the byte-domain gate — "it cannot see a reader
   that computes a field from another family's file" — applies verbatim to an
   unguarded locality rule. The difference is that the rule is one sentence
   instead of nineteen declarations, which is a real improvement, but only if
   it is written down and checked.

5. **Serial restart now crosses instances unprotected (amendment 5).** Today,
   `normalize_head` floors the head at the stored serial and the tail
   high-water (journal.rs:130-139), so a lost database with a surviving journal
   never reuses serials downward. The ADR removes both, so serial restart at 1
   happens whenever the database is deleted, and the only client protection is
   the instance id — which the wire stamp does not yet carry (plan open
   question 7, still unresolved). Latent today (nothing serves stamps yet),
   but the ADR increases OQ7's importance without recording it.

6. **Unit-set and residue specification.** `write_jsonl_atomic_unlocked`
   creates its temp file inside the shard's own directory (jsonl.rs:99); a
   crash leftover permanently enters the unit digest, and so does any stray
   file a tool drops under `state/` (`state/dictionary.json` already exists).
   Consequences are benign (spurious re-derivation, non-determinism across
   machines that hold identical records), but the ADR should say "regular
   `*.jsonl` files under the unit" or equivalent, and the unit-set sentence
   should not enumerate "the edges shards and the manifest" as if that were
   the complete set of files outside scopes/.

7. **Lost coverage that still matters.** The ADR rightly deletes journal tests,
   but three deletions take real knowledge with them: (a) the journal mapping
   tests (`journal_emission.rs:24,45`) were the only *executable* record of
   which files feed which families — after the ADR, that knowledge exists
   nowhere, not even in prose (see finding 4's guard as replacement); (b) the
   derivation audit table (PR 174 body, round 4) is prose in a PR body that
   squash-histories poorly — copy it into docs/cache.md or the ADR; (c) the
   head/tail crash tests go, which is correct, but the ADR's verification
   section should name the remaining crash points precisely
   (`catch_up_before_commit`, `db_committed_before_head_fsync` — the probes
   exist, catch_up.rs:121-123). Coverage that survives is genuinely
   equivalent at unit grain: the derived-state tests flip card state through
   assertions/dispositions (catch_up_derived_state.rs:52-116), and per-scope
   hashing moves those digests, so the round-4 defect class stays covered.

8. **The precision trade is measured and acceptable — record the numbers.**
   The ADR says "No measurement shows that cost as a problem"; that was
   untested. I measured (probe with the crate's own `byte_domain`,
   `domain_bytes`, `canonical_records`, release build, this machine, synthetic
   uniform tree, one scope + global edges):

   | Canonical bytes | hash sweep, per-family (current) | hash sweep, per-unit (ADR) | re-derive 1 family | re-derive all families in scope (ADR) |
   |---|---|---|---|---|
   | 0.6 MB (50 rec/family) | 3.5 ms | 2.9 ms | 0.4 ms | 5.7 ms |
   | 6.2 MB (500 rec/family) | 35.6 ms | 28.1 ms | 2.2 ms | 67.5 ms |
   | 62 MB (5,000 rec/family) | 366.5 ms | 280.8 ms | 32.1 ms | 678.9 ms |

   Reading: hashing dominates and is ~the same or *cheaper* per unit (the
   per-family sweep hashes 11% more bytes because cards/dispositions
   double-count their inputs). The coarsening multiplies the re-derivation
   share: a one-family change costs ~3x at every scale (hash + 1 family vs
   hash + whole scope). At 100x the largest measured tree (~6 GB, implausible
   for this tool), that is ~30 s vs ~96 s per changed pass. Defensible; put
   the table in the ADR so W5 has a baseline.

## Question 1 — is the diagnosis true at f121b55?

**(a) Does the journal change any answer? Essentially no — and it can only add
work, never save it.** Every unit is hashed on every pass regardless of the
journal (`hash_unit` at catch_up.rs:223-227 runs inside `sweep_unit`, which
runs for all 19 × scopes; the tests pin `families_hashed == 19`). The skip
decision is the digest comparison *and* the drained set (catch_up.rs:177-180:
`unchanged` requires `!self.drained.contains(&key)`), so the journal never
enables a skip — it only *forces* re-derivation of units whose bytes did not
change. Forced re-derivation of a digest-matching unit re-runs the same
deterministic loaders over the same snapshot bytes and rewrites identical rows
(catch_up.rs:182-195), so rows, content digests, and the revision digest are
unchanged. The journal does alter: the *serial value* (writer events allocate
from the shared space, journal.rs:147-176, so the pass serial is head, not
stored+1 — catch_up.rs:61-62, 97, 119), the report counters
(`events_drained`, catch_up.rs:100), and post-commit pass success
(`normalize_head`/`prune_up_to` run after `tx.commit()`; their I/O errors fail
the pass report even though the data committed, catch_up.rs:124-125 — the
mirror of the R1-F4 finding, now degraded-to-hint at the writer side,
publication.rs:477-482). One narrow semantic footnote: a drained unit with
unchanged bytes gets its rows *rebuilt*, which incidentally repairs rows left
by older derivation logic; the digest sweep alone would keep them. That is a
logic-drift repair neither design guarantees (see Q5), and it is not a reason
to keep a journal.

**(b) Is the byte domain a hand-maintained mirror with no type-level tie?**
True, with one nuance the ADR understates in its own favor. Two families'
domains *are* the readers' own discovery functions (`message_shard_paths`,
readers.rs:320-345; `edge_shard_paths`, readers.rs:377-395 — called from both
readers.rs and projection_families.rs:148,152), so those cannot drift. The
rest is hand-assembled, and the cross-family derived inputs are exactly where
it broke twice: `ProjectionFamily::ProposalCards` now enumerates assertions,
dispositions, and legacy promotion decisions (projection_families.rs:157-166),
matching what `project_proposal_cards` actually reads
(state_store.rs:292-320) through `effective_proposal_state`
(provenance-core lifecycle.rs:127-146). The "completeness gate" is not a code
mechanism — it is a set of behavioral tests, and the broadest one
(`every_family_invalidation_reaches_the_projection`,
catch_up_domain_coverage.rs:20-27) *cannot* catch this class: its
`change_one_record(AssertionRecords)` renames the assertion id
(projection_digest_sensitivity.rs:167-170), which leaves
`effective_proposal_state`'s `any(proposal_id ==)` predicate true and the card
rows identical, so parity holds even if the cards domain dropped assertions
again. Only the hand-written derived-state tests catch it, and each new
derived field needs a new hand-written test. The ADR's "harder, not
impossible" is exactly right, and if anything generous.

**(c) Is "a third of review findings" fair? Yes — conservative.** Counting
distinct findings in the PR 174 body: rounds 1+2 list 10 items plus 3 deferred
(R2-E i-iii) plus 1 declared open boundary = 14. Journal/serial-attributable:
R1-F4 (journal I/O poisons a committed write), R1-F6+R2-A (serial reuse across
both named scenarios, plus the head record's placement), R2-E(i) (tail rescan),
R2-E(iii) (journal re-exports/guard typing), and the known boundary
(events+head lost together) = 5-6 of 14 ≈ 36-43%, depending on whether the two
serial-reuse scenarios are counted separately and whether prune-before-commit
(surface in the round-3 table as M7) is included. Adding the round-3 mutation
survivors (M2 emission mapping, M6 reservation, M6b post-commit normalize, M7
prune order — 4 of 8) and round 4 (1 non-journal finding) gives ~10 of 23 ≈
43%. "A third" is fair and slightly understated. Worth adding to the ADR: the
byte-domain declaration generated the *other* large share, and both of its
misses were rejection-grade (stale stamped answers), which strengthens, not
weakens, the case for removing it too.

## Question 2 — is the decision sound?

**(a) Per-scope hashing: no family reads across scopes today.** Traced every
`list_*` in state_store.rs:148-352, the loader bindings in family_rows.rs,
and the direct readers: sources/domains/requirements/boundaries/topics/
questions/resolutions/rules/verifications/implementations/reviews/threads read
exactly their own shard; messages read `threads/YYYY-MM.jsonl` discovered in
the scope's threads dir; the five overlay families read their shard plus
`ideation/landings.jsonl` (scope-local); cards add assertions/dispositions/
promotions (all scope-local, and read *unconditionally*, so this is not even
data-dependent); edges read the global dir. Global non-scope files that feed
stored rows: edge shards only. The manifest gates validity
(`validate_ideation_scope` reads it, ideation_batches.rs:126-131) but no row
value derives from it, and the validator reruns every pass, so a manifest-only
change either refuses the pass or changes nothing — sound. `state/dictionary.json`
feeds no stored row (it feeds STE resolution outside the projection) but
belongs in the global unit's file set (amendment 2). The one unit-boundary
subtlety is edges (findings 1), not cross-scope reads.

**(b) "Delete a scope's rows across all 19 tables and reload the scope" is
incorrect as written** — see finding 1 and amendment 1. With the mapping
amended (scope unit → 18 scoped families; global unit → edges), the rule is
correct for every family: all 19 tables key rows by `scope_id` (migration 002
for edges, 018 for the three integration tables), `delete_rows` already
handles both shapes (family_rows.rs:13-35), and no family has global rows
besides edges. I probed the suspicious case — scope departs while its edges
remain in the global shard — and rebuild and catch-up agree (both keep the
rows, because the bytes still hold them); when the shard bytes change, the
global unit digest moves and edges reload wholesale. Consistent.

**(c) The stored+1 serial is safe for W3/W5.** Nothing in the plan's W3 or W5
consumes journal sequences: the stamp table is `{serial, digest, policy,
attested, live}`; `refuse_stale` needs monotonicity within one instance and
names a gap; trace cursors are depth+rank+id watermarks, not serials; the
differential gate compares serialized answers, not serials. The shared space
existed only because the journal needed allocation/drain bounds; with the
journal gone, stored+1 under the guard has one writer in one transaction, and
a crash before commit reuses nothing that was ever observed. The one genuine
dependency is instance identity on total cache loss (finding 5 / OQ7), which
the plan already owns and the code already implements
(`projection_instance`, stamp.rs:23-26; test
`total_cache_loss_restarts_the_serial_inside_a_fresh_instance`).

**(d) Losing per-family digest rows: "first pass rebuilds once" is the whole
user-visible cost, but not the whole engineering cost.** The rebuild is
automatic: migration 020 makes `migrations_applied` non-empty and catch-up
routes to rebuild (catch_up.rs:56-58; pinned by
`a_schema_move_routes_catch_up_to_a_full_rebuild`). The rest: the revision
digest recipe must be redefined or re-derived (finding 3 / amendment 3), the
W1 tests that pin per-family baseline rows (`materialize_guard_behavior.rs:101`,
`projection_stamp_behavior.rs:218`) need rewriting at unit grain, and the
diagnostic columns (size/mtime/record_count per family) disappear — nothing
reads them today (grep: only stamp.rs and catch_up.rs touch the table), so
that loss is real but unpriced. The ADR's sentence "as W1 defines it" is the
part that cannot survive contact with "one digest row per unit."

**(e) Precision at scale: defensible, now with numbers** — see finding 8.
Per-unit hashing costs the same or less hashing than per-family (no
double-counting); the entire coarsening cost is re-derivation of unaffected
families in a changed scope, measured at ~3x the single-family delta and
sub-second at 62 MB of canonical state (≈90k records). The realistic failure
mode is skew (one huge messages family, one tiny rule edit), which raises the
ratio further; W5's measurement gate covers it, and the equivalence tests keep
finer grain available as a later optimization.

## Question 3 — what the ADR misses

Mostly captured in the amendments and findings; consolidated:

- The unit→family replacement rule (finding 1) and the digest-recipe
  contradiction (finding 3) — the two places the Decision text is wrong or
  self-contradictory.
- The framing/residue/unit-set specification (findings 2 and 6): relative-path
  framing, temp-file filtering, `state/dictionary.json`.
- The scope-locality invariant and its guard (finding 4) — a year from now,
  a reader adding a cross-scope join to a loader breaks this design silently;
  the ADR records the removal of the old guard but not the new rule.
- The OQ7 dependency (finding 5).
- Knowledge preservation: the derivation audit table and the file→family
  facts (landings feeds five families; month shards live in the threads dir;
  promotions are frozen shipped-v1) survive only in PR 174's body
  (finding 7b). Copy them out before the branch squashes.
- docs/cache.md updates beyond what the plan already schedules: the journal
  paragraphs (lines 19+) and the digest property sentence (lines 9-12) both
  change; the ADR's verification section enumerates tests but no doc edits.
- `CatchUpReport` shape: `events_drained` disappears; `serial` semantics
  become stored+1; W3 consumers and the tests that assert on reports should be
  named in the ADR's verification list.
- A small but honest admission the ADR could make: the per-family sweep hashes
  *more* bytes than a per-scope sweep (derived inputs are hashed once per
  consuming family), so removing the declaration is also a small hashing win —
  measured ~11% on synthetic state (finding 8). The ADR argues cost only on
  the re-derivation side.

## Question 4 — alternatives, honestly evaluated

1. **Instrument reads during rebuild to derive domains.** More viable than the
   ADR implies: the readers have two choke points (`with_state_path_access`,
   publication.rs:441-456, and `read_jsonl`/`read_ideation_landings`,
   readers.rs:185-204) where a task-local "current family" context could
   record every path read per loaded family; the cards case reads
   assertions/dispositions *unconditionally* (state_store.rs:305-306), so the
   round-4 miss would have been derived, not missed; and instrumentation errs
   toward *over*-inclusion (validator reads get attributed too), which is the
   safe direction — the mirror failed by under-inclusion. Real costs: a
   persistence table, a refresh policy (reader-code changes need a
   re-instrumenting rebuild, the same logic-drift gap as Q5), and plumbing.
   Verdict: the ADR's rejection is right *for now* — but its stated reason
   ("a per-scope hash makes the question moot") conflates precision (moot at
   current sizes) with correctness (the instrumented read-set is also the
   cheapest way to guard scope locality, amendment 4). Fold the one-pass
   assertion in even if the full mechanism never lands.
2. **Hash per scope, re-derive per family from per-family digests off the same
   snapshot.** Unsound as stated: digesting each family's *directory* cannot
   see derived inputs (the landings/promotions/assertions problem returns at
   finer grain), and digesting derived records requires parsing everything,
   which forfeits the skip. Correctly rejected; I could not repair it without
   reintroducing a mapping.
3. **Keep the journal as an optional accelerator with no serial coupling.** If
   it names only scopes (dropping the family mapping), the drain window
   becomes "these scopes changed" — which the digest comparison already
   returns, at the same hash cost, with less durable state and no head record.
   The ADR's rejection holds verbatim; the journal's only nonzero residue is
   observability (an audit trail of writer events), which the plan never
   promised and no consumer uses.
4. **Worth adding: a hybrid the ADR skips.** Decide freshness at unit grain
   (per this ADR) but keep maintaining per-unit *content* digests carried
   forward for unchanged units, so the revision digest stays W1's
   content-derived, split-invariant recipe. This is amendment 3's preferred
   resolution; it costs one extra column and keeps the W1 stamp contract and
   tests meaningful.

## Question 5 — adversarial review of what survives the ADR at f121b55

Defects or hazards in guard/snapshot/validator/delete-then-insert/stamp/
departed-units/equivalence that the simplification keeps:

- **The guard's starvation constraint is a deferred defect, documented not
  fixed.** guard.rs:11-19: while an async guard is held across awaits, a
  synchronous `with_repository_publication` entered on a runtime worker blocks
  that worker in flock with no awareness of the guard; N such callers starve
  the runtime and the guard-holder's continuation can never run. The comment
  says "today's one-command CLI cannot reach this" — W3's
  snapshot-consistent-read design will put a reader-entry helper inside a
  guard scope in a serving process. The ADR inherits this constraint without
  mentioning it. Not fixable in W2; must not be forgotten at W3.
- **Reader calls inside the guard scope take a second, real flock — on the
  snapshot's lock path.** Every `list_*`/`manifest()` call during rebuild and
  catch-up runs on the snapshot layout (catch_up.rs:68-69,
  materialize.rs:47-49), so `with_repository_publication`
  (state_store.rs:96-103 → publication.rs:53-71) misses the thread-local
  `HELD_LOCKS` (the async guard never registers there) and acquires a fresh
  flock — which does not deadlock only because the snapshot's lock path
  differs from the real one (publication.rs:410-420 builds a fresh root). Two
  accidental facts (different path; uncontended tempdir) make a plan-invariant
  ("no held scope opens a second lock file description") hold by luck at a
  rate of ~40 acquisitions per pass. Harmless today, fragile under refactoring;
  worth one sentence in guard.rs.
- **`remove_departed_units` drops digest rows for unknown family names but
  keeps their rows** (catch_up.rs:244-254: `find` returns None → row deletion
  skipped). Unreachable until a future migration renames a family; a schema
  move routes to rebuild first. Leave or tighten, but note it.
- **Logic drift has no trigger.** Rows are refreshed only when bytes move; a
  derivation fix (e.g., in `effective_proposal_state`) with no shard edit and
  no migration leaves stale rows stamped fresh — in *both* designs, and the
  journal only papered over it when a phantom event happened to name the
  family. The plan's trigger 5 covers schema versions only. A derivation
  version (bumped with the code, stored beside the instance, forcing rebuild
  on change) is a two-column fix the plan never schedules; propose it for W3's
  stamp freeze rather than blaming either W2 design.
- Checked, clean: guard acquisition on the blocking pool with owned-file
  release (guard.rs:77-92, Drop at 56-60); read-only bypass symmetry between
  both entries (guard.rs:79-81, publication.rs:58-60); the forged-guard
  trybuild pin; snapshot taken under the guard hashes baselines from the
  snapshot while metadata stays live-and-diagnostic (stamp.rs:92-117,
  catch_up.rs:197); validator runs on every pass on the same snapshot the rows
  come from and refusal commits nothing (catch_up.rs:71-75; pinned by
  `catch_up_refuses_state_the_aggregate_validator_refuses`); delete-then-insert
  dispatches through the same loaders as rebuild for all 19 families
  (family_rows.rs:37-89) with equivalence pinned by a full 19-table,
  all-column, `quote()`-normalized dump plus digest equality
  (catch_up_behavior.rs:8-53); reservation-before-commit ordering closes the
  serial-reuse window and is mutation-pinned (journal.rs:94-99; M6); departed
  units lose rows and baselines for all scoped families; same-size
  mtime-restored edits are hashed and caught with the journal absent and
  inside a drained window; `families_hashed` is derived at the only hash site
  so a skipping sweep cannot fake the counter (catch_up.rs:223-227); the
  `INSERT OR IGNORE` instance row cannot duplicate under its CHECK constraint
  (migration 018); pools are closed before return on every path
  (materialize.rs:23-26, catch_up.rs:126, 276). Full store suite: 289 passed,
  0 failed on this checkout.
