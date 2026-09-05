# Adversarial review: W3 operations re-back, plan revision 2 (7d82d4c)

Reviewed against origin/main at 577ab96. The four resolutions and
`boundary_query_answers_do_not_page` on 1wh-w3-rulings were read, not relitigated.
Nearly every file:line the plan cites is accurate at 577ab96; the citation audit
found no invented paths. The findings below are about behavior the plan gets
wrong or leaves unspecified.

**Verdict: ACCEPT WITH AMENDMENTS.**

## Findings

1. **Blocker — G.** `scan_path_bounded` is specified as cutting "after `max_files`
   regular files in the walker's order" (plan G; `walker.rs:78-83,85-110`). The
   walker's order does not exist as a deterministic fact: `walkdir::WalkDir::new`
   (walker.rs:87) yields readdir order, and the only ordering —
   `scans.sort_by(file_path)` at walker.rs:108 — runs after the whole walk
   completes, which a bounded walk never reaches. A cut therefore keeps a
   filesystem-dependent set of files: the same repository at the same commit
   reports different rules from `impact` and `resolve_symbol` between runs and
   machines. `scan_cut: true` would attest "a lower bound" that moves run to run,
   and the J test "sub-limit runs answer the same union as `scan_path`" only
   holds by luck on the cut path. Amendment: define the cut over a deterministic
   order — sort entries (`sort_by_file_name`) during the walk, or collect paths,
   sort by full path, then scan the kept prefix — and add a test that two cut
   runs over one repository return identical rule sets.

2. **Major — C, B.2.** The `SqlFront` hop has no retired handling. Today's walks
   build `RecordFront` from `records::load(store, scope, request.include_retired)`
   (walk.rs:34,122,154; records.rs:66), so a retired record contributes no
   outgoing rows and is never an endpoint. The `relations` table has no retired
   marker (021_relations_table.sql:9-19), and the planned hop query
   (`WHERE scope_id = ? AND owner_type = ? AND owner_id IN (...)`) has no filter;
   the executor filters the plan keeps (walk.rs:86-103) filter direction and
   relation name only. As specified, every flip of `neighbors` and `trace`
   resurrects retired records and edges that today's answers exclude. Amendment:
   state the hop-time retired filter — join the kind tables' `retired` column (or
   carry `retired` on relation rows) honoring `include_retired` on both the owner
   and target side — and extend the front equivalence property to a scope that
   contains retired records with relations.

3. **Major — C.** The front equivalence claim ("`related_nodes` over
   `RecordFront` equals `related_nodes` over `SqlFront`") is false where one
   record names the same target twice. `RecordFront::outgoing/incoming` yield one
   entry per field entry (front.rs:250-291), while the table dedups:
   `INSERT OR IGNORE` against `PRIMARY KEY (scope_id, owner_type, owner_id,
   relation, target_id)` (relation_rows.rs:48-58), a case the tree documents —
   "Two citations of one source with different clauses are one row"
   (relation_rows.rs:48-50). Two `source_refs` entries citing one source with
   different clauses give two `cites` rows over `RecordFront` and one over the
   table, so flipped `neighbors` loses a row today's oracle returns (or the
   equivalence test fails on the first corpus that has such a record).
   Amendment: pick one semantics — dedup the fetched hop rows to match the table
   — state it in C, and pin it with a fixture that cites one source twice.

4. **Major — D, J.** The golden drift alarm digests answers "over the seeded
   store and the repository's own state", keyed by `READ_DERIVATION`. The
   repository's own canonical state moves on main independently of reader logic —
   the repo dogfoods, and `res.jsonl` itself changed this week on 1wh-w3-rulings —
   so every canonical edit fails the golden and forces a `READ_DERIVATION` bump
   that means nothing, which in turn hollows out the constant the drift alarm
   depends on. The supporting claim "(the in-tree tests already export it)" is
   also false: no test on main reads the committed `.provenance/state`; all
   build temp fixtures (checked across crates/provenance-cli/tests and
   provenance-store). Amendment: key the golden to the fixed fixtures only
   (seeded store, CLI fixtures, gap fixtures); if the repository's own state
   stays in the corpus, compare it structurally, not by digest, or keep it out
   of the committed golden file.

