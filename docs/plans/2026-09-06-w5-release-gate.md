# W5 release gate: implementation plan (revision 2)

Bead `provenance-1wh.3`. Read at `03a935c` (W3 fully merged: PRs 192, 193, 194, 195, 196). Every `path:line` below
was counted there. This document is the W5 charter of the approved W3 plan, section M, worked out against the
tree as it stands. Section B is the new bead text. Section L holds the questions for Ben. Section N answers the
adversarial review of revision 1.

Revision 2 folds that review: the readers lock by path, not through the store, so K.1 is split into a
readers-threading PR and an in-place PR (C.2.2, K.1a, K.1b); the read-only cases get a lock-free scope listing
and a fixture that makes the lock untakeable (G.1, I); the 500-line gate is named as already live (B); the
rebuild numbers are re-measured and labelled (C.1); the validators-on-changed-units change is paired with a
validation version that routes to a rebuild (C.2.7); the refusal carries the stored digest (G.2); the rules name
their parent requirement (C.3, E.3, G.2, I, K.2).

Settled and not reopened: no pagination and no cursors anywhere (`res_query_answers_stop_at_the_limit`,
`boundary_query_answers_do_not_page`); the scan limit is a configured default, not a request field; the timing
comparison is a hand-run report and never a CI gate; core stays pure; freshness annotates and never refuses,
except the explicit `refuse_stale` policy; code files stay under 500 lines; every implementation PR lands its Rule
records and bindings.

## A. Purpose and the release rule

W3 is merged but not released. W3 and W5 ship together in one release, so this bead gates the release. The rule:
no version tag is pushed until every PR in section K is merged and the timing report of section K.4 has run on
the release commit and met its criterion. The bead is P2 while `provenance-1wh.2` was P1; raising it is question
L.6.

W5 does four kinds of work. It removes cost from the read path (the tree copy and the every-pass validators). It
fills the gaps W3 left (the `read.*` settings, `refuse_stale`). It deletes what W3 kept only for comparison
(`records::load`, the `records` baseline, the canonical side of the comparison tests). It writes down what the
next consumer needs (the MCP handoff criteria).

## B. New bead text for `provenance-1wh.3`

Title: `W5: release gate for the served read path`

Description:

```
Goal. Close the release gate the W3 plan (section M) left open, so W3 and W5 ship in one release.

Where the code stands at 03a935c. Every catch_up read copies the state tree to a temporary directory
(publication.rs:406-435, catch_up.rs:68), runs both validators over every scope (catch_up.rs:72-75), and
then hashes every unit. Every canonical reader takes the publication lock by path
(with_state_path_access, publication.rs:437-453), which is why the pass reads a copy: on the real tree under a
held guard the first reader would take a second lock and block. The scan stops at DEFAULT_SCAN_LIMIT = 2000
(read_policy.rs:11), a number taken from one debug-build measurement. No settings file exists;
queries::served uses ReadPolicy::default() (queries.rs:37). refuse_stale is reserved and refuses as
unimplemented (freshness.rs:54). records::load survives under cfg(test) (records.rs:20-82) for the records
baseline and the comparison tests. The timing comparison is an ignored test (comparison.rs:198-208). A
read-only checkout answers through an immutable open (freshness.rs:79-96), undocumented in docs/cache.md.
The 500-line file cap is already a CI gate: tracked_rust_files_stay_below_the_hard_line_limit
(crates/provenance-cli/tests/cli_structure.rs:31-42) runs in every CI test job (ci.yml:197); nothing more is
built for it.

Items.
1. Canonical readers take their lock state from the store they read through. A store built from a held
   publication guard reads without a second lock; a plain store locks by path as today.
2. Catch-up hashes each unit in place under the guard. No tree copy. Validators run for the units that moved
   (every scope when the global unit moved, or when the validation version moved, which routes to a rebuild).
   A changed scope re-derives only the families whose content digest moved, as today. The digest a pass
   stores is the digest of the bytes it parsed.
3. The scan default is set from a measurement over four repositories in release and debug builds, stated in
   files and in bytes; the procedure is documented beside the constant.
4. .provenance/settings.json carries read.freshness_policy and read.scan_limit. An invalid value is a typed
   settings refusal raised before any read, never a freshness outcome. The eight sdk query commands take
   --freshness, which wins over the file.
5. records::load, records::find, the records and stale baselines, and the canonical side of the comparison
   tests are deleted. Their cases live in the pinned answers file and the served tests; one count invariant
   over the repository's own state stays.
6. refuse_stale hashes in place without writing, answers at the stored serial when every unit matches, and
   otherwise refuses with a typed error naming the stored serial and digest and each moved unit with its
   stored and live digests. A unit it cannot hash is a refusal, not an answer.
7. The timing comparison runs by hand in release on the release commit; its rows go in the release notes and
   are compared to the previous release's rows against a stated criterion.
8. catch_up_failed stays the policy word for a failed freshness step.
9. A read-only checkout answers as an immutable image under every policy, with a lock-free scope listing, and
   docs/cache.md says so.
10. The MCP handoff criteria of the plan's section J are met and recorded on this bead.

Rulings this text rests on: res_query_answers_stop_at_the_limit, res_impact_follows_declared_flow,
res_projection_tables_mirror_record_types, res_stamp_names_projection_instance,
res_catch_up_hashes_scopes_no_journal, boundary_query_answers_do_not_page. Plan:
docs/plans/2026-09-06-w5-release-gate.md on branch 1wh-w5-plan.
```

Acceptance criteria:

```
- A store built from a held guard reads every family without blocking while the guard is held; a plain
  store still waits for the lock; both are tests.
- An unchanged catch-up pass opens no temporary directory and parses no shard other than the manifest; a
  test records every canonical path read and asserts it.
- An invalid scope that did not change does not refuse a read; a changed invalid scope still refuses and
  commits nothing; a manifest change validates every scope; a validation version move rebuilds.
- The pinned answers file is byte-identical before and after each W5 PR, and READ_DERIVATION stays 1.
- Per kind, the projection's row count over a copy of this repository's state equals the canonical
  reader's record count, after the baselines are gone.
- .provenance/settings.json is read on every query; a bad key, a bad word, or a non-positive scan_limit
  refuses with the file path, the key, and the allowed values in the message, before any read.
- --freshness on the eight sdk commands overrides the file; the stamp's policy word is the policy that ran.
- refuse_stale answers with policy "refuse_stale" when current and refuses with the stored serial, the
  stored digest, and the moved units when not; it never writes a revision row.
- records::load, records::find, tests/baseline/, and the baseline column of the comparison tests are gone;
  the test count after each PR equals the count before it plus the tests the PR names as added minus the
  tests it names as deleted.
- docs/release.md carries the timing report command, the checklist line, and the pass criterion;
  docs/cache.md documents the read-only checkout behaviour and the blocking sections; docs/cli.md documents
  the settings file and the flag.
- Each PR lands its Rule records, each under a named requirement, and #[rule]/#[verifies] bindings;
  provenance coverage scan --validate-rules is clean.
- The MCP handoff criteria (section J) are checked off on this bead with the commit that met each one.
```

