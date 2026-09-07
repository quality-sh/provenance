# Pull request 205 verification: the second close-order fix

Verifier run against `origin/1wh-w5-release-mechanics` at head `716bd00`
(local branch `verify-pr205`, created read-only from the fetched remote
branch and pushed nowhere). Every number below was reproduced in this run.
Builds and tests ran in the foreground with `CARGO_BUILD_JOBS=3`; `df -h /`
was checked before each build; each build directory was deleted when the
phase that needed it finished, and no build directory remains.

## Verdict

mergeable. The original defect stays closed under repetition ordinary and
under load; the two majors the adversarial review found at `ad3098a` are
gone on `716bd00` and their tests fail on a revert of the fix, so none of
the guards passes vacuously; the retry wait costs no measurable throughput;
cross-repository closes do not block; immutable reads still bypass the lock
and leave nothing behind. The gates pass and the changed text is clean.

## Per-finding status

| Finding | Status on `716bd00` | Evidence |
|---|---|---|
| Two simultaneous reads leave `provenance.db-wal` behind | Closed | 6,000 test executions over 3,600 fresh processes, zero failures (below) |
| Close lock waits on the blocking pool and can deadlock | Fixed | Both `close_progress` tests finish on head; both time out (`timed_out=true`, 301 ms and 2,001 ms) on the reverted close |
| Cancellation or a panic releases the lock before teardown ends | Fixed | Both `close_cancellation` tests pass on head and fail on the revert with the early-release assertion at `close_cancellation.rs:110` |
| Retry sleep costs read throughput | No measurable cost | Interleaved 800-read rounds pinned under steady load: main 248–268 reads/s, this branch 246–264 reads/s |
| Two repositories closing in one process block each other | They do not | Second repository closes in 0–1 ms while the first close is paused holding its own lock; the paused owner still holds it |
| Immutable read no longer bypasses the lock | Still bypasses | Immutable close finishes in 0 ms while a normal close of the same database holds the lock; creates no lock file; leaves no -wal or -shm files |

## New findings

