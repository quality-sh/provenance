# W5 release gate: implementation plan (revision 1)

Bead `provenance-1wh.3`. Read at `03a935c` (W3 fully merged: PRs 192, 193, 194, 195, 196). Every `path:line` below
was counted there. This document is the W5 charter of the approved W3 plan, section M, worked out against the
tree as it stands. Section B is the new bead text. Section L holds the questions for Ben.

Settled and not reopened: no pagination and no cursors anywhere (`res_query_answers_stop_at_the_limit`,
`boundary_query_answers_do_not_page`); the scan limit is a configured default, not a request field; the timing
comparison is a hand-run report and never a CI gate; core stays pure; freshness annotates and never refuses,
except the explicit `refuse_stale` policy; code files stay under 500 lines; every implementation PR lands its Rule
records and bindings.

## A. Purpose and the release rule

W3 is merged but not released. W3 and W5 ship together in one release, so this bead gates the release. The rule:
no version tag is pushed until every PR in section K is merged and the timing report of section K.4 has run on
the release commit. The bead is P2 while `provenance-1wh.2` was P1; raising it is Ben's call and is not a question
in L.

W5 does four kinds of work. It removes cost from the read path (the tree copy and the every-pass validators). It
fills the seams W3 left (the `read.*` settings, `refuse_stale`). It deletes what W3 kept only for comparison
(`records::load`, the `records` baseline, the canonical side of the comparison tests). It writes down what the
next consumer needs (the MCP handoff criteria).

## B. New bead text for `provenance-1wh.3`

Title: `W5: release gate for the served read path`

Description:

```
Goal. Close the release gate the W3 plan (section M) left open, so W3 and W5 ship in one release.

Where the code stands at 03a935c. Every catch_up read copies the state tree to a temporary directory
(publication.rs:406-435, catch_up.rs:68), runs both validators over every scope (catch_up.rs:72-75), and
then hashes every unit. The scan stops at DEFAULT_SCAN_LIMIT = 2000 (read_policy.rs:11), a number taken from
one debug-build measurement. No settings file exists; queries::served uses ReadPolicy::default()
(queries.rs:36). refuse_stale is reserved and refuses as unimplemented (freshness.rs:54). records::load
survives under cfg(test) (records.rs:20-82) for the records baseline and the comparison tests. The timing
comparison is an ignored test (comparison.rs:198-208). A read-only checkout answers through an immutable open
(freshness.rs:79-96), undocumented in docs/cache.md.

Items.
1. Catch-up hashes each unit in place under the publication guard. No tree copy. Validators run for the
   units that moved (every scope when the global unit moved). A changed scope re-derives only the families
   whose content digest moved, as today. The digest a pass stores is the digest of the bytes it parsed.
2. The scan default is set from a measurement over four repositories in release and debug builds; the
   measurement procedure is documented beside the constant.
3. .provenance/settings.json carries read.freshness_policy and read.scan_limit. An invalid value is a typed
   refusal, never a silent default. The eight sdk query commands take --freshness, which wins over the file.
4. records::load, records::find, the records and stale baselines, and the canonical side of the comparison
   tests are deleted. Their cases live in the pinned answers file and the served tests.
5. refuse_stale hashes in place without writing, answers at the stored serial when every unit matches, and
   otherwise refuses with a typed error naming the serial and each moved unit with its stored and live digests.
6. The timing comparison runs by hand in release on the release commit; its rows go in the release notes.
7. catch_up_failed stays the policy word for a failed freshness step.
8. A read-only checkout answers as an immutable image under every policy, and docs/cache.md says so.
9. The MCP handoff criteria of the plan's section J are met and recorded on this bead.

Rulings this text rests on: res_query_answers_stop_at_the_limit, res_impact_follows_declared_flow,
res_projection_tables_mirror_record_types, res_stamp_names_projection_instance,
res_catch_up_hashes_scopes_no_journal, boundary_query_answers_do_not_page. Plan:
docs/plans/2026-09-06-w5-release-gate.md on branch 1wh-w5-plan.
```

Acceptance criteria:

```
- An unchanged catch-up pass opens no temporary directory and parses no shard other than the manifest; a
  test records every canonical path read and asserts it.
- An invalid scope that did not change does not refuse a read; a changed invalid scope still refuses and
  commits nothing; a manifest change validates every scope.
- The pinned answers file is byte-identical before and after each W5 PR, and READ_DERIVATION stays 1.
- .provenance/settings.json is read on every query; a bad key, a bad word, or a non-positive scan_limit
  refuses with the file path, the key, and the allowed values in the message.
- --freshness on the eight sdk commands overrides the file; the stamp's policy word is the policy that ran.
- refuse_stale answers with policy "refuse_stale" when current and refuses with the moved units when not;
  it never writes a revision row.
- records::load, records::find, tests/baseline/, and the baseline column of the comparison tests are gone;
  the test count after each PR equals the count before it plus the tests the PR names as added minus the
  tests it names as deleted.
- docs/release.md carries the timing report command and a checklist line; docs/cache.md documents the
  read-only checkout behaviour; docs/cli.md documents the settings file and the flag.
- Each PR lands its Rule records and #[rule]/#[verifies] bindings; provenance coverage scan
  --validate-rules is clean.
- The MCP handoff criteria (section J) are checked off on this bead with the commit that met each one.
```