## C. In-place hashing (items 1 and 2)

### C.1 What the read pays today

Under `catch_up` every read runs `catch_up_with_guard` (`catch_up.rs:45-130`). The pass copies the whole state
tree into a temporary directory (`snapshot_state_under_guard`, `guard.rs:94-99`, through `snapshot_state_unlocked`
and `copy_tree`, `publication.rs:406-435`), builds a `StateStore` over the copy (`catch_up.rs:69`), runs both
validators over every scope (`catch_up.rs:72-75`; `validate_graph_scope` parses seven kinds,
`graph_validation.rs:23-45`; `validate_ideation_scope` parses five ideation shards under its own lock section,
`ideation_batches.rs:127-132`), and only then hashes each unit (`catch_up.rs:98-106`, `units.rs:45-66`). A rebuild
does the same copy (`materialize.rs:50`) and hashes the copy after loading (`materialize/stamp.rs:44-51`).

The measurements below were taken on this repository at `03a935c` with the release CLI built from the tree,
medians after one warm-up run. Each row says what it includes. The synthetic trees hold the default scope's
graph and thread shards copied under N scopes (the ideation shards are pinned to one scope by the lifecycle
validator and were left out of the copies).

| Step, this repository (17 files, 1.0 MB of state) | ms | What the number includes |
|---|---|---|
| `provenance --version` | 3.3 | process start-up |
| `provenance sdk get` under `catch_up`, unchanged projection | 27.4 (26.3 to 28.4 over three sessions) | start-up, guard, tree copy, validators, hash, read |
| `provenance sdk get`, first read on a fresh cache | 153.0 | start-up, guard, database creation, 22 migrations, rebuild, read |
| `provenance materialize`, existing database | 119.1 | start-up, guard, tree copy, validators, clear and reload, stamp |
| `provenance materialize`, fresh cache | 157.3 | the same plus database creation and 22 migrations |
| in-process unchanged pass (`catch_up_ms`, `repository_state`, release test) | 21.8 (19.7 to 27.0 over four runs, one outlier at 76.6) | guard, tree copy, validators, hash; no process start-up |
| in-process first catch-up on a fresh file (`rebuild_ms`, same store) | 144 (121.1, 139.1, 148.2; one outlier at 863) | database creation, migrations, rebuild; the first SQLite open of the test binary |
| copy of the state tree (python `shutil.copytree`) | 2.6 | |
| sha256 of every state file in place (python) | 2.1 | |

Revision 1 had the two rebuild figures wrong (738 ms for `materialize` and 121 ms for the in-process rebuild);
the 738 came from one session under load, and the in-process number is the noisiest row here because it holds
the test binary's first SQLite open. The reviewer's 596 ms for that row is the same noise.

| Synthetic tree | files | MB | `sdk get` under `catch_up` (ms) | `materialize`, existing database (ms) | tree copy (ms) | hash in place (ms) |
|---|---|---|---|---|---|---|
| 20 scopes | 207 | 8.2 | 153.6 | 1070 | 26.0 | 17.8 |
| 100 scopes | 1007 | 38.4 | 680.9 | 4856 | 138.3 | 94.2 |

The copy is 12 to 20 percent of an unchanged pass and the hash 11 to 14 percent. The remainder is the
validators parsing every shard of every scope, plus the manifest read, the guard, and the pool open. The python
sha256 figures are a ceiling; `canonical_digest::digest` uses `sha2` and hashes faster.

### C.2 The pass after items 1 and 2

1. `catch_up_with_guard` reads `layout.state_dir()` directly. `snapshot_state_under_guard`, `StateSnapshot`,
   `snapshot_state`, `snapshot_state_unlocked`, and `copy_tree` are deleted (`guard.rs:85-99`,
   `publication.rs:17-26,402-435`). The compile-fail case `tests/compile_fail/forged_guard.rs` and its doctest
   twin (`guard.rs:87-93`) move onto the constructor in step 2, which is the same proof.
2. Readers take their lock state from the store. Today every family reader locks by path: `read_jsonl`,
   `read_ideation_landings`, `read_legacy_dispositions`, and `read_jsonl_closed` (`readers.rs:184,194,205,250`)
   call `with_state_path_access` (`publication.rs:437-453`), which derives a layout from the path and calls the
   free `with_repository_publication`; `read_message_shards` (`readers.rs:309`) calls the free function too.
   The `StateStore` method of the same name (`publication.rs:456-461`) is a second entry the readers never use.
   So `validate_graph_scope` (`graph_validation.rs:24-30`), `rederive_scope` (`catch_up.rs:227`),
   `relation_rows::scope_rows` (`relation_rows.rs:14-34`), the artifact index (`canonical_artifacts.rs:24-80`),
   `manifest()` (`state_store.rs:99-107`), and `validate_ideation_scope` (`ideation_batches.rs:127-132`) all take
   the lock, and on the real layout under a held guard (taken on the blocking pool through its own file
   description, `guard.rs:74-81`) the first of them is a second `flock` on the same file, which blocks for ever.
   The change: `StateStore` gains a private `access: Access` field (`enum Access { Lock, Held }`); the five
   reader functions take `&StateStore` as their first argument and lock through
   `store.state_path_access(path, op)`, a method that records the read (`test_probes::record_read`) and locks
   only under `Access::Lock`; the store's `with_repository_publication` reads the same field. Call sites: 28 in
   `state_store.rs`, 7 in `ideation_batches.rs`, 6 inside `readers.rs`; nothing under `cache/` reads a shard
   except through the store. `StateStore::under_guard(&'g PublicationGuard, layout) -> GuardedStore<'g>`
   builds the `Held` form; `GuardedStore<'g>` holds `PhantomData<&'g PublicationGuard>`, derefs to `&StateStore`,
   and is not `Clone`, so the proof cannot outlive the guard. `StateStore` keeps `Clone` for the plain form
   (`state_store.rs:70-71`) through a hand-written `impl` that always yields `Access::Lock`, so a clone taken
   through the deref locks again. Rejected: `with_read_only_validation` (`read_only.rs:23-31`), thread-local
   and unseen by a task that moves between worker threads (`main.rs:16-21`); a process-wide set of held lock
   paths, which is process-global read-path state (J.4) and would let one task skip a lock another holds.
   This lands alone as K.1a, with the snapshot still in place: zero behaviour change.