5. **Major — D.** Section D contradicts itself on the stamp's `attested` and
   `live` vocabularies. The definition says `attested` "names the projection
   tables ... in the family words of projection_families.rs:69-90 plus
   `relations`", but the per-operation rows list response field names (`found`,
   `node`, `limit`, `has_more`, `id`), words that are neither family words nor
   tables (`implementations`, `verifications` — the family words are
   `implementation_bindings`, `verification_bindings`), and a `live` word
   (`latest_verification_run`) outside the closed four-word list (`canonical`,
   `scanned_sites`, `verification_runs`, `diff`). This is the wire contract of
   `res_stamp_names_projection_instance`; two implementers will produce two
   different arrays. Amendment: rewrite the table so each cell holds the literal
   wire strings, family words plus `relations` for `attested`, the closed
   four-word list for `live`, and nothing else.

6. **Major — H.** Ideation dangling-target validation is new write-path scope:
   refusals in `validate_graph_scope` (graph_validation.rs:23), a new gap kind
   pass, `IdeationTargetType` gaining `Boundary` (a canonical-format change,
   ideation.rs:14-45), and index extension to eight kinds. Nothing in the bead
   (`provenance-1wh.2`, serve eight queries from the projection) or in the four
   resolutions or in section I's PR 180 leftovers assigns this to W3, and it
   lands in PR 3 beside seven behavior flips. Amendment: split H into its own
   bead, or cite the owner decision that assigned it; if it stays, split it into
   a PR 4 so PR 3 stays reviewable as flips plus leftovers.

7. **Minor — C.** `RelationSource::outgoing/incoming` return
   `Vec<(&'static str, RelationEndpoint)>` (front.rs:57-65); SQLite returns the
   relation name as `String`. The plan says "`HopRows` implements
   `RelationSource` over the fetched rows" but never says how the names become
   `&'static str`, and "run unchanged" rules out editing the trait. An
   implementer will either leak strings or change core. Amendment: state that
   `HopRows` interns each fetched name through
   `declaration_for(owner_kind, name).name` (the vocabulary is closed,
   relations.rs:73-83) and refuses a fetched name with no declaration.

8. **Minor — C.** Trace keeps the whole per-depth `next` as the next frontier
   (walk.rs:178-181) and impact keeps every reached node (impact.rs:53,59); the
   limit cuts only what is recorded (walk.rs:183-187). The hop's
   `owner_id IN (...)`/`target_id IN (...)` is therefore unbounded per depth;
   beyond SQLite's host-parameter limit (32766) the hop aborts mid-answer where
   today's in-memory walk succeeds. Amendment: chunk the IN lists in
   `SqlFront::hop` and say so in C.

9. **Minor — E.** `annotate_only` skips catch-up entirely, and `open_cache`
   (cache.rs:34-39) runs no migrations. On a fresh or un-migrated database,
   `annotate_only` reads stamp rows and kind tables that do not exist; "the
   stamp reports the stored serial" is undefined when there is no serial.
   Amendment: under `annotate_only`, still run `migrations::run_migrations` (or
   fail with a typed error), and define the answer when no revision row exists.

10. **Minor — J, K.** The oracle copies will not compile verbatim. The copies are
    named `{records, walk, impact, evidence, symbols}.rs` under
    `tests/oracle/`, but `impact.rs` does `use super::{bindings::Bindings, walk}`
    and `evidence.rs` does `use super::{bindings::Bindings, stale}`
    (impact.rs:11, evidence.rs:8); `bindings.rs` and `stale.rs` are not in the
    copy set, and `super::` inside `tests/oracle/` resolves to the oracle module,
    not the real one. Amendment: extend the copy set to the module closure
    (`bindings.rs`, `stale.rs`, and `sites.rs` for `impact`) or rewrite the
    copies' imports to `crate::` paths, and say which in J.

11. **Minor — K, J.** After PR 3 step 9 deletes the oracle, `records::load` and
    `records::find` (records.rs:12-83) lose their last callers, and the plan
    keeps the loader alive until W5. `pub(super)` items with no callers trigger
    `dead_code`, and the plan's own green gate is `clippy -D warnings`.
    Amendment: name the keep-alive — keep the oracle's `records` usage under
    `#[cfg(test)]`, or move `load`/`find` behind a test-gated module, or an
    explicit scoped allow, until W5 deletes them.

12. **Minor — C.** The round-trip gate fills every field, so the `None` path is
    never exercised, and that is exactly where the decode spec has a hole:
    `Option<DeclarationAddress>` and `Boundary::source_ref` (both
    `#[column(json)]`) encode `None` as `ColumnValue::Null`, and "parse their
    text" has no Null arm; `skip_serializing_if` fields re-serialize differently
    if a decode produces a non-default value. Amendment: state the Null decode
    (Null maps to JSON null, `from_value` gives `None`) and add one all-default
    fixture per type beside the all-filled one.