## C. In-place hashing (item 1)

### C.1 What the read pays today

Under `catch_up` every read runs `catch_up_with_guard` (`catch_up.rs:45-130`). The pass copies the whole state
tree into a temporary directory (`snapshot_state_under_guard`, `guard.rs:94-99`, through `snapshot_state_unlocked`
and `copy_tree`, `publication.rs:406-435`), builds a `StateStore` over the copy (`catch_up.rs:69`), runs both
validators over every scope (`catch_up.rs:72-75`; `validate_graph_scope` parses seven kinds,
`graph_validation.rs:23-45`; `validate_ideation_scope` parses five ideation shards under its own lock section,
`ideation_batches.rs:127-132`), and only then hashes each unit (`catch_up.rs:98-106`, `units.rs:45-66`). A rebuild
does the same copy (`materialize.rs:50`) and hashes the copy after loading (`materialize/stamp.rs:44-51`).

The measurements below were taken on this repository at `03a935c` with the release CLI built from the tree,
medians of five to seven runs after one warm-up. The synthetic trees hold the default scope's graph and thread
shards copied under N scopes (the ideation shards are pinned to one scope by the lifecycle validator and were
left out of the copies).

| Step, this repository (17 files, 1.0 MB of state) | ms |
|---|---|
| `provenance --version` (process floor) | 3.3 |
| `provenance sdk get` under `catch_up`, end to end | 28.4 |
| in-process unchanged catch-up pass (`timing_comparison_rows`, `repository_state`) | 19.7 |
| in-process rebuild (`rebuild_ms`, same store) | 121.1 |
| `provenance materialize`, end to end | 738 |
| copy of the state tree (python `shutil.copytree`) | 2.6 |
| sha256 of every state file in place (python) | 2.1 |

| Synthetic tree | files | MB | `sdk get` under `catch_up` (ms) | `materialize` (ms) | tree copy (ms) | hash in place (ms) |
|---|---|---|---|---|---|---|
| 20 scopes | 207 | 8.2 | 153.6 | 1070 | 26.0 | 17.8 |
| 100 scopes | 1007 | 38.4 | 680.9 | 4856 | 138.3 | 94.2 |

The copy is 15 to 20 percent of an unchanged pass and the hash 12 to 14 percent. The remaining two thirds is the
validators parsing every shard of every scope, plus the manifest read, the guard, and the pool open. The python
sha256 figures are a ceiling; `canonical_digest::digest` uses `sha2` and hashes faster.

### C.2 The pass after item 1

1. `catch_up_with_guard` reads `layout.state_dir()` directly. `snapshot_state_under_guard`, `StateSnapshot`,
   `snapshot_state`, `snapshot_state_unlocked`, and `copy_tree` are deleted (`guard.rs:85-99`,
   `publication.rs:17-26,401-435`). The compile-fail case `tests/compile_fail/forged_guard.rs` moves onto the
   constructor in step 2, which is the same proof.
2. `StateStore::under_guard(&PublicationGuard, layout) -> StateStore` is the store a pass reads through. The
   guard is the proof that the publication lock is held, so `with_repository_publication` on that store runs the
   operation directly (`publication.rs:456-461`). Today `manifest()` (`state_store.rs:99-107`), the five ideation
   list readers (`state_store.rs:250-352`), and `validate_ideation_scope` (`ideation_batches.rs:127-132`) each take
   the lock; on the real layout under a held guard that is a second `flock` on the same file from a second file
   description, which blocks forever. The private field of `PublicationGuard` (`guard.rs:60-62`) means only a real
   guard can build such a store. Rejected: `with_read_only_validation` (`read_only.rs:23-31`), because it is
   thread-local and the CLI runs the read on a multi-thread runtime (`main.rs:16-21`), where a task moves between
   worker threads across await points.
3. Per unit, in the sorted order of `units_for` (`units.rs:36-42`): hash in place. An unchanged unit ends here, as
   today (`catch_up.rs:100-102`). No shard of an unchanged scope is parsed.
4. A changed scope unit: run both validators for that scope, parse its nineteen families, rewrite the families
   whose content digest moved, reload the scope's `relations` rows when an owner family moved
   (`rederive_scope`, `catch_up.rs:218-244`, unchanged), then hash the unit again. When the second digest equals
   the first, that digest is stored. When it differs, the bytes moved between hash and parse, which only an
   unlocked editor can do (every writer takes the lock, `docs/cache.md:71-75`); the pass repeats the unit, at most
   three times, then refuses with `canonical state changed during catch-up under <scope>`. A refusal commits
   nothing, as today.