3. Per unit, in the sorted order of `units_for` (`units.rs:36-42`): hash in place. An unchanged unit ends here, as
   today (`catch_up.rs:100-102`). No shard of an unchanged scope is parsed.
4. A changed scope unit: run both validators for that scope, parse its nineteen families, rewrite the families
   whose content digest moved, reload the scope's `relations` rows when an owner family moved
   (`rederive_scope`, `catch_up.rs:218-244`, unchanged), then hash the unit again. When the second digest equals
   the first, that digest is stored. When it differs, the bytes moved between hash and parse, which only an
   unlocked editor can do (every canonical writer locks through `with_repository_publication`,
   `state_store/*_writers.rs`, and the pass holds the guard throughout, `freshness.rs:64-72`); the pass repeats
   the unit, at most three times, then refuses with `canonical state changed during catch-up under <scope>`.
   A refusal commits nothing; on the read path it is a failed freshness step, so the answer comes at the
   stored serial with `policy: "catch_up_failed"` and that text in `freshness_error`.
5. A changed global unit: store its digest, and run the validators for every scope, because
   `manifest.disposition_actor_ids` feeds the ideation validator (`ideation_batches.rs:129-130`). No family derives
   from the global unit (`catch_up.rs:187-202`).
6. A departed scope loses its rows and digest rows (`remove_departed_scopes`, `catch_up.rs:155-185`, unchanged).
   A rebuild (`materialize_with_guard`, `materialize.rs:46-96`) validates every scope and uses the same
   hash, parse, hash-again step per unit, so `write_stamp` stores the digest of the bytes it loaded.
7. A validation version. `pub const VALIDATION_VERSION: u32` in `provenance-store` (the validators live there),
   with a numbered history in its doc comment as `READ_DERIVATION` has (`stamp.rs:7-19`). Migration 023 adds a
   one-row table `projection_validation (only_row, version)`; a rebuild writes the constant; a catch-up pass
   whose stored version differs from the constant routes to a rebuild exactly as an applied migration does
   (`catch_up.rs:64-66`). A release that tightens a validator moves the constant, so an unchanged scope that
   the newer validator would refuse is re-validated once, on the first read after the upgrade, instead of being
   served for ever. `provenance materialize` and `provenance check` validate every scope regardless.

The validator change is the one behaviour change: an unchanged scope that fails validation stops refusing reads
until it changes, the manifest changes, or the validation version moves. This is question L.5.

### C.3 Byte identity of answers

Before and after items 1 and 2, for the same canonical bytes: every served answer is byte-identical with `stamp`
and `freshness_error` removed (the pinned answers file `tests/pinned_answers.json` does not change and
`READ_DERIVATION` stays `1`, `stamp.rs:20`); the unit digest recipe (`units.rs:45-66`) and the revision digest
recipe (`revision_digest_from_stored_rows`, `catch_up.rs:122`) are untouched, so the stamp's `digest` for the same
bytes is the same string; a pass that changes nothing commits no revision, so serial progression is unchanged.
What changes is the directory the bytes are read from and how many shards are parsed. Migration 023 moves the
serial once, as every migration does.

Tests kept green without edits: `catch_up_behavior.rs:59,74,105,134`, `catch_up_serial_behavior.rs:58-224`,
`catch_up_validation_behavior.rs:18,46,79,106`, `unit_digest_behavior.rs:14-95`, `scope_locality_guard.rs`,
`reader/guard.rs:20,61,96`, `wal_behavior.rs:32,121`, and the three comparison tests
(`comparison.rs:181-196`), which still hold their baseline column at this point (K.1a, K.1b).

New tests, written first. K.1a: `a_store_under_the_guard_takes_no_second_lock` (a guard held on the blocking
pool; `under_guard(..).list_sources` and `validate_ideation_scope` return), `a_plain_store_still_waits_for_the_lock`
(the `test_probes` pattern of `materialize_guard_behavior.rs`), `a_cloned_store_locks_again`. K.1b:
`catch_up_reads_the_state_tree_in_place` (the recorded reads of `test_probes::record_read` all lie under the
repository layout and no temporary directory is created), `an_unchanged_pass_parses_no_shard_but_the_manifest`,
`a_changed_scope_alone_is_validated`, `a_manifest_change_validates_every_scope`,
`an_unchanged_invalid_scope_does_not_refuse_a_read`, `an_edit_between_hash_and_parse_is_hashed_again` (a probe
at a new label `catch_up_before_parse` edits a shard; the stored digest is the digest of the parsed bytes),
`a_retry_that_runs_out_answers_catch_up_failed`, `a_rebuild_stores_the_digest_of_the_bytes_it_loaded`,
`a_validation_version_move_routes_catch_up_to_a_full_rebuild`.

Rules, all under `req_query_answers_carry_a_freshness_stamp`: `rule_store_under_guard_takes_no_second_lock`
(K.1a); `rule_catch_up_hashes_canonical_state_in_place`, `rule_catch_up_validates_changed_units_only`,
`rule_catch_up_stores_the_digest_of_parsed_bytes`, `rule_validation_version_move_rebuilds_the_projection` (K.1b).

## D. Scan default tuning (item 3)

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

| Repository | Commit | Language files | Source MB | Release scan ms | Release ms per file | Release ms per MB | Debug scan ms | Debug ms per file |
|---|---|---|---|---|---|---|---|---|
| ripgrep | 3fce3b5 | 110 | 1.9 | 17.5 | 0.159 | 9.2 | 121.0 | 1.100 |
| provenance | 03a935c | 740 | 4.3 | 49.1 | 0.066 | 11.4 | 333.8 | 0.451 |
| tokio | 060cc4e | 796 | 5.7 | 54.2 | 0.068 | 9.5 | 378.1 | 0.475 |
| rust-analyzer | 04a5986 | 1511 | 17.7 | 152.8 | 0.101 | 8.6 | 1275.5 | 0.844 |