1. **A test-only pause inside SQLite teardown can deadlock a
   current-thread runtime — probe-design caveat, not a branch defect.** The
   first version of two probes in this verification paused the physical
   close (progress-handler destructor, the reviewer's technique) on a
   `current_thread` runtime. That hung intermittently (about one run in
   three under load). Stack dump at the hang, taken with gdb:
   `Condvar::wait` inside `PauseOnDrop::drop` called from
   `ConnectionState::drop` inside `SqliteConnection::close` running on the
   runtime thread itself. In sqlx 0.8.6, `PoolInner::close` pops idle
   connections and closes them inline in the caller's task
   (`pool/inner.rs:97-115`), and `SqliteConnection::close` drops the
   connection state on that thread, racing the connection's worker thread
   for the last `Arc<ConnectionState>` reference. When the runtime thread
   loses the race it blocks inside the pause, and even a
   `tokio::time::timeout` wrapped around the test can never fire because
   nothing drives the runtime. Both sqlx versions are 0.8.6 on `origin/main`
   and on this branch, the pause hook exists only in tests, and the
   permanent close tests use multi-thread runtimes, so nothing here is
   reachable from production or introduced by this branch. The probes were
   rebuilt on multi-thread runtimes (two task workers) and then passed 42
   consecutive rounds under load. Worth keeping in mind for future close
   tests: pause-based close tests must not run their close on a
   single-threaded runtime.
2. **`provenance prime` and `provenance gaps` never open the cache
   database.** Both read the state store directly
   (`cache/prime.rs:32-41`, `cache/gaps/state_adapter.rs:9-12`), so the
   byte-identity check below is a genuine output-identity result for those
   commands, but it does not exercise the close lock. `materialize` was run
   in addition so the projection instance id existed at all; the -wal and
   -shm files were gone after every command in both copies, and the
   branch's persistent `provenance.db.close.lock` appears only in the copy
   primed by this branch's binary, as documented.
3. **Contention probe that does not discriminate.** Four simultaneous
   closes of one database on a one-worker blocking pool pass on both the
   reverted close and the fixed close: the first fix's `spawn_blocking`
   flock attempt releases its worker as soon as the lock is granted, so
   this shape alone never starves. The discriminating probes remain the
   two `close_progress` tests, where the cycle includes a publication
   waiter that parks the workers. Reported so the probe's pass is not
   overread.

## Reproductions with exact commands and observed output

The release store test binary was rebuilt after the source change below;
its path is `target/release/deps/provenance_store-1933c933e9756c04`
(`$BIN`). One fresh process per repetition.

### The original defect stays closed (must be zero failures; is zero)

Each iteration ran both concurrent tests in one process, or the
eight-reader test alone:

```text
$BIN --exact \
  operations::queries::tests::concurrent::answers_that_close_together_remove_the_wal_files \
  operations::queries::tests::concurrent::eight_answers_that_close_together_remove_the_wal_files \
  --test-threads=2
$BIN --exact \
  operations::queries::tests::concurrent::eight_answers_that_close_together_remove_the_wal_files \
  --test-threads=1
```

Loaded runs pinned the test process and four busy loops to CPUs 0–3
(`taskset -c 0-3 $BIN ...`, `taskset -c $c bash -c 'while :; do :; done'`
for `c` in 0 1 2 3):

```text
ordinary pair   FINAL iterations=600 tests=1200 failures=0 elapsed=85s
ordinary eight  FINAL iterations=600 tests=600  failures=0 elapsed=69s
loaded pair     FINAL iterations=600 tests=1200 failures=0 elapsed=96s
loaded eight    FINAL iterations=600 tests=600  failures=0 elapsed=84s
```

Per test: `answers_that_close_together_remove_the_wal_files` 1,200
executions (600 ordinary + 600 loaded); `eight_answers_that_close_together`
2,400 (600 ordinary + 600 loaded in pairs, 600 + 600 solo). Both above 500
in each mode. Every iteration asserts the same serial and instance id for
all overlapping reads and requires both the -wal and -shm files to be gone.

### The blocking-pool deadlock is gone, and the probes that found it bite

On head, all eight permanent close tests pass:

```text
$BIN --exact cache::tests::close_cancellation::... cache::tests::close_progress::... \
     cache::tests::close_order_behavior::... --test-threads=8
test result: ok. 8 passed; 0 failed; ... finished in 0.33s
100 repetitions of the eight: iterations=100 tests=800 failures=0 elapsed=38s
```

On the reverted close (the `ad3098a` first fix restored in `cache.rs`:
blocking `flock` inside `spawn_blocking`, lock owned by the caller's
future; tests unchanged), the same tests fail:

```text
test cache::tests::close_cancellation::cancelled_close_keeps_the_lock_until_sqlite_finishes ... FAILED
thread ... panicked at close_cancellation.rs:110:5      # early lock release
test cache::tests::close_cancellation::panicking_close_keeps_the_lock_until_sqlite_finishes ... FAILED
thread ... panicked at close_cancellation.rs:110:5
test cache::tests::close_progress::failed_freshness_closes_with_one_busy_blocking_worker ... FAILED
blocking_workers=1 read_timed_out=true cycle=read_holds_publication_lock/worker_waits_publication_lock/read_close_waits_worker
blocking_workers=1 elapsed_ms=301 timed_out=true
test cache::tests::close_progress::materialization_closes_with_four_busy_blocking_workers ... FAILED
blocking_workers=4 materialization_timed_out=true fault_injected=false
blocking_workers=4 elapsed_ms=2001 timed_out=true
a_read_closes_while_the_only_blocking_worker_waits_for_its_guard ... FAILED
a_materialization_closes_while_every_blocking_worker_waits_for_its_guard ... FAILED
a_reader_cancelled_mid_close_keeps_the_close_order ... FAILED
a_reader_panicking_mid_close_keeps_the_close_order ... FAILED
test result: FAILED. 0 passed; 4 failed; ... finished in 10.01s
```

The reviewer's two cycles reproduce exactly on the revert and disappear on
head. `cache.rs` was then restored with `git checkout --` and the eight
tests passed again before any other step ran.

### New deadlock constructions (probes; untracked files, removed afterwards)

Probe file `crates/provenance-store/src/cache/tests/verify_probes.rs` plus
one `mod verify_probes;` line in `cache/tests/mod.rs`; both removed and
`mod.rs` restored after the run, so the branch carries nothing.

- More waiters than workers: four pools on one database close together on
  `max_blocking_threads(1)`:
  `probe=more_waiters_than_workers finished=true elapsed_ms=6`
- A close waiting while another close holds the lock and its owner is
  descheduled: owner paused inside its physical teardown, both blocking
  workers busy (owner's teardown plus a parked publication waiter). The
  second close stays blocked for the whole 300 ms proof window and finishes
  only after the teardown resumes:
  `probe=paused_owner_with_busy_workers finished=true blocked_proof=300ms`
  with `stage=blocked_proof_elapsed blocked=true` then
  `stage=owner_finished`, `stage=second_finished`, `stage=waiter_finished`.
- Two databases in one process:
  `probe=two_databases serial_pair_ms=1 close_while_owner_paused_ms=0 paused_owner_still_holds=true joined_pair_ms=0`
  (other rounds: 1–6 ms serial pair, 0–1 ms while the owner is paused,
  joined pair 0–2 ms).
- Immutable read: creates no `provenance.db.close.lock`, leaves no -wal or
  -shm files, and closes while a normal close of the same database holds
  the lock: `probe=immutable_bypass finished_while_lock_held=true elapsed_ms=0`.
- Stability: 42 consecutive rounds of the four probes under the four-busy-
  loop load: `rounds=12 failures=0` and `rounds=30 failures=0`. The earlier
  hangs were the probe defect in New findings 1, diagnosed and removed.

### The retry sleep does not cost throughput

Probe `crates/provenance-store/tests/read_throughput_probe.rs` (untracked,
removed afterwards; identical bytes in both trees, same test-binary hash):
seed one projection, then 100 rounds of eight simultaneous reads, 800
reads per run, interleaved main/branch pairs on the same machine, pinned to
CPUs 0–3 with the four-busy-loop load running:

```text
pair 1 main reads=800 elapsed_ms=3221 reads_per_sec=248.32 | pr reads=800 elapsed_ms=3202 reads_per_sec=249.82
pair 2 main 268.38 | pr 262.43
pair 3 main 264.59 | pr 263.45
pair 4 main 250.20 | pr 246.42
pair 5 main 256.68 | pr 262.84
pair 6 main 241.44 | pr 246.00
pair 7 main 252.91 | pr 257.53
```

main median 252.9, branch median 257.5 reads/s — indistinguishable. A
first unpinned interleaved series (80-read rounds, 20 pairs) gave main
median 558.5 vs branch 522.5 with a 13/7 sign split, not significant and
contradicted by the pinned series; the branch pays no measurable cost.

### Byte identity of prime, gaps and check against origin/main

Two detached worktrees at `716bd00` (`git worktree add`, byte-identical
tracked state), each driven by a different release binary
(`target/release/provenance` and `.verify-main/target/release/provenance`).
Each copy ran `materialize --repo .`, then the three commands; raw outputs
diffed:

```text
PRIME BYTE-IDENTICAL   (12,477 bytes)
GAPS BYTE-IDENTICAL    (9,336 bytes)
CHECK BYTE-IDENTICAL   (42 bytes: {"status":"ok","diagnostics":[]})
```

The random projection instance ids in the two caches differ —
`fc6320f1-556b-74ad-b31d-4673cbb37a0c` versus
`ea5c4aa2-0831-40e3-a1cd-d83b8cd80b1b` (the shared
`01a05ccf-...` uuid is a schema id) — so the identity holds across
independent primings, and no instance id leaks into the three outputs.
After `materialize`, the branch copy keeps `provenance.db.close.lock` and
the main copy does not; both copies keep no -wal or -shm files. See New
findings 2 for what prime and gaps actually read.

### Coverage scan warning count

`provenance coverage scan --path . --scope default --validate-rules` in
both copies: outputs byte-identical, `"warnings"` count 135 in both. The
same command on the real repository with this branch's binary after
cleanup: `rc=0 warnings=135`.

### Gates

```text
cargo fmt --all --check                                    pass
cargo clippy --workspace --all-targets --all-features -- -D warnings
                                                           pass (1m07s)
cargo test --workspace --all-features                      138 suites, 0 failed
cargo test -p provenance-cli --no-fail-fast                93 suites, 0 failed
                                                           (default features)
provenance check                                           {"status":"ok","diagnostics":[]}
```

One environmental note: with `CARGO_TARGET_DIR` pointed outside the
workspace, the sdk `compile_fail` test fails because its blessed stderr
contains `$WORKSPACE/target/tests/trybuild/...`; with the default target
directory it passes. The suite numbers above are from the default
directory. Two external interruptions during the run (the workspace
`target/` directory was deleted by something outside this session between
phases) cost one release rebuild and invalidated nothing; the affected
steps were rerun afterwards.

### Things checked and found fine

- Every tracked Rust file under 500 lines: `git ls-files '*.rs' | xargs
  wc -l` peaks at 498 (`state_store/typed_specs/adoption.rs`).
- No jargon in the changed text (`git diff origin/main..HEAD` grep for
  corpus, oracle, golden, sidecar, harness, knob: no matches) and none in
  the PR body (`gh pr view 205 --json body`, same grep: no matches). The
  two files are called the -wal and -shm files throughout.
- PR head is the verified commit: `headRefOid 716bd004c95a5f57da806c3c988ac8f759e52428`
  as reported by `gh`, matching the local checkout.
- The pool-return test now checks the close result; no test was deleted or
  ignored (`git diff origin/main..HEAD --stat`: five test files added, none
  removed).
- Build hygiene: `df -h /` before every build (61–64 GB free throughout);
  one build directory at a time; the workspace `target/`, the main
  worktree and its target, and `/tmp/opencode/pr205` scratch were all
  deleted at the end; `git status` clean on `verify-pr205` at `716bd00`
  and the branch was pushed nowhere.
- Report written on the run branch `agent-run/4d456c468505809a`.

mergeable