5. A changed global unit: store its digest, and run the validators for every scope, because
   `manifest.disposition_actor_ids` feeds the ideation validator (`ideation_batches.rs:129-130`). No family derives
   from the global unit (`catch_up.rs:187-202`).
6. A departed scope loses its rows and digest rows (`remove_departed_scopes`, `catch_up.rs:155-185`, unchanged).
   A rebuild (`materialize_with_guard`, `materialize.rs:46-96`) validates every scope and uses the same
   hash, parse, hash-again step per unit, so `write_stamp` stores the digest of the bytes it loaded.

The validator change is the one behaviour change: an unchanged scope that fails validation stops refusing reads
until it changes. The pass that stored its digest validated it, so this can only happen when validator logic
changed without a migration. `provenance materialize` and `provenance check` still validate every scope. This is
question L.6.

### C.3 Byte identity of answers

Before and after item 1, for the same canonical bytes: every served answer is byte-identical with `stamp` and
`freshness_error` removed (the pinned answers file `tests/pinned_answers.json` does not change and
`READ_DERIVATION` stays `1`, `stamp.rs:20`); the unit digest recipe (`units.rs:45-66`) and the revision digest
recipe (`revision_digest_from_stored_rows`, `catch_up.rs:122`) are untouched, so the stamp's `digest` for the same
bytes is the same string; a pass that changes nothing commits no revision, so serial progression is unchanged.
What changes is the directory the bytes are read from and how many shards are parsed.

Tests kept green without edits: `catch_up_behavior.rs:59,74,105,134`, `catch_up_serial_behavior.rs:58-224`,
`catch_up_validation_behavior.rs:18,46,79,106`, `unit_digest_behavior.rs:14-95`, `scope_locality_guard.rs`,
`reader/guard.rs:20,61,96`, `wal_behavior.rs:32,121`, and the three comparison tests
(`comparison.rs:181-196`), which still hold their baseline column at this point (K.1).

New tests (RED first): `catch_up_reads_the_state_tree_in_place` (the recorded reads of `test_probes::record_read`
all lie under the repository layout and no temporary directory is created),
`an_unchanged_pass_parses_no_shard_but_the_manifest`, `a_changed_scope_alone_is_validated`,
`a_manifest_change_validates_every_scope`, `an_unchanged_invalid_scope_does_not_refuse_a_read`,
`an_edit_between_hash_and_parse_is_hashed_again` (a probe at a new label `catch_up_before_parse` edits a shard;
the stored digest is the digest of the parsed bytes), `a_store_under_the_guard_takes_no_second_lock`,
`a_rebuild_stores_the_digest_of_the_bytes_it_loaded`.

Rules: `rule_catch_up_hashes_canonical_state_in_place`, `rule_catch_up_validates_changed_units_only`,
`rule_catch_up_stores_the_digest_of_parsed_bytes`, `rule_store_under_guard_takes_no_second_lock`.

## D. Scan default tuning (item 2)

### D.1 Measurement

The scan behind `impact` is `scan_path_bounded` (`bounded.rs:19-50`): a sorted walk that skips `.git`,
`node_modules`, and `target` (`walker.rs:116-122`), counts files with a known extension (`walker.rs:30-40`: rs,
py, js, jsx, ts, tsx, java, go), reads each whole, and lexes it. `resolve_symbol` scans one file and never meets
the limit (`live.rs:100-116`).

Procedure, run at `03a935c`: clone each repository at depth 1, run `provenance init --scope default
--path-prefix .` in the clone, then time `provenance sdk impact` and `provenance sdk get` over the empty
projection, six runs each, medians of the last five. `impact` walks nothing (the id names no record) and looks up
no bindings, so `impact` minus `get` is the tree scan. The release build is the served product; the debug build
is what `cargo test` runs.

```sh
git clone --depth 1 https://github.com/tokio-rs/tokio.git tokio
provenance init --path tokio --scope default --path-prefix .
printf '%s' '{"id":"rule_none"}' | provenance sdk impact --repo tokio --scope default --format json
printf '%s' '{"node_type":"rule","id":"rule_none"}' | provenance sdk get --repo tokio --scope default --format json
```

| Repository | Commit | Language files | Source MB | Release scan ms | Release ms per file | Debug scan ms | Debug ms per file |
|---|---|---|---|---|---|---|---|
| ripgrep | 3fce3b5 | 110 | 1.9 | 17.5 | 0.159 | 121.0 | 1.100 |
| provenance | 03a935c | 740 | 4.3 | 49.1 | 0.066 | 333.8 | 0.451 |
| tokio | 060cc4e | 796 | 5.7 | 54.2 | 0.068 | 378.1 | 0.475 |
| rust-analyzer | 04a5986 | 1511 | 17.7 | 152.8 | 0.101 | 1275.5 | 0.844 |