The cost follows bytes: 8.6 to 11.4 ms per MB in release, and the per-file figure only tracks the average file
size (rust-analyzer 11.7 KB, tokio 7.2 KB). The 1.1 s for 657 files behind the current 2000
(`read_policy.rs:8-11`) was a debug figure taken through the `impact` command, which also loaded canonical state.

| Default | Release scan at the cut, about | Debug scan at the cut, about | Holds while the scanned source is under | Measured repositories cut |
|---|---|---|---|---|
| 2000 | 0.2 s | 1.7 s | 20 MB | none of the four |
| 5000 | 0.5 s | 4 s | 50 MB | none of the four |
| 10000 | 1.0 s | 8 s | 100 MB | none of the four |

### D.2 Choosing the default

The rule for the number: the largest round file count whose release scan stays under half a second at the
highest measured per-file cost (0.1 ms per file, which is 10 KB per file at 10 ms per MB). That gives 5000 files,
and it holds while the scanned source is under about 50 MB; a repository of 5000 files at 30 KB each scans in
about 1.3 s, and the file count cannot see that. The cut then reaches only repositories whose scan would take
longer than half a second at ordinary file sizes, and the answer says so through `scan_cut`. This is question
L.1; the plan recommends 5000 and keeps 2000 if Ben prefers the tighter bound.

Whatever the number, the constant's doc comment (`read_policy.rs:8-11`) states the file count, the bytes
assumption behind it (10 KB per file, 10 ms per MB), the table above, and the procedure, so the next tuning
repeats the measurement instead of inheriting a number. No code beyond the constant and its comment changes;
`impact_says_when_the_scan_was_cut` and `a_cut_scan_reads_the_same_files_twice` (`tests/impact.rs`) set the limit
through `ReadPolicy` and do not depend on the default. A limit in bytes is not proposed: the ruling names a file
count, and `scan_path_bounded` counts files.

## E. The `read.*` settings file (item 4)

### E.1 Location and format

`.provenance/settings.json`, beside `state/` and `cache/`. It is not canonical state (`docs/state-format.md:3-5`
forbids cache files and volatile fields in shards, and the settings are neither records nor cache) and it is not
cache (it must survive a cache delete), so it sits in neither directory. It is tracked in git, as `manifest.json`
is: the scan limit describes the repository, and a checkout-specific policy is set with the flag of E.3. JSON, as
every file under `.provenance` is; there is no TOML anywhere in the product's files. Nothing under `.provenance`
reads a settings or config file today, and `prepare_publication_lock` (`publication.rs:69-90`) touches only
`cache/` and `cache/locks`, so the name collides with nothing.

```json
{
  "read": {
    "freshness_policy": "catch_up",
    "scan_limit": 5000
  }
}
```

Both keys are optional; an absent file means the built-in defaults. Unknown keys are refused at every level, so
a typo cannot pass as a default. `freshness_policy` is one of `catch_up`, `annotate_only`, `refuse_stale`, the
same words the stamp uses (`protocol/stamp.rs:28-41`). `scan_limit` is an integer of at least 1. A settings
change moves no unit digest, so the stamp does not say which scan limit or policy word was configured; the stamp's
`policy` says which policy ran, and `scan_cut` says whether the limit was met. A later setting from another bead
(`provenance-sfzf.1`, evidence storage per document kind) can take its own top-level object in the same file.

### E.2 Loading

`crates/provenance-store/src/settings.rs` (new; the store does I/O, core stays pure):

```
pub struct Settings { pub read: ReadSettings }
pub struct ReadSettings { pub freshness_policy: Option<FreshnessPolicy>, pub scan_limit: Option<usize> }
impl Settings { pub fn load(layout: &ProvenanceLayout) -> Result<Self, SettingsError> }
pub enum SettingsError { Unreadable { path, error }, Invalid { path, key, problem } }
impl ReadPolicy { pub fn resolve(settings: &Settings, freshness: Option<FreshnessPolicy>) -> Self }
```

`load` parses the file into `serde_json::Value` and walks the two keys by hand, so every `Invalid` names the
key that failed without a new dependency: an unknown key at either level, a `freshness_policy` that is not one
of the three words, a `scan_limit` that is not a whole number of at least 1. `FreshnessPolicy` gains
`Deserialize` with the three words. `SettingsError` is `thiserror`; the `Invalid` message reads
`.provenance/settings.json: read.freshness_policy must be one of catch_up, annotate_only, refuse_stale (found
"fast")`, and for the limit `read.scan_limit must be a whole number of at least 1 (found 0)`.

A settings refusal is a settings error raised before any read. It is not a freshness outcome: it never becomes
`catch_up_failed`, never lands in `freshness_error`, and no answer is produced beside it. A read that cannot
resolve its policy refuses before it opens anything and never falls back to a default (dispatch guidance of
2026-09-01, criterion 6, on bead `provenance-1wh`).

The eight `queries::*` functions take `policy: ReadPolicy` in place of the `ReadPolicy::default()` inside
`served` (`queries.rs:30-38`; `discover_repository` at `:36`, the default at `:37`). The only caller is
`handlers/sdk/query.rs:24-64`, which resolves the policy once per command; the Rust SDK crate does not call
`queries::*`. An in-process consumer (section J) resolves once and passes it on every call.

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
`a_zero_scan_limit_is_refused`, `a_settings_refusal_produces_no_answer_and_no_freshness_error`,
`the_freshness_flag_wins_over_the_settings_file` (CLI), `the_stamp_names_the_policy_the_flag_chose` (CLI).

Rules: `rule_invalid_read_setting_is_a_typed_refusal`, `rule_freshness_flag_wins_over_the_settings_file`. No
requirement in the default scope speaks of settings or configuration (the 102 requirements were read at
`03a935c`; the nearest, `req_query_answers_carry_a_freshness_stamp`, is about the stamp, not about how a policy
is chosen). K.2 therefore creates one requirement record, `req_read_settings_are_explicit_and_checked`, domain
`domain_cli`, statement in the shape of the grounded-writing skill: "A read's freshness policy and scan limit
come from one tracked settings file and an explicit flag, and an invalid setting refuses before any read." Its
source reference is a new Source for the dispatch guidance comment of 2026-09-01 on bead `provenance-1wh`
(criterion 6), and the two rules refine it.