13. **Minor — K.** The file-cap ledger misses `walker.rs`, at 417 lines the
    closest Rust file to the cap that the plan grows (`scan_path_bounded`).
    Projected growth keeps it near 440, under the limit, but K should name it
    the way it names `protocol.ts` at 399. Everything else checked:
    artifacts.rs 280, front.rs 346 (untouched), walk.rs 215, records.rs 155,
    evidence.rs 94, catch_up.rs 273, response.rs 116, query.rs 175,
    macros/relations.rs 320, protocol.ts 399 — all stay under 500 with the
    planned edits.

14. **Minor — H, J.** Two wording defects that will cost an implementer time:
    "GraphRecords ... gains as three lists" names two record kinds
    (contributions, synthesis packets) — say which three lists; and "the four
    writer sites" cites ideation_writers.rs:32,151, which are destructuring
    lines of the two create functions, not write sites — the write sites are the
    `target` fields at :53 and :166 (plus the batch landing paths the
    catch-up hook covers).

## Binding decisions: verified, not violated

- `res_query_answers_stop_at_the_limit`: no cursors or tokens anywhere; the
  200 cap and `has_more` untouched (protocol.rs:31,84-88; plan F); one
  `has_more` per evidence list beside the OR (evidence.rs:60-63,85); the scan
  limit is a `ReadPolicy` field with a configured default, never a request
  field (E, G; query.rs requests deny unknown fields and gain nothing).
  No visit budget; depth ten via `TRACE_MAX_DEPTH` is the only walk bound.
- `res_impact_follows_declared_flow`: multi-step over declared flows only,
  `flow_neighbors` never follows `flow = none` (front.rs:147-172, :168),
  matching the resolution's rejected one-step reading; the plan quotes the
  resolution's reach rule exactly.
- `res_projection_tables_mirror_record_types`: one column per field through a
  derive, list fields as JSON text, a round-trip gate, `get` and `search` read
  the tables (flips 1-2). `search_text` and `retired` are additive to the
  mirror, not violations. The field census is complete: across the eleven
  types, the only non-Vec struct-typed fields are `declaration_address` and
  `source_ref`, both marked `#[column(json)]`; enums are unit-only serde
  words; timestamps are `i64`; `confidence` is `Option<f64>`; `SchemaVersion`
  is a transparent u32 — no field is incapable of round-trip (finding 12 covers
  the None-path hole; f64 through REAL is IEEE-lossless and pinned by the
  planned non-round value test).
- `res_stamp_names_projection_instance`: all seven stamp fields on the wire,
  `instance_id` from `projection_instance` (018:45-48, stamp.rs:24-27), serials
  within one instance (docs/cache.md:15-17) — subject to finding 5's vocabulary
  fix.
- `boundary_query_answers_do_not_page`: honored verbatim.
- `res_relations_are_fields_or_action_records` and
  `res_catch_up_hashes_scopes_no_journal` (main): served reads go over the
  derived `relations` table; no journal anywhere; catch-up hashes every unit
  (catch_up.rs:86-97). The relation vocabulary stays the closed thirteen names
  (relations.rs:73-83, docs/cli.md:138-142).
- Migration 022 drop-and-recreate: safe as specified. Catch-up rebuilds after
  any migration (catch_up.rs:55-57); the rebuild clears and rewrites all
  families including `relations`; `projection_instance` is not dropped, so the
  instance id and the "serials within one instance" rule survive; unit, family,
  and revision digests derive from canonical bytes and do not move. The PRAGMA
  drift test does catch a struct field added without a column (COLUMNS is
  derived from fields; table_info is not), and the insert built from COLUMNS
  fails at runtime too.
- Reader entry: no deadlock found beyond the one the plan states. The list
  readers the eight ops use are plain `read_jsonl` (state_store.rs:151-206);
  the only `with_repository_publication` site reachable from a query is
  `stale::disturbed` via `graph_evidence` (stale.rs:36, health.rs:65), which
  the plan routes to the snapshot layout per guard.rs:14-15;
  `catch_up_with_guard` takes the guard by reference (catch_up.rs:37).
  `block_in_place` is rightly rejected. The cost claims are accurate:
  snapshot on every pass (catch_up.rs:59), every unit hashed (catch_up.rs:86-97),
  reads serialize.

Every other file:line spot-checked in sections A through L matched the tree,
including the small ones (stamp.rs:24-27,56-67; sites.rs:26-33,51-62;
relation_names count; file sizes in K; the "old citations not verifiable" list,
which is honest).