The cost follows bytes more than files: rust-analyzer averages 11.7 KB per file and costs 8.6 ms per MB in
release; tokio averages 7.2 KB per file and costs 9.5 ms per MB. The 1.1 s for 657 files behind the current 2000
(`read_policy.rs:8-11`) was a debug figure taken through the `impact` command, which also loaded canonical state.

| Default | Release scan at the cut, about | Debug scan at the cut, about | Measured repositories cut |
|---|---|---|---|
| 2000 | 0.2 s | 1.7 s | none of the four |
| 5000 | 0.5 s | 4 s | none of the four |
| 10000 | 1.0 s | 8 s | none of the four |

### D.2 Choosing the default

The rule for the number: the largest round file count whose release scan stays under half a second at the
highest measured per-file cost (0.1 ms). That gives 5000. The cut then reaches only repositories whose scan would
take longer than half a second, and the answer says so through `scan_cut`. This is question L.1; the plan
recommends 5000 and keeps 2000 if Ben prefers the tighter bound.

Whatever the number, the constant's doc comment (`read_policy.rs:8-11`) cites the table above and the procedure,
so the next tuning repeats the measurement instead of inheriting a number. No code beyond the constant and its
comment changes; `impact_says_when_the_scan_was_cut` and `a_cut_scan_reads_the_same_files_twice` (`tests/impact.rs`)
set the limit through `ReadPolicy` and do not depend on the default.

## E. The `read.*` settings file (item 3)

### E.1 Location and format

`.provenance/settings.json`, beside `state/` and `cache/`. It is not canonical state (`docs/state-format.md:3-5`
forbids cache files and volatile fields in shards, and the settings are neither records nor cache) and it is not
cache (it must survive a cache delete), so it sits in neither directory. It is tracked in git, as `manifest.json`
is: the scan limit describes the repository, and a checkout-specific policy is set with the flag of E.3. JSON, as
every file under `.provenance` is; there is no TOML anywhere in the product's files.

```json
{
  "read": {
    "freshness_policy": "catch_up",
    "scan_limit": 5000
  }
}
```

Both keys are optional; an absent file means the built-in defaults. Unknown keys are refused at every level
(`deny_unknown_fields`), so a typo cannot pass as a default. `freshness_policy` is one of `catch_up`,
`annotate_only`, `refuse_stale`, the same words the stamp uses (`protocol/stamp.rs:28-41`). `scan_limit` is an
integer of at least 1. A later setting from another bead (`provenance-sfzf.1`, evidence storage per document
kind) can take its own top-level object in the same file.

### E.2 Loading

`crates/provenance-store/src/settings.rs` (new; the store does I/O, core stays pure):

```
pub struct Settings { pub read: ReadSettings }
pub struct ReadSettings { pub freshness_policy: Option<FreshnessPolicy>, pub scan_limit: Option<usize> }
impl Settings { pub fn load(layout: &ProvenanceLayout) -> Result<Self, SettingsError> }
pub enum SettingsError { Unreadable { path, error }, Invalid { path, key, problem } }
impl ReadPolicy { pub fn resolve(settings: &Settings, freshness: Option<FreshnessPolicy>) -> Self }
```

`FreshnessPolicy` gains `Deserialize` with the three words. `SettingsError` is `thiserror`; the `Invalid` message
reads `.provenance/settings.json: read.freshness_policy must be one of catch_up, annotate_only, refuse_stale
(found "fast")`, and for the limit `read.scan_limit must be a whole number of at least 1 (found 0)`. A read that
cannot resolve its policy refuses before it opens anything; it never falls back to a default (dispatch guidance
of 2026-09-01, criterion 6, on bead `provenance-1wh`).

The eight `queries::*` functions take `policy: ReadPolicy` in place of the `ReadPolicy::default()` inside
`served` (`queries.rs:30-37`). The only caller is `handlers/sdk/query.rs:24-64`, which resolves the policy once
per command; the Rust SDK crate does not call `queries::*`. An in-process consumer (section J) resolves once and
passes it on every call.

### E.3 Precedence and the flag

The eight `sdk` query commands gain `--freshness <catch_up|annotate_only|refuse_stale>` on `QueryArgs`
(`cli/sdk.rs:7-14`). Precedence: the flag, then `read.freshness_policy`, then the built-in `catch_up`. The scan
limit has no flag and no request field (`res_query_answers_stop_at_the_limit`): the file, then the built-in
default. No environment variable; the SDK's environment settings name the repository, scope, and owners
(`sdk/settings.rs:11-33`) and adding a third layer for one value buys nothing. The TypeScript package passes the
flag through when a caller sets `freshness` on a query call; the type is the existing `StampPolicy` minus
`catch_up_failed`. This is question L.3.

`docs/cli.md:138-139` ("no flag selects a policy yet, and the `read.freshness_policy` setting is reserved") is
replaced by a paragraph naming the file, the two keys, the flag, and the precedence.