## F. Deleting the baselines (item 5)

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
code for the wiki and gaps, and the property guards the one traversal home. The canonical `list_*` readers stay;
only the whole-graph load goes.

### F.2 What moves and what guards each deleted path afterwards

| Deleted path | Evidence afterwards |
|---|---|
| `get` against canonical | the twelve pinned `get` answers (`pinned.rs:61-72`); `get_reads_a_retired_record_only_when_asked` (`tests/retired.rs`); `get_answers_a_domain_and_a_boundary_by_id` (`tests.rs:91`) |
| `search` against canonical | the four pinned `search` answers (`pinned.rs:73-82`); `search_visits_kinds_in_rank_order_and_stops_at_the_limit` (`tests/limits.rs`); `search_reaches_domains_and_boundaries_by_kind_and_text`, `default_search_keeps_the_six_settled_kinds_under_protocol_five` (`tests.rs:118,148`) |
| `stale` against canonical | the pinned `stale` answer over the two-commit pinned store (`pinned.rs:134-141`); `a_bad_base_is_refused_before_the_store_is_read` (`tests/reader.rs:323`); `evidence_without_a_base_lists_no_diff` (`tests/reader.rs:246`) |
| kind order across `load` | `rank_appends_the_new_kinds_after_the_six_settled_positions` (`tests.rs:174`) stays; a new served case `search_answers_new_kinds_after_every_settled_kind` replaces the `load` order test |
| the repository's own state, every operation | a new check, `every_operation_answers_over_the_repository_state`: each operation answers `Ok` with a stamp over the `repository_state` store (`test_stores.rs:94-106`) |
| the repository's own state, canonical bytes | a new count invariant, `projection_counts_match_canonical_over_the_repository_state`: for each of the eight kinds and the three integration tables, `snapshot.table::<K>().count()` (`snapshot.rs:133-136`) equals the length of the canonical `list_*` reader's answer, retired records included; cheap, and it is the one comparison to canonical bytes over a store of real size after the baselines go |

`comparison/requests.rs::for_store` (`requests.rs:90-`) samples records through `records::load`; it samples through
the served `search` per kind instead (`Table<K>` listing through a snapshot is the alternative; `search` needs no
new reader). `test_stores.rs` keeps its four stores and `copy_tree`, which the timing report and the two new
tests use.

The timing report keeps one row per case (`operation request served_ms`), one summary row per operation,
`scan_ms`, `rebuild_ms`, and `catch_up_ms` (`timing.rs:38-76` minus the baseline column). The comparison is then
release to release, through the numbers in each release's notes (K.4).

For the record, the last baseline-against-served rows over a copy of this repository's state, release build,
`03a935c`, medians over three runs of the ignored test:

| Row | Baseline ms | Served ms | Ratio |
|---|---|---|---|
| get (24 cases, summary) | 1.6 | 1.0 | 0.60 |
| search (4 cases, summary) | 1.7 | 1.7 | 0.99 |
| scan (`scan_ms`) | 0.1 | | |
| first catch-up on a fresh file (`rebuild_ms`) | | 144 | |
| unchanged pass (`catch_up_ms`) | | 21.8 | |

Test count: the deletion removes four tests (`comparison.rs:181-208`) and one (`tests.rs:191`), and adds three.
The PR body names them. The pinned request set covering all eight operations and the timing test carrying
`#[ignore]` stay as tests (an exhaustion test and a source-scan test), not as Rule records: a Rule is a product
obligation, and no existing rule binds the test suite or the process.

## G. `refuse_stale` (item 6)

### G.1 The policy

`refuse_stale` takes the guard for the hash step only, as `catch_up` does (`freshness.rs:63-73`), lists the scopes,
hashes every unit in place (the step of C.2.3, no parse, no validator, no write), and compares each digest to
`projection_unit_digests`. Every unit equal: the read goes on at the stored serial and the stamp says `policy:
"refuse_stale"`, which the stamp word already exists for (`protocol/stamp.rs:37`, `protocol.ts:215`). Any unit
different: the read refuses. It never inserts a revision row and never rewrites a family. It shares the
`annotate_only` refusals for an absent, migration-behind, or half-migrated database (`freshness.rs:106-145`), since
no freshness step will run to fix them. `ReadRefusal::RefuseStaleUnimplemented` (`reader.rs:55-56`) is deleted.

The scope list comes from a lock-free manifest read, `units::scope_ids(state_dir)` (new): it reads
`manifest.json` bytes, parses the core `Manifest` type, and checks the schema version, without a `StateStore`.
The hash step needs scope ids and bytes, not records, and the manifest is written atomically (a `.tmp*` file
then a rename, `units.rs:68-72`), so an unlocked read sees one whole version of it. Today the only manifest read
locks (`state_store.rs:99-107`), which is why revision 1 had no lock-free path.

When the guard cannot be taken because of a permission error (`.provenance/cache/locks` not creatable, or the
lock file not openable read-write: `prepare_publication_lock`, `publication.rs:69-90`;
`LockedPublicationFile::acquire`, `guard.rs:33-47`), the hash runs unlocked with the same scope list. A
publication landing during an unlocked hash can only make a unit differ from its stored digest, so a torn read
refuses and never answers stale. Any other guard error refuses with that error.

A unit that cannot be hashed (an unreadable shard, a shard that vanishes mid-walk) refuses with
`ReadRefusal::UnitUnreadable { unit, path, error }`. Under `catch_up` the same I/O error is a failed freshness
step and the answer comes as `catch_up_failed`; under `refuse_stale` an answer whose freshness is unknown is the
one thing the policy exists to refuse.

### G.2 The typed refusal

```
ReadRefusal::Stale {
    database: Utf8PathBuf,
    serial: i64,
    digest: String,          // the stored revision digest, the value a caller holds from its last stamp
    instance_id: String,
    moved: Vec<MovedUnit>,   // MovedUnit { unit: String, stored: String, live: String }, sorted by unit name
}
```

A new scope has `stored` empty; a departed scope has `live` empty. The message is one line:

```
refuse_stale: the projection in .provenance/cache/provenance.db at serial 41 (digest sha256:d155..., instance
9a56781d-...) is behind canonical state; moved: scope:default (stored sha256:ab12..., live sha256:cd34...).
Read under catch_up or run `provenance materialize`.
```

On the wire the CLI prints every error as text to stderr and exits 1 (the `anyhow` main contract,
`main.rs:17-21`); the TypeScript engine turns a non-zero exit into a thrown `Error` carrying the text
(`engine.ts:84-88`). W5 lands the Rust type and the message. A JSON refusal envelope is the contract layer's
(`provenance-80qn`, whose text names a typed-refusal channel), and W5 does not invent one ahead of it. This is
question L.4.

Callers who see it: the CLI under `--freshness refuse_stale` or the settings file; the Rust SDK through
`anyhow::Error::downcast_ref::<ReadRefusal>()`; the TypeScript SDK through the message; an MCP server (section J)
through the type.

Tests: `refuse_stale_answers_at_the_stored_serial_when_every_unit_matches`,
`refuse_stale_refuses_and_names_the_moved_unit` (stored and live digests both present, the stored revision digest
on the refusal), `refuse_stale_names_a_new_scope_with_no_stored_digest`, `refuse_stale_writes_no_revision_row`,
`refuse_stale_refuses_a_unit_it_cannot_hash`, `refuse_stale_decides_when_the_lock_cannot_be_taken` (unix; the
fixture of section I), `refuse_stale_refuses_a_half_migrated_projection`.

Rules, under `req_query_answers_carry_a_freshness_stamp`: `rule_refuse_stale_names_the_moved_units`,
`rule_refuse_stale_writes_no_revision`.

## H. `catch_up_failed` (item 8)

Recommendation: keep the word. It names the step that ran (`catch_up`) and its outcome (`failed`) in the words of
the policy list, and the answer beside it is at the stored serial with the error text in `freshness_error`
(`freshness.rs:79-96`, `docs/cli.md:136-138`). The alternatives hide one half: `stored` and `stale_answer` drop
the step, `catch_up_refused` drops the I/O case. The word is already on the wire (`protocol.ts:216`), in the docs,
and in the TypeScript union; a rename would cost a type change for no gain in meaning. No code change; K.2 records
the confirmation in `docs/cache.md`.

## I. Read-only checkouts under WAL (item 9)

Recommendation: keep the immutable fallback. In WAL mode a reader needs a writable `-shm` file, and a completed
read removes the `-wal` and `-shm` files (`rule_completed_read_leaves_no_wal_files`, `cache.rs:127-147`), so a
read-only checkout usually holds neither; `immutable(true)` (`open_immutable_cache`, `cache.rs:96-105`) is the one
open SQLite offers that needs no lock and no `-shm`. The alternative, copying the database to a temporary
directory, costs the file size on every read and reintroduces a copy W5 is removing.

Two defects go with it. Under `annotate_only` a read-only checkout refuses as `NoProjection` today, because every
open error maps to that refusal (`freshness.rs:41-43`). And `ensure_current_schema` reads the manifest through a
locking store (`freshness.rs:135-136`), so on a checkout where the lock cannot be taken it fails at the lock
before any fallback is reached; it moves onto the lock-free `units::scope_ids` of G.1. After W5 every policy
falls back to the immutable open when the ordinary open fails for a permission reason, keeps its own policy word
(no freshness step failed under `annotate_only`; `refuse_stale` decides through an unlocked hash, G.1), and
`catch_up` keeps stamping `catch_up_failed` with the permission error in `freshness_error`, as today
(`a_read_only_checkout_answers_at_its_serial`, `reader/freshness.rs:106`).

Two fixtures, both unix. The existing one makes the cache directory `0o555` after a first read has created
`locks/` and the lock file (`reader/freshness.rs:112-118`), so the lock is still takeable and what fails is the
`-shm` creation; it stays for that case. The new one, `lock_untakeable(store)`, makes the lock file itself
`0o444` after the first read, so `LockedPublicationFile::acquire` fails at its read-write open (`guard.rs:37-43`)
and the "lock cannot be taken" branch of G.1 and of this section runs; the ordinary `0o555` on the cache
directory comes with it so the `-shm` case holds too.

Text for `docs/cache.md`, after the "Read path" paragraph (`docs/cache.md:79-93`):

```
A checkout whose cache directory this process cannot write still answers. WAL needs a writable -shm file
beside the database, and a finished read removes the -wal and -shm files, so the reader opens the database as
an immutable image instead: SQLite then takes no lock and reads the file as it is. The scope list for the
freshness step comes from an unlocked read of manifest.json, which is written atomically. Under catch_up the
stamp says catch_up_failed and freshness_error names the permission error; under annotate_only the policy
word is unchanged; under refuse_stale the units are hashed without the lock, and a publication landing
meanwhile can only turn the answer into a refusal. The answer is at the serial the file holds. This fallback
is exercised on Unix; on Windows a read-only directory is not reproduced in the test suite.
```

Tests: `annotate_only_on_a_read_only_checkout_answers_at_its_serial` (the `0o555` fixture),
`annotate_only_answers_when_the_lock_cannot_be_taken` (the new fixture), and the two refuse_stale cases of G.

Rule, under `req_query_answers_carry_a_freshness_stamp`: `rule_read_only_checkout_answers_as_an_immutable_image`.

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
3. Refusal. `refuse_stale` (G) gives a server a typed way to decline a stale answer, with the stored serial and
   digest and the moved units in the error.
4. No process-global state on the read path. `test_probes` is `cfg(test)` only (`test_probes.rs:10-39`); the
   read future is `Send` (`reader.rs:13-15,43`); nothing on the path calls `std::env::set_var`; the held lock
   state lives on the store, not in a process-wide set (C.2.2). A source-scan test over `operations/` asserts
   the `set_var` point.
5. Blocking sections named. The guard is taken on the blocking pool (`guard.rs:74-81`). The handoff writes the
   table below into `docs/cache.md`; moving these sections under `spawn_blocking` is the MCP bead's first task,
   not W5's.

   | Section | Where | What it blocks |
   |---|---|---|
   | the in-place hash of every unit under the guard | `catch_up_with_guard`, `catch_up.rs:98-106`; `units.rs:45-66` | a runtime worker for the hash of the whole state (about 2 ms here, about 100 ms at 38 MB) while the guard is held |
   | the validators' parse of a changed scope under the guard | `catch_up.rs:72-75` as moved by C.2.4 | a runtime worker for the parse, on a change only |
   | `LiveHandle::graph_evidence` | `live.rs:73-80`, through `health.rs:65` | a worker in `flock` while another holder has the lock |
   | `LiveHandle::store()` | `live.rs:66-69`; every read through the bare `StateStore` locks by path (`readers.rs:184-250`) | a worker in `flock` per read while another holder has the lock |
   | `LiveHandle::runs` | `live.rs:118-121`, `list_verification_runs` | a worker on the run file's lock |