Tests: `an_absent_settings_file_means_the_defaults`, `a_settings_file_sets_the_policy_and_the_limit`,
`an_unknown_settings_key_is_refused_by_name`, `a_bad_policy_word_is_refused_with_the_allowed_words`,
`a_zero_scan_limit_is_refused`, `the_freshness_flag_wins_over_the_settings_file` (CLI),
`the_stamp_names_the_policy_the_flag_chose` (CLI).

Rules: `rule_invalid_read_setting_is_a_typed_refusal`, `rule_freshness_flag_wins_over_the_settings_file`.

## F. Deleting the baselines (item 4)

### F.1 What is deleted

| Item | Where at `03a935c` | Callers |
|---|---|---|
| `records::load` | `queries/records.rs:20-82`, `#[cfg(test)]` | `queries/tests.rs:193`, `comparison/requests.rs:92` |
| `records::find` | `tests/baseline/records.rs:78` (the production copy went in PR 195) | the `records` baseline |
| `records` baseline | `tests/baseline/records.rs` (154 lines) | `comparison.rs:54-55` |
| `stale` baseline | `tests/baseline/stale.rs` (101 lines) | `comparison.rs:56` |
| `tests/baseline.rs` | 9 lines | the two above |
| the canonical side of the comparison tests | `comparison.rs`: `baseline_answer` (50-65), `assert_agreement` (121-146), the three `served_answers_match_the_baseline_*` tests (181-196), the `baseline_ms` column and `ratio` (`timing.rs:21-34,38-49`) | |
| `load_orders_new_kinds_after_every_settled_kind` | `queries/tests.rs:190-` | |

The `stale` baseline goes with the `records` one: it copies an operation that reads no projection table, so the
comparison it feeds is the operation against itself. `front_equivalence.rs` stays: `RecordFront` is production
code for the wiki and gaps, and the property guards the one traversal home.

### F.2 What moves and what guards each deleted path afterwards

| Deleted path | Evidence afterwards |
|---|---|
| `get` against canonical | the twelve pinned `get` answers (`pinned.rs:61-72`); `get_reads_a_retired_record_only_when_asked` (`tests/retired.rs`); `get_answers_a_domain_and_a_boundary_by_id` (`tests.rs:91`) |
| `search` against canonical | the four pinned `search` answers (`pinned.rs:73-82`); `search_visits_kinds_in_rank_order_and_stops_at_the_limit` (`tests/limits.rs`); `search_reaches_domains_and_boundaries_by_kind_and_text`, `default_search_keeps_the_six_settled_kinds_under_protocol_five` (`tests.rs:118,148`) |
| `stale` against canonical | the pinned `stale` answer over the two-commit pinned store (`pinned.rs:134-141`); `a_bad_base_is_refused_before_the_store_is_read` (`tests/reader.rs:323`); `evidence_without_a_base_lists_no_diff` (`tests/reader.rs:246`) |
| kind order across `load` | `rank_appends_the_new_kinds_after_the_six_settled_positions` (`tests.rs:174`) stays; a new served case `search_answers_new_kinds_after_every_settled_kind` replaces the `load` order test |
| the repository's own state | a new smoke, `every_operation_answers_over_the_repository_state`: each operation answers `Ok` with a stamp over the `repository_state` store (`test_stores.rs:94-106`); no baseline, no byte comparison |

`comparison/requests.rs::for_store` (`requests.rs:90-`) samples records through `records::load`; it samples through
the served `search` per kind instead (`Table<K>` listing through a snapshot is the alternative; `search` needs no
new reader). `test_stores.rs` keeps its four stores and `copy_tree`, which the timing report and the smoke use.

The timing report keeps one row per case (`operation request served_ms`), one summary row per operation,
`scan_ms`, `rebuild_ms`, and `catch_up_ms` (`timing.rs:38-76` minus the baseline column). The comparison is then
release to release, through the numbers in each release's notes (K.4).

For the record, the last baseline-against-served rows over a copy of this repository's state, release build,
`03a935c` (medians, ms):

| Operation | Baseline | Served | Ratio |
|---|---|---|---|
| get (24 cases, summary) | 1.6 | 1.0 | 0.62 |
| search (4 cases, summary) | 1.7 | 1.7 | 1.02 |
| scan | 0.1 | | |
| rebuild | | 121.1 | |
| unchanged catch-up pass | | 19.7 | |

Test count: the deletion removes four tests (`comparison.rs:181-208`) and one (`tests.rs:191`), and adds two.
The PR body names them.

Rule: `rule_pinned_answers_cover_every_operation`, verified by an exhaustion test that the pinned request set
names all eight operations.

## G. `refuse_stale` (item 5)

### G.1 The policy