6. Concurrency. Two reads in one process each open their own one-connection pool (`cache.rs:128-147`) and pin
   their own snapshot; two `catch_up` reads serialize on the file lock. A test with two concurrent `answer` calls
   over one store asserts both answer and both leave no `-wal` file.
7. Discovery. `discover_repository` runs per call today (`queries.rs:36`); a server passes a fixed repository
   path, which the `ReadPolicy` change leaves as an `Option<Utf8PathBuf>` argument.
8. Contract. The wire shapes a server projects are the ones `protocol.ts:205-251` already spells: the stamp, the
   four `has_more` flags, `scan_cut`, `freshness_error`, and the refusal text of G.2 until the contract layer
   gives it an envelope.

"Handoff criteria met" means: criteria 1 to 4 and 6 to 8 hold on the release commit with a test or a doc line
each; criterion 5's table is written; the bead comment lists the commit for each. Nothing here is an MCP
implementation.

## K. Staging

Five PRs, in dependency order. Each is green at every commit (fmt, clippy with all targets and features, the Rust
and TypeScript suites), deslopped before ready, reviewed by one adversarial reviewer, and lands its Rule records
and bindings. Each PR body states the test count before and after with the added and deleted tests named, and
asserts the pinned answers file is unchanged. Section F's deletion comes last on purpose: through K.1 to K.3 the
baseline comparison over the repository's state still guards `get`, `search`, and `stale` byte for byte.

*K.1a, branch `1wh-w5-readers-under-guard`: C.2.2 alone.* Steps: `Access` on the store; the five readers take
the store; `state_path_access` as a method; `GuardedStore<'g>` and `StateStore::under_guard`; the hand-written
`Clone`; the compile-fail case and doctest moved. The snapshot copy stays; catch-up and rebuild still read the
copy. Zero behaviour change: no served byte moves, the comparison tests are green, the test count grows by the
three K.1a tests of C.3. File cap: `readers.rs` (356) gains one argument per function; `ideation_batches.rs`
(492) cannot take a line, so its validation block (`validate_ideation_scope`, `_with_actor_ids`,
`_snapshot`, `ideation_batches.rs:127-237`, about 110 lines) moves to `state_store/ideation_validation.rs`
(new) first, in this PR. Rule: `rule_store_under_guard_takes_no_second_lock`.

*K.1b, branch `1wh-w5-in-place-catch-up`: the rest of section C.* Steps: in-place hashing with the hash, parse,
hash-again step; validators on changed units and on every scope after a global change; the rebuild in place;
delete the snapshot machinery; `VALIDATION_VERSION` and migration 023; `docs/cache.md:38-47` says the pass reads
the tree in place. Byte identity: pinned answers unchanged; the three comparison tests green. File cap:
`catch_up.rs` (279) grows by about 60 lines; the validation step goes in `materialize/validation.rs` (new) if
`catch_up.rs` passes 400. Rules: the four K.1b rules of C.3.

*K.2, branch `1wh-w5-read-settings`: sections E, H, I.* Steps: `settings.rs`, `ReadPolicy::resolve`, the eight
`queries::*` take a policy, `--freshness` on `QueryArgs`, the TypeScript `freshness` option; `units::scope_ids`
and `ensure_current_schema` on it; the immutable fallback under every policy and the `lock_untakeable` fixture;
`docs/cli.md` and `docs/cache.md` text; the scan default of D with its doc comment; the requirement record
`req_read_settings_are_explicit_and_checked` with its Source. Byte identity: no served answer changes; the
stamp's `policy` word is the resolved policy. Rules: the two of E.3 and the one of I.

*K.3, branch `1wh-w5-refuse-stale`: section G.* Depends on K.1b (the in-place hash without a write) and K.2
(`scope_ids`). Steps: the hash-only step as a function shared with catch-up; `ReadRefusal::Stale` and
`UnitUnreadable`; the policy arm in `freshness::run`; the message; tests. Byte identity: answers under
`refuse_stale` when current equal answers under `annotate_only` with the policy word swapped, asserted over the
pinned store. Rules: the two of G.2. Files: `reader/refuse_stale.rs` (new) for the step, `tests/reader/refuse_stale.rs`
(new) for its tests, so `freshness.rs` (162) and `tests/reader/freshness.rs` (262) stay where they are.

*K.4, branch `1wh-w5-baseline-removal`: section F and the release mechanics.* Steps: delete the baselines and the
canonical side; the three replacement tests; the timing report's served-only columns; `docs/release.md` gains
the command, the checklist line, and the criterion below; section J's criteria 4 and 6 tests and the criterion 5
table; the bead's handoff comment. Byte identity: pinned answers unchanged; the test count moves by the named
tests only. No Rule record: the PR deletes test code and documents a hand-run step.

The release cuts after K.4. The mechanics are decided here, not asked: one documented command and one checklist
line in `docs/release.md`, under "Cut A Release" before the tag step (`docs/release.md:45-59`); no `cargo xtask`
(the workspace has no xtask crate, `Cargo.toml:1-10`, and one command does not earn one).

```sh
cargo test -p provenance-store --release -- --ignored timing_comparison_rows --nocapture 2>/dev/null \
  | grep -E 'summary|_ms' > /tmp/timing-$(git rev-parse --short HEAD).txt
```

The rows for `repository_state` go into the GitHub Release body after the workflow generates it, under a heading
"Timing (release build, `<commit>`)". The criterion, within the ruling that this is hand-run and never CI: each
operation's served summary row and `catch_up_ms` over `repository_state` are compared to the same rows in the
previous release's notes; a row that is more than twice the previous value and more than 5 ms above it sends the
release back until the cause is named in the notes or fixed. `scan_ms` and `rebuild_ms` are recorded and not
gated: the scan depends on the checkout's file count, and the rebuild row holds the test binary's first SQLite
open (C.1). For the first release the reference rows are the ones in F.2 (get 1.0 ms, search 1.7 ms, unchanged
pass 21.8 ms), taken at `03a935c` before K.1b, so the first comparison also shows what item 2 bought.

## L. Questions for Ben

Each is self-contained; the plan proceeds on the recommendation unless told otherwise.

1. The scan default. The release build scans about 10 ms per MB of source, which is about 0.1 ms per file at
   ordinary file sizes (rust-analyzer, 1511 files and 17.7 MB, takes 0.15 s; the debug build takes 1.3 s). Keep
   2000, or raise to 5000, which keeps the worst release scan under half a second while the scanned source is
   under about 50 MB, and cuts none of the four measured repositories? Recommendation: 5000.
2. The settings file. Put `read.freshness_policy` and `read.scan_limit` in `.provenance/settings.json`, tracked in
   git beside `state/` and `cache/`, in JSON like the manifest? Recommendation: yes, that file and that place.
3. The flag. Give the eight `sdk` query commands `--freshness <catch_up|annotate_only|refuse_stale>`, winning over
   the file, so a CI job or a server can pick a policy without editing a tracked file? Recommendation: yes; no
   environment variable.
4. The refusal on the wire. Ship `refuse_stale` as a typed Rust error whose one-line message names the stored
   serial and digest and each moved unit, printed to stderr with exit 1 as every error is today, and leave the
   JSON refusal envelope to the contract layer bead? Recommendation: yes, no envelope ahead of `provenance-80qn`.
5. Validation on unchanged scopes. Let catch-up validate only the scopes whose bytes moved, plus every scope when
   the manifest moved, and rebuild once when a release moves the validation version, so an unchanged pass parses
   nothing? The one visible change: a scope that fails a newer validator without changing is served until that
   release's first read triggers the rebuild. Recommendation: yes; it is most of the per-read cost.
6. Priority. Raise this bead from P2 to P1? It gates the release that carries W3, which was P1, and nothing else
   is scheduled between them. Recommendation: raise it.

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
- Bead `provenance-1wh.3` as it stands (description and acceptance criteria of 2026-09-03; Ben's comment of
  2026-09-06 02:16 naming what was overruled).
- Beads `provenance-q82f` (MCP server) and `provenance-80qn` (contract layer) for section J and G.2.
- The adversarial review of revision 1 (Fable, 2026-09-06), answered in section N; it goes into the plan's gist
  beside the plan on approval.
- Measurements: release CLI built from `03a935c` with `cargo build --release -p provenance-cli`, debug CLI with
  `cargo build -p provenance-cli`, `cargo test -p provenance-store --release -- --ignored timing_comparison_rows
  --nocapture` (four runs), and the clone-and-time procedure of D.1; medians, single machine, 2026-09-06. The
  scripts are reproducible from the commands in C.1 and D.1; on approval this plan goes to a gist with its
  reviews and is cited as a Source, as the W3 plan was.

## N. Review response (revision 1 review, Fable, 2026-09-06)

Majors.

- M1 (readers lock by path). Verified: `read_jsonl`, `read_ideation_landings`, `read_legacy_dispositions`,
  `read_jsonl_closed` (`readers.rs:184,194,205,250`) and `read_message_shards` (`readers.rs:309`) lock through
  `with_state_path_access` (`publication.rs:437-453`) or the free `with_repository_publication`, and the
  `StateStore` method (`publication.rs:456-461`) is not on their path. Changed: C.2.2 rewritten; the bead text
  says so; K.1 split into K.1a (readers take their lock state from the store, snapshot kept, zero behaviour
  change) and K.1b (in place). Line counts: `ideation_batches.rs` 492, split named in K.1a; `readers.rs` 356.
- M2 (lock-free scope list, fixture). Changed: `units::scope_ids` reads `manifest.json` without a store (G.1);
  `ensure_current_schema` moves onto it (I); the `lock_untakeable` fixture makes the lock file `0o444` so
  `acquire` fails at open (I); the affected tests are renamed to say which fixture they use.
- M3 (the 500-line gate). Verified: `tracked_rust_files_stay_below_the_hard_line_limit`
  (`cli_structure.rs:31-42`) runs in every CI test job through `cargo test --workspace` (`ci.yml:197`). Changed:
  the bead text says the gate is live and nothing more is built; no question needed.

Minors.

- m1: `GuardedStore<'g>` with `PhantomData<&'g PublicationGuard>`, no `Clone`, and a hand-written `Clone` on the
  plain store that always locks (C.2.2).
- m2: `ideation_batches.rs:127-237` moves to `state_store/ideation_validation.rs` in K.1a.
- m3: re-measured; C.1 and F.2 label each number; the two rebuild figures are corrected.
- m4: the count invariant `projection_counts_match_canonical_over_the_repository_state` (F.2).
- m5: `VALIDATION_VERSION` and migration 023 route to a rebuild (C.2.7); L.5 names it.
- m6: L.5 (the old Q5) dropped; the priority raise is L.6.
- m7: `digest` on `ReadRefusal::Stale`; an unhashable unit refuses with `UnitUnreadable` (G.1, G.2).
- m8: the J.5 table adds the in-place hash and parse under the guard and `LiveHandle::store()`.
- m9: the two test-suite rules are dropped and their tests kept (F.2); every rule names its parent; K.2 creates
  `req_read_settings_are_explicit_and_checked` for the settings rules (E.3).
- m10: D.1 gives ms per MB; D.2 states the 50 MB assumption and puts it in the doc comment.
- m11: K.4 states the criterion (twice the previous row and more than 5 ms above it) and the reference rows.
- m12: E.2 states that a settings refusal is not a freshness outcome.

Nits: `queries.rs:36` and `:37` corrected; `engine.ts:84-88`; the lock sentence now cites the canonical writers
and the held guard, not `docs/cache.md:71-75`; `main.rs:17-21` is cited as the `anyhow` contract; "seams",
"smoke", "RED first", and "process floor" replaced; E.2 names the hand walk over `serde_json::Value`; E.1 says the
stamp does not carry the settings; C.2.4 says the exhausted retry answers `catch_up_failed`. `docs/cli.md` is a
document and outside the gate, which counts tracked `*.rs` files only (`cli_structure.rs:84-105`).

Stands: the staging order (deletion last, so the baseline guards K.1a and K.1b), the immutable fallback, the
`catch_up_failed` word, no environment variable, no JSON envelope ahead of the contract layer.