`refuse_stale` takes the guard for the hash step only, as `catch_up` does (`freshness.rs:63-73`), hashes every
unit in place (the step of C.2.3, no parse, no validator, no write), and compares each digest to
`projection_unit_digests`. Every unit equal: the read goes on at the stored serial and the stamp says `policy:
"refuse_stale"`, which the stamp word already exists for (`protocol/stamp.rs:37`, `protocol.ts:215`). Any unit
different: the read refuses. It never inserts a revision row and never rewrites a family. It shares the
`annotate_only` refusals for an absent, migration-behind, or half-migrated database (`freshness.rs:106-145`), since
no freshness step will run to fix them. `ReadRefusal::RefuseStaleUnimplemented` (`reader.rs:55-56`) is deleted.

When the lock cannot be taken (a cache directory this process cannot write, section I), the hash runs unlocked.
A publication landing during an unlocked hash can only make a unit differ from its stored digest, so a torn read
refuses and never answers stale.

### G.2 The typed refusal

```
ReadRefusal::Stale {
    database: Utf8PathBuf,
    serial: i64,
    instance_id: String,
    moved: Vec<MovedUnit>,   // MovedUnit { unit: String, stored: String, live: String }, sorted by unit name
}
```

A new scope has `stored` empty; a departed scope has `live` empty. The message is one line:

```
refuse_stale: the projection in .provenance/cache/provenance.db at serial 41 (instance 9a56781d-...) is behind
canonical state; moved: scope:default (stored sha256:ab12..., live sha256:cd34...). Read under catch_up or run
`provenance materialize`.
```

On the wire the CLI prints errors to stderr and exits 1 (`main.rs:17-21`); the TypeScript engine turns that into
a thrown `Error` carrying the text (`engine.ts:88-92`). W5 lands the Rust type and the message. A JSON refusal
envelope is the contract layer's (`provenance-80qn`: "typed-refusal channel, StaleBase and siblings"), and W5 does
not invent one ahead of it. This is question L.4.

Callers who see it: the CLI under `--freshness refuse_stale` or the settings file; the Rust SDK through
`anyhow::Error::downcast_ref::<ReadRefusal>()`; the TypeScript SDK through the message; an MCP server (section J)
through the type.

Tests: `refuse_stale_answers_at_the_stored_serial_when_every_unit_matches`,
`refuse_stale_refuses_and_names_the_moved_unit` (stored and live digests both present),
`refuse_stale_names_a_new_scope_with_no_stored_digest`, `refuse_stale_writes_no_revision_row`,
`refuse_stale_on_a_read_only_checkout_still_decides` (unix), `refuse_stale_refuses_a_half_migrated_projection`.

Rules: `rule_refuse_stale_names_the_moved_units`, `rule_refuse_stale_writes_no_revision`.

## H. `catch_up_failed` (item 7)

Recommendation: keep the word. It names the step that ran (`catch_up`) and its outcome (`failed`) in the words of
the policy list, and the answer beside it is at the stored serial with the error text in `freshness_error`
(`freshness.rs:79-96`, `docs/cli.md:136-138`). The alternatives hide one half: `stored` and `stale_answer` drop
the step, `catch_up_refused` drops the I/O case. The word is already on the wire (`protocol.ts:216`), in the docs,
and in the TypeScript union; a rename would cost a type change for no gain in meaning. No code change; K.2 records
the confirmation in `docs/cache.md`.

## I. Read-only checkouts under WAL (item 8)

Recommendation: keep the immutable fallback. In WAL mode a reader needs a writable `-shm` file, and a completed
read removes the `-wal` and `-shm` files (`rule_completed_read_leaves_no_wal_files`, `cache.rs:127-147`), so a
read-only checkout usually holds neither; `immutable(true)` (`open_immutable_cache`, `cache.rs:96-105`) is the one
open SQLite offers that needs no lock and no `-shm`. The alternative, copying the database to a temporary
directory, costs the file size on every read and reintroduces a copy W5 is removing.

One defect goes with it: under `annotate_only` the same checkout refuses as `NoProjection` today, because every
open error maps to that refusal (`freshness.rs:41-43`). After W5 every policy falls back to the immutable open
when the ordinary open fails for a permission reason, keeps its own policy word (no freshness step failed under
`annotate_only`; `refuse_stale` decides through an unlocked hash, G.1), and `catch_up` keeps stamping
`catch_up_failed` with the permission error in `freshness_error`, as today (`a_read_only_checkout_answers_at_its_serial`,
`reader/freshness.rs:106`).

Text for `docs/cache.md`, after the "Read path" paragraph (`docs/cache.md:79-93`):

```
A checkout whose cache directory this process cannot write still answers. WAL needs a writable -shm file
beside the database, and a finished read removes the -wal and -shm files, so the reader opens the database as
an immutable image instead: SQLite then takes no lock and reads the file as it is. Under catch_up the stamp
says catch_up_failed and freshness_error names the permission error; under annotate_only and refuse_stale the
policy word is unchanged. The answer is at the serial the file holds. This fallback is exercised on Unix; on
Windows a read-only directory is not reproduced in the test suite.
```

Tests: `annotate_only_on_a_read_only_checkout_answers_at_its_serial` (unix), and the two refuse_stale cases of G.

Rule: `rule_read_only_checkout_answers_as_an_immutable_image`.

## J. MCP consumer handoff criteria

The MCP server (`provenance-q82f`) is a long-running process that answers many reads. It does not need a
persistent `ReadContext`: a context is one read (one transaction pinned at one serial, one stamp derived from the
handles taken, consumed by `stamp::seal`, `reader.rs:122-158`), and holding one open would pin a WAL snapshot and
block checkpoints for the life of the server. What it needs is a cheap per-call entry. The criteria, each a fact a
reviewer can check on the tree:

1. Entry. The eight operations are callable in process with an explicit `ReadPolicy` (E.2), and the policy is
   resolved once from `.provenance/settings.json` plus an override (a server can run `annotate_only` between its
   own refresh calls, or `catch_up` on every call).
2. Cost. An unchanged `catch_up` pass parses no shard (C.2), so the per-call freshness cost is the hash of the
   state bytes plus the guard and the pool open; the numbers for this repository and the synthetic trees are in
   C.1 and in the release notes (K.4). A server that wants zero per-call cost runs `annotate_only` and refreshes
   on its own clock through `catch_up_state` (`catch_up.rs:32-41`).
3. Refusal. `refuse_stale` (G) gives a server a typed way to decline a stale answer, with the moved units in the
   error.
4. No process-global state on the read path. `test_probes` is `cfg(test)` only (`test_probes.rs:10-39`); the
   read future is `Send` (`reader.rs:13-15,43`); nothing on the path calls `std::env::set_var`. A source-scan
   test over `operations/` asserts the last point.
5. Blocking sections named. The guard is taken on the blocking pool (`guard.rs:74-81`), but the synchronous lock
   sections behind the `Live::Canonical` handle (`graph_evidence`, `live.rs:73-80`, through `health.rs:65`) and the
   ideation readers block a runtime worker in `flock` while another holder has the lock (`guard.rs:7-12`). The
   handoff names these call sites in a table in `docs/cache.md`; moving them under `spawn_blocking` is the MCP
   bead's first task, not W5's.
6. Concurrency. Two reads in one process each open their own one-connection pool (`cache.rs:128-147`) and pin
   their own snapshot; two `catch_up` reads serialize on the file lock. A test with two concurrent `answer` calls
   over one store asserts both answer and both leave no `-wal` file.
7. Discovery. `discover_repository` runs per call today (`queries.rs:35`); a server passes a fixed repository
   path, which the `ReadPolicy` change leaves as an `Option<Utf8PathBuf>` argument.
8. Contract. The wire shapes a server projects are the ones `protocol.ts:205-251` already spells: the stamp, the
   four `has_more` flags, `scan_cut`, `freshness_error`, and the refusal text of G.2 until the contract layer
   gives it an envelope.

"Handoff criteria met" means: criteria 1 to 4 and 6 to 8 hold on the release commit with a test or a doc line
each; criterion 5's table is written; the bead comment lists the commit for each. Nothing here is an MCP
implementation.

## K. Staging

Four PRs, in dependency order. Each is green at every commit (fmt, clippy with all targets and features, the Rust
and TypeScript suites), deslopped before ready, reviewed by one adversarial reviewer, and lands its Rule records
and bindings. Each PR body states the test count before and after with the added and deleted tests named, and
asserts the pinned answers file is unchanged. Section F's deletion comes last on purpose: through K.1 to K.3 the
baseline comparison over the repository's state still guards `get`, `search`, and `stale` byte for byte.

*K.1, branch `1wh-w5-in-place-catch-up`: section C.* Steps: `StateStore::under_guard` and the compile-fail move;
in-place hashing in catch-up with the hash, parse, hash-again step; validators on changed units and on every
scope after a global change; the rebuild in place; delete the snapshot machinery; `docs/cache.md:38-47` says the
pass reads the tree in place. Byte identity: pinned answers unchanged; the three comparison tests green.
File cap: `catch_up.rs` (279) grows by about 50 lines; the validation step goes in `materialize/validation.rs`
(new) if `catch_up.rs` passes 400. Rules: the four of C.3.

*K.2, branch `1wh-w5-read-settings`: sections E, H, I.* Steps: `settings.rs`, `ReadPolicy::resolve`, the eight
`queries::*` take a policy, `--freshness` on `QueryArgs`, the TypeScript `freshness` option; the immutable fallback
under every policy; `docs/cli.md` and `docs/cache.md` text; the scan default of D with its doc comment. Byte
identity: no served answer changes; the stamp's `policy` word is the resolved policy. Rules: the two of E.3 and
the one of I.

*K.3, branch `1wh-w5-refuse-stale`: section G.* Depends on K.1 (the in-place hash without a write). Steps: the
hash-only step as a function shared with catch-up; `ReadRefusal::Stale`; the policy arm in `freshness::run`;
the message; tests. Byte identity: answers under `refuse_stale` when current equal answers under `annotate_only`
with the policy word swapped, asserted over the pinned store. Rules: the two of G.2. Files: `reader/refuse_stale.rs`
(new) for the step, `tests/reader/refuse_stale.rs` (new) for its tests, so `freshness.rs` (162) and
`tests/reader/freshness.rs` (262) stay where they are.

*K.4, branch `1wh-w5-baseline-removal`: section F and the release mechanics.* Steps: delete the baselines and the
canonical side; the two replacement tests; the timing report's served-only columns; `docs/release.md` gains the
command and the checklist line below; section J's criteria 4 and 6 tests and the criterion 5 table; the bead's
handoff comment. Byte identity: pinned answers unchanged; the test count moves by the named tests only. Rules:
the one of F.2 and `rule_timing_comparison_runs_by_hand_only` (a source-scan test asserts the timing test carries
`#[ignore]`).

The release cuts after K.4. The checklist line for `docs/release.md`, under "Cut A Release" before the tag step
(`docs/release.md:45-59`):

```sh
cargo test -p provenance-store --release -- --ignored timing_comparison_rows --nocapture 2>/dev/null \
  | grep -E 'summary|_ms' > /tmp/timing-$(git rev-parse --short HEAD).txt
```

The summary rows and the three `_ms` rows for `repository_state` go into the GitHub Release body after the
workflow generates it, under a heading "Timing (release build, `<commit>`)". The mechanics are question L.5; the
plan recommends this documented command plus the checklist line over a `cargo xtask` (the workspace has no xtask
crate, `Cargo.toml:1-10`, and one command does not earn one).

## L. Questions for Ben

Each is self-contained; the plan proceeds on the recommendation unless told otherwise.

1. The scan default. The release build scans about 0.1 ms per file at worst (rust-analyzer, 1511 files, 0.15 s;
   the debug build takes 1.3 s). Keep 2000, or raise to 5000, which keeps the worst release scan under half a
   second and cuts none of the four measured repositories? Recommendation: 5000.
2. The settings file. Put `read.freshness_policy` and `read.scan_limit` in `.provenance/settings.json`, tracked in
   git beside `state/` and `cache/`, in JSON like the manifest? Recommendation: yes, that file and that place.
3. The flag. Give the eight `sdk` query commands `--freshness <catch_up|annotate_only|refuse_stale>`, winning over
   the file, so a CI job or a server can pick a policy without editing a tracked file? Recommendation: yes; no
   environment variable.
4. The refusal on the wire. Ship `refuse_stale` as a typed Rust error whose one-line message names the serial and
   each moved unit, printed to stderr with exit 1 as every error is today, and leave the JSON refusal envelope to
   the contract layer bead? Recommendation: yes, no envelope ahead of `provenance-80qn`.
5. The timing report. Record it as one documented command and one checklist line in `docs/release.md`, with the
   rows pasted into the GitHub Release body, rather than a `cargo xtask`? Recommendation: the command and the
   checklist line.
6. Validation on unchanged scopes. Let catch-up validate only the scopes whose bytes moved (and every scope when
   the manifest moved), so an unchanged pass parses nothing? The one visible change: a scope that fails a newer
   validator without changing no longer refuses reads until `provenance materialize` or `check` runs.
   Recommendation: yes; it is most of the per-read cost.

## M. Sources

- W3 plan, revision 6, with four reviews: `source_w3_plan_rev6_2026_09_05`, gist
  `https://gist.github.com/BNasraoui/8b3824425806bac4d123715edb6918cf/b6ddd8d5fe2a7869ad137e8f4c08bc9c421944c5`
  (file 01-plan, branch `1wh-w3-plan` at `60ab19b`); sections E, E.2, J, B, and M are this plan's charter.
- Ben's rulings of 2026-09-05: `source_w3_rulings_session_2026_09_05`.
- Resolutions: `res_query_answers_stop_at_the_limit`, `res_impact_follows_declared_flow`,
  `res_projection_tables_mirror_record_types`, `res_stamp_names_projection_instance`,
  `res_catch_up_hashes_scopes_no_journal`; boundary `boundary_query_answers_do_not_page`; requirement
  `req_query_answers_carry_a_freshness_stamp`.
- Bead `provenance-1wh`, comment of 2026-09-01 (dispatch guidance), criterion 6: invalid config is a typed error.
- Beads `provenance-q82f` (MCP server) and `provenance-80qn` (contract layer) for section J and G.2.
- Measurements: release CLI built from `03a935c` with `cargo build --release -p provenance-cli`, debug CLI with
  `cargo build -p provenance-cli`, `cargo test -p provenance-store --release -- --ignored timing_comparison_rows
  --nocapture`, and the clone-and-time procedure of D.1; medians, single machine, 2026-09-06. The scripts are
  reproducible from the commands in C.1 and D.1; on approval this plan goes to a gist with its reviews and is
  cited as a Source, as the W3 plan was.